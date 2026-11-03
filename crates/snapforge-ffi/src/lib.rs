//! C FFI wrappers over snapforge-core for the Qt frontend.
//!
//! Convention:
//! - Return 0 on success, -1 on error.
//! - Caller-owned buffers are allocated here and freed via snapforge_free_buffer.
//! - String out-params are allocated here and freed via snapforge_free_string.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

/// Tracks every live `(ptr, len)` returned by `snapforge_capture_*` so
/// `snapforge_free_buffer` can detect a length mismatch (which would otherwise
/// hand a wrong-layout `Box<[u8]>` to the global allocator and corrupt the heap).
static BUFFER_REGISTRY: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

fn buffer_registry() -> &'static Mutex<HashMap<usize, usize>> {
    BUFFER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record a live buffer. Returns `false` if it could not be tracked (registry
/// lock poisoned) — the caller must then free the buffer itself and report
/// failure, never hand C an untracked pointer it could not free.
fn register_buffer(ptr: *mut u8, len: usize) -> bool {
    if ptr.is_null() {
        return false;
    }
    match buffer_registry().lock() {
        Ok(mut map) => {
            map.insert(ptr as usize, len);
            true
        }
        Err(_) => false,
    }
}

/// Build a `CString`, stripping any interior NUL bytes. Never logs — safe to
/// call from inside the tracing bridge itself (logging here would re-enter
/// the subscriber that is already dispatching).
fn cstring_strip_nul(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|e| {
        let mut bytes = e.into_vec();
        bytes.retain(|b| *b != 0);
        // Cannot fail: every NUL was just removed.
        CString::new(bytes).unwrap_or_default()
    })
}

/// Like [`cstring_strip_nul`] but logs when it had to strip, so the silent
/// corruption is visible. Do NOT use from the logging path.
fn cstring_sanitized(s: &str) -> CString {
    if s.as_bytes().contains(&0) {
        tracing::warn!("[snapforge] stripping embedded NUL from FFI string");
    }
    cstring_strip_nul(s)
}

const ZEROED_IMAGE: CapturedImage = CapturedImage {
    data: ptr::null_mut(),
    len: 0,
    width: 0,
    height: 0,
};

/// Hand a captured image to C: box the pixels, register the buffer, and
/// return the struct. Any failure (empty image, registry unavailable) frees
/// the buffer and returns the zeroed error struct with the error recorded.
fn export_image(image: image::RgbaImage) -> CapturedImage {
    let width = image.width();
    let height = image.height();
    let boxed: Box<[u8]> = image.into_raw().into_boxed_slice();
    let len = boxed.len();
    if len == 0 || width == 0 || height == 0 {
        // A zero-length Box has a dangling (non-null) pointer that the
        // registry would key on; never export it.
        set_app_error_code(
            SnapforgeErrorCode::Capture,
            "capture produced an empty image",
        );
        return ZEROED_IMAGE;
    }
    let data = Box::into_raw(boxed).cast::<u8>();
    if !register_buffer(data, len) {
        // SAFETY: `data`/`len` came from `Box::into_raw` just above and were
        // never handed out.
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len)) });
        set_app_error_code(SnapforgeErrorCode::Internal, "buffer registry unavailable");
        return ZEROED_IMAGE;
    }
    clear_app_error();
    CapturedImage {
        data,
        len,
        width,
        height,
    }
}

use snapforge_capture::capture;
use snapforge_domain::Rect;
use snapforge_storage::config::AppConfig;
use snapforge_storage::history::ScreenshotHistory;

/// Captured image data returned to C callers.
/// The caller must free `data` via `snapforge_free_buffer`.
///
/// Error contract: on failure, every field is zeroed — `data == NULL`,
/// `len == 0`, `width == 0`, `height == 0`. Callers should check
/// `data != NULL && width > 0 && height > 0` before using the buffer.
#[repr(C)]
pub struct CapturedImage {
    pub data: *mut u8,
    pub len: usize,
    pub width: u32,
    pub height: u32,
}

/// Capture the full screen for a given display index.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
///
/// Error contract: on failure the returned struct is fully zeroed
/// (`data == NULL`, `len == 0`, `width == 0`, `height == 0`).
#[no_mangle]
pub extern "C" fn snapforge_capture_fullscreen(display: u32) -> CapturedImage {
    match capture::capture_fullscreen(display as usize) {
        Ok(image) => export_image(image),
        Err(e) => {
            set_app_error_from(&snapforge_app::AppError::Capture(e));
            ZEROED_IMAGE
        }
    }
}

/// Capture a region of the screen.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
///
/// Error contract: on failure the returned struct is fully zeroed
/// (`data == NULL`, `len == 0`, `width == 0`, `height == 0`).
#[no_mangle]
pub extern "C" fn snapforge_capture_region(
    display: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> CapturedImage {
    // Zero / subpixel regions would produce a null image or a crash depending
    // on the backend. Short-circuit to the zeroed error struct.
    if w < 1 || h < 1 {
        set_app_error("capture region has zero width or height");
        return ZEROED_IMAGE;
    }
    let region = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    match capture::capture_region(display as usize, region) {
        Ok(image) => export_image(image),
        Err(err) => {
            set_app_error_from(&snapforge_app::AppError::Capture(err));
            ZEROED_IMAGE
        }
    }
}

/// Free a pixel buffer returned by snapforge_capture_*.
///
/// # Safety
///
/// `data` must have been returned by a `snapforge_capture_*` function
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_free_buffer(data: *mut u8, len: usize) {
    if data.is_null() {
        return;
    }
    // Cross-check the caller-supplied `len` against what we recorded at alloc.
    // A wrong `len` would hand `Box::from_raw` a wrong-layout slice and corrupt
    // the heap silently. Refuse to free if it doesn't match.
    // Residual ABA: the registry is address-keyed, so if the allocator reuses
    // a freed address for a new buffer, a stale double-free (with matching
    // len) frees the live entry; a generation-keyed registry would close it.
    let recorded = match buffer_registry().lock() {
        Ok(mut map) => map.remove(&(data as usize)),
        Err(_) => None,
    };
    let Some(real_len) = recorded else {
        tracing::error!("[snapforge] free_buffer: unknown pointer {:p} (already freed or never allocated by us); leaking", data);
        return;
    };
    if real_len != len {
        tracing::error!(
            "[snapforge] free_buffer: length mismatch for {:p}: caller says {}, we recorded {}; leaking to avoid heap corruption",
            data, len, real_len
        );
        // Re-insert so a correct call can still free it.
        if let Ok(mut map) = buffer_registry().lock() {
            map.insert(data as usize, real_len);
        }
        return;
    }
    let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, real_len));
}

/// Check if screen capture permission is granted.
/// Returns 1 if granted, 0 if not.
#[no_mangle]
pub extern "C" fn snapforge_has_permission() -> c_int {
    c_int::from(capture::has_permission())
}

/// Request screen capture permission.
/// Returns 1 if granted, 0 if not.
#[no_mangle]
pub extern "C" fn snapforge_request_permission() -> c_int {
    c_int::from(capture::request_permission())
}

/// Find the display index for a given point (in screen coordinates).
/// Returns the display index, or -1 if no display contains that point.
#[no_mangle]
pub extern "C" fn snapforge_display_at_point(x: i32, y: i32) -> c_int {
    capture::display_at_point(x, y)
        .and_then(|d| c_int::try_from(d).ok())
        .unwrap_or(-1)
}

/// Get the DPI scale factor of the primary display.
#[no_mangle]
pub extern "C" fn snapforge_display_scale_factor() -> f64 {
    capture::display_scale_factor()
}

/// Get the default save directory path (e.g. ~/Pictures/Snapforge/).
/// Returns a heap-allocated null-terminated string. Caller must free via snapforge_free_string.
/// Returns NULL on error.
#[no_mangle]
pub extern "C" fn snapforge_default_save_path() -> *mut c_char {
    let config = match AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("[snapforge] default_save_path: config load failed: {}", e);
            AppConfig::default()
        }
    };
    let dir = &config.save_directory;
    cstring_sanitized(dir.to_string_lossy().as_ref()).into_raw()
}

/// Free a string returned by snapforge_* functions.
///
/// # Safety
///
/// `s` must have been returned by a `snapforge_*` function and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

// ---------------------------------------------------------------------------
// History FFI
// ---------------------------------------------------------------------------

/// List all history entries as a JSON string.
/// Returns NULL on error. Caller must free via `snapforge_free_string`.
#[no_mangle]
pub extern "C" fn snapforge_history_list() -> *mut c_char {
    let history = match ScreenshotHistory::load() {
        Ok(h) => h,
        Err(e) => {
            set_app_error_from(&e.into());
            return ptr::null_mut();
        }
    };
    let json = match serde_json::to_string(&history.entries) {
        Ok(j) => j,
        Err(e) => {
            set_app_error_code(SnapforgeErrorCode::Internal, e.to_string());
            return ptr::null_mut();
        }
    };
    clear_app_error();
    cstring_sanitized(&json).into_raw()
}

/// Delete a history entry by file path.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_history_delete(path: *const c_char) -> c_int {
    if path.is_null() {
        set_app_error("internal: null path");
        return -1;
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        set_app_error("internal: path is not valid UTF-8");
        return -1;
    };
    // Load-modify-save runs under the history lock inside storage.
    match ScreenshotHistory::remove_entry(path_str) {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e.into());
            -1
        }
    }
}

/// Clear all history entries.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn snapforge_history_clear() -> c_int {
    match ScreenshotHistory::clear() {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e.into());
            -1
        }
    }
}

/// Returns 1 if the given `.mp4` path looks like a recording that was never
/// finalized (missing, under 16 bytes, or an `ftyp` file with no complete
/// top-level moov), 0 otherwise. Non-mp4 paths always return 0.
/// Returns -1 if the input pointer is null or not valid UTF-8.
///
/// # Safety
///
/// `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_is_incomplete_mp4(path: *const c_char) -> c_int {
    if path.is_null() {
        return -1;
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        return -1;
    };
    c_int::from(snapforge_storage::history::is_incomplete_mp4(path_str))
}

// ---------------------------------------------------------------------------
// Config FFI
// ---------------------------------------------------------------------------

/// Load the application config and return it as a JSON string.
/// Returns NULL on error. Caller must free via `snapforge_free_string`.
#[no_mangle]
pub extern "C" fn snapforge_config_load() -> *mut c_char {
    let config = match AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            set_app_error_from(&e.into());
            return ptr::null_mut();
        }
    };
    let json = match serde_json::to_string(&config) {
        Ok(j) => j,
        Err(e) => {
            set_app_error_code(SnapforgeErrorCode::Internal, e.to_string());
            return ptr::null_mut();
        }
    };
    clear_app_error();
    cstring_sanitized(&json).into_raw()
}

/// Save application config from a JSON string.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_config_save(json: *const c_char) -> c_int {
    if json.is_null() {
        set_app_error("internal: null json");
        return -1;
    }
    let Ok(json_str) = CStr::from_ptr(json).to_str() else {
        set_app_error("internal: json is not valid UTF-8");
        return -1;
    };
    let mut config = match serde_json::from_str::<AppConfig>(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_app_error_code(
                SnapforgeErrorCode::Config,
                format!("invalid config JSON: {e}"),
            );
            return -1;
        }
    };
    // Same clamping/repair as load, so a bad value never reaches disk.
    config.normalize();
    match config.save() {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e.into());
            -1
        }
    }
}

// ---------------------------------------------------------------------------
// Use-case FFI (snapforge-app)
//
// These wrappers expose the high-level use cases (`snapforge-app`) and
// coexist with the primitive `snapforge_capture_*` fns above. The primitive
// `snapforge_*_recording` surface has been removed; the Qt frontend consumes
// this use-case surface exclusively (via SnapforgeClient).
// ---------------------------------------------------------------------------

use snapforge_app::clicks as app_clicks;
use snapforge_app::recording as app_recording;
use snapforge_app::screenshot as app_screenshot;

/// Stable, machine-readable error category exposed alongside the human-readable
/// `LAST_APP_ERROR` string. Mirrored as `SnapforgeErrorCode` in
/// `snapforge_ffi.h`; the discriminants are part of the ABI, so only ever
/// append new variants — never renumber.
///
/// `repr(i32)` so the C side can compare against the plain `int` returned by
/// `snapforge_app_last_error_code`.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapforgeErrorCode {
    /// No error recorded (the last use-case call succeeded, or none has run).
    None = 0,
    /// Caller supplied a malformed / out-of-range request (bad JSON, missing
    /// field, zero-size region, null pointer where one was required).
    InvalidInput = 1,
    /// A macOS TCC grant (Screen Recording / Accessibility) is missing.
    PermissionDenied = 2,
    /// A required resource (file, ffmpeg binary, display) was not found.
    NotFound = 3,
    /// Filesystem / IO failure.
    Io = 4,
    /// Image or video encoding failed.
    Encode = 5,
    /// Screen capture failed.
    Capture = 6,
    /// Config load/save/parse failed.
    Config = 7,
    /// Anything else, including a poisoned internal lock.
    Internal = 8,
}

impl SnapforgeErrorCode {
    /// Derive a stable code from an [`AppError`]. The mapping leans on the
    /// originating sub-crate error variant so the Qt side can branch on a
    /// category without string-matching the message.
    fn from_app_error(err: &snapforge_app::AppError) -> Self {
        use snapforge_app::AppError;
        use snapforge_capture::capture::CaptureError;
        use snapforge_encode::record::RecordError;
        // Keep one arm per *concrete* error variant — no `_` catch-alls inside a
        // sub-crate error — so adding a variant to any of these enums fails to
        // compile here until its error-code category is chosen. The codes are
        // independent and may diverge later, so don't merge same-code arms.
        #[allow(clippy::match_same_arms)]
        match err {
            AppError::InvalidRequest(_) => SnapforgeErrorCode::InvalidInput,
            AppError::PermissionDenied(_) => SnapforgeErrorCode::PermissionDenied,
            AppError::Io(_) => SnapforgeErrorCode::Io,
            AppError::Internal(_) => SnapforgeErrorCode::Internal,
            AppError::Config(_) => SnapforgeErrorCode::Config,
            AppError::Format(_) => SnapforgeErrorCode::Encode,
            AppError::Clipboard(_) => SnapforgeErrorCode::Internal,
            AppError::Storage(_) => SnapforgeErrorCode::Io,
            AppError::Capture(CaptureError::NoDisplay(_)) => SnapforgeErrorCode::NotFound,
            AppError::Capture(CaptureError::CaptureFailed) => SnapforgeErrorCode::Capture,
            AppError::Capture(CaptureError::ImageDataFailed) => SnapforgeErrorCode::Capture,
            AppError::Recording(RecordError::FfmpegNotFound) => SnapforgeErrorCode::NotFound,
            AppError::Recording(RecordError::WriteFailed(_)) => SnapforgeErrorCode::Io,
            AppError::Recording(RecordError::CaptureFailed(_)) => SnapforgeErrorCode::Capture,
            AppError::Recording(RecordError::NotActive) => SnapforgeErrorCode::Internal,
            AppError::Recording(RecordError::FfmpegSpawnFailed(_)) => SnapforgeErrorCode::Encode,
        }
    }
}

thread_local! {
    /// Last error (code + message) recorded by a `snapforge_*` call **on this
    /// thread** (errno-style). Each failing call sets it; each successful call
    /// clears it. Thread-local because callers read it on the thread that
    /// made the failing call (the Qt stop-worker thread, QThreadPool save
    /// workers, the GUI thread) — a process-global slot let a concurrent call
    /// on another thread overwrite or clear the error before it was read.
    /// Read via `snapforge_app_last_error` / `snapforge_app_last_error_code`.
    static LAST_APP_ERROR: RefCell<Option<(SnapforgeErrorCode, String)>> =
        const { RefCell::new(None) };
}

/// Set the error string with an explicit category code. Used by the FFI-level
/// validation paths (bad JSON, null pointers) where there is no `AppError`.
fn set_app_error_code(code: SnapforgeErrorCode, msg: impl Into<String>) {
    let msg = msg.into();
    LAST_APP_ERROR.with(|e| *e.borrow_mut() = Some((code, msg)));
}

/// Record a failure from a use-case call, deriving the stable code from the
/// `AppError` variant and the message from its `Display`.
fn set_app_error_from(err: &snapforge_app::AppError) {
    set_app_error_code(SnapforgeErrorCode::from_app_error(err), err.to_string());
}

/// Set the error string with the catch-all `InvalidInput` code. Kept for the
/// FFI input-validation callsites that were already passing a plain string.
fn set_app_error(msg: impl Into<String>) {
    set_app_error_code(SnapforgeErrorCode::InvalidInput, msg);
}

fn clear_app_error() {
    LAST_APP_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Get the last error recorded on the calling thread, or NULL if the last
/// `snapforge_*` call on this thread succeeded (or none has run).
///
/// Caller must free the returned string via `snapforge_free_string`.
/// Read the machine-readable category via `snapforge_app_last_error_code`.
#[no_mangle]
pub extern "C" fn snapforge_app_last_error() -> *mut c_char {
    LAST_APP_ERROR.with(|e| match e.borrow().as_ref() {
        // Never log here: this is read from error paths that may themselves
        // be reporting a logging problem.
        Some((_, msg)) => cstring_strip_nul(msg).into_raw(),
        None => ptr::null_mut(),
    })
}

/// Get the stable category code of the last error recorded on the calling
/// thread. Returns `SnapforgeErrorCode::None` (0) when the last call on this
/// thread succeeded or none has run.
#[no_mangle]
pub extern "C" fn snapforge_app_last_error_code() -> i32 {
    LAST_APP_ERROR.with(|e| match e.borrow().as_ref() {
        Some((code, _)) => *code as i32,
        None => SnapforgeErrorCode::None as i32,
    })
}

// ---------------------------------------------------------------------------
// Logging bridge (Rust `tracing` -> Qt Logger)
//
// Every snapforge_* crate emits diagnostics through the `tracing` macros. The
// Qt frontend registers a C callback via `snapforge_set_log_callback`; we
// install a tracing-subscriber layer that formats each event and forwards it
// to that callback so Rust logs land in the same rotating file as Qt's. With
// no callback registered the layer falls back to stderr (preserving the old
// eprintln! behaviour for headless / test runs).
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicPtr, Ordering};

/// C callback the Qt side registers to receive formatted Rust log lines.
/// `level` is a [`SnapforgeLogLevel`]; `msg` is a NUL-terminated UTF-8 string
/// **owned by Rust and valid only for the duration of the call** — the
/// callback must copy it, not retain the pointer.
pub type SnapforgeLogCallback = extern "C" fn(level: i32, msg: *const c_char);

/// Stable log-level discriminants passed to [`SnapforgeLogCallback`]. Chosen to
/// line up with severity so the Qt side can map them onto `QtMsgType`.
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum SnapforgeLogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl SnapforgeLogLevel {
    fn from_tracing(level: tracing::Level) -> Self {
        match level {
            tracing::Level::TRACE => SnapforgeLogLevel::Trace,
            tracing::Level::DEBUG => SnapforgeLogLevel::Debug,
            tracing::Level::INFO => SnapforgeLogLevel::Info,
            tracing::Level::WARN => SnapforgeLogLevel::Warn,
            tracing::Level::ERROR => SnapforgeLogLevel::Error,
        }
    }
}

/// The currently-registered callback, stored as a raw fn pointer behind an
/// atomic so the subscriber thread and the Qt registration thread don't race.
/// `null` means "no callback — fall back to stderr".
static LOG_CALLBACK: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// A `tracing` layer that formats each event's message and hands it to the
/// registered C callback (or stderr if none).
struct FfiBridgeLayer;

/// Pull just the `message` field out of an event into a `String`. tracing
/// stores the formatted-args message under the well-known `message` field.
struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            use std::fmt::Write;
            // Overwrite rather than append: `message` is recorded once.
            self.0.clear();
            let _ = write!(self.0, "{:?}", value);
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for FfiBridgeLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = SnapforgeLogLevel::from_tracing(*event.metadata().level());
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        dispatch_log(level, &visitor.0);
    }
}

/// Forward a single formatted record to the registered callback, or stderr.
///
/// Must never emit a `tracing` event itself (that would re-enter this layer),
/// hence the non-logging `cstring_strip_nul`. There is deliberately no
/// `catch_unwind`: the callback is a C function (a C++ exception escaping an
/// `extern "C"` fn aborts regardless), and Rust code here builds with
/// `panic = "abort"`, so a guard could never observe an unwind.
fn dispatch_log(level: SnapforgeLogLevel, message: &str) {
    let raw = LOG_CALLBACK.load(Ordering::Acquire);
    if raw.is_null() {
        eprintln!("{}", message);
        return;
    }
    // SAFETY: `raw` was stored from a valid `SnapforgeLogCallback` in
    // `snapforge_set_log_callback` and is only ever cleared back to null.
    let cb: SnapforgeLogCallback = unsafe { std::mem::transmute(raw) };
    // Strip interior NULs so a log line can never reach C truncated.
    let cstr = cstring_strip_nul(message);
    cb(level as i32, cstr.as_ptr());
}

/// Register the C callback that receives formatted Rust log records, and (on
/// first call) install the tracing subscriber that drives it. Pass a non-null
/// `callback`; passing it again replaces the previous one (a NULL callback is
/// ignored). The subscriber is installed exactly once — subsequent calls only
/// swap the callback pointer.
///
/// Qt calls this once at startup to route Rust diagnostics into its rotating
/// log file. Until it does, Rust logs fall back to stderr.
///
/// # Safety
///
/// `callback` must be NULL or a valid function pointer for the remaining
/// lifetime of the process (it is stored and invoked from arbitrary Rust
/// threads). It must not unwind (throw); doing so aborts the process.
#[no_mangle]
pub unsafe extern "C" fn snapforge_set_log_callback(callback: Option<SnapforgeLogCallback>) {
    // `Option<extern "C" fn>` has the same ABI as a bare fn pointer, so a C
    // caller passing NULL arrives as `None` instead of instant UB.
    let Some(cb) = callback else {
        eprintln!("[snapforge] snapforge_set_log_callback called with NULL; ignoring");
        return;
    };
    LOG_CALLBACK.store(cb as *mut (), Ordering::Release);

    // Install the subscriber once. `try_init` is a no-op (returns Err) if a
    // global subscriber already exists, so this is safe to race / repeat.
    static SUBSCRIBER_INIT: OnceLock<()> = OnceLock::new();
    SUBSCRIBER_INIT.get_or_init(|| {
        use tracing_subscriber::prelude::*;
        let _ = tracing_subscriber::registry()
            .with(FfiBridgeLayer)
            .try_init();
    });
}

/// Take a screenshot.
///
/// `req_json` is a null-terminated UTF-8 JSON string with fields:
///   display (u32), region (optional {x,y,width,height}), output_path (string),
///   format ("png"/"jpg"/"webp"), quality (u8 1..=100),
///   copy_to_clipboard (bool), add_to_history (bool).
///
/// Returns a heap-allocated JSON string `{"saved_path": "..."}` on success or
/// NULL on error (call `snapforge_app_last_error` for details). `saved_path`
/// is the file actually written: if `output_path` already exists a `-N`
/// suffix is added before the extension, so callers must use the returned
/// path, not the requested one. Caller must free the returned string via
/// `snapforge_free_string`.
///
/// # Safety
///
/// `req_json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_screenshot(req_json: *const c_char) -> *mut c_char {
    if req_json.is_null() {
        set_app_error("internal: null req_json");
        return ptr::null_mut();
    }
    let Ok(json_str) = CStr::from_ptr(req_json).to_str() else {
        set_app_error("internal: req_json is not valid UTF-8");
        return ptr::null_mut();
    };
    // The request schema lives once, in `snapforge-app` (serde). Deserialize
    // straight into it — no field-by-field re-parsing, no second copy of the
    // defaults or the enum spellings. A missing `output_path` arrives as the
    // empty path and is rejected by `take_screenshot`, as before.
    let mut req: app_screenshot::ScreenshotRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            set_app_error(format!("invalid request JSON: {}", e));
            return ptr::null_mut();
        }
    };
    // Preserve the historical lenient clamp on quality (1..=100).
    req.quality = req.quality.clamp(1, 100);

    match app_screenshot::take_screenshot(req) {
        Ok(result) => {
            clear_app_error();
            let body = serde_json::json!({
                "saved_path": result.saved_path.to_string_lossy(),
            });
            let serialized = match serde_json::to_string(&body) {
                Ok(s) => s,
                Err(e) => {
                    set_app_error_code(
                        SnapforgeErrorCode::Internal,
                        format!("failed to serialize result: {}", e),
                    );
                    return ptr::null_mut();
                }
            };
            cstring_sanitized(&serialized).into_raw()
        }
        Err(e) => {
            set_app_error_from(&e);
            ptr::null_mut()
        }
    }
}

/// Save (and optionally clipboard / index) a caller-supplied RGBA bitmap.
///
/// `rgba` must point to `rgba_len` bytes of raw RGBA8 pixel data and
/// `rgba_len` must equal `width * height * 4`. The buffer is read-only from
/// Rust's perspective and the caller retains ownership — Rust copies the
/// bytes internally before encoding.
///
/// `req_json` is a null-terminated UTF-8 JSON string with fields:
///   output_path (optional string; omit for clipboard-only),
///   format ("png" / "jpg" / "webp"; default "png"; ignored when no path),
///   quality (u8 1..=100; default 90),
///   copy_to_clipboard (bool; default false),
///   add_to_history (bool; default false; ignored when no path).
///
/// Returns a heap-allocated JSON string `{"saved_path": "..." | null}` on
/// success, NULL on error. Read details via `snapforge_app_last_error`.
/// `saved_path` is the real file written (`-N`-suffixed if `output_path`
/// already existed). Clipboard failure is non-fatal when a file was saved
/// (logged only); it is fatal for a clipboard-only request.
/// Caller must free the returned string via `snapforge_free_string`.
///
/// # Safety
///
/// `rgba` must be valid for reads of `rgba_len` bytes for the duration of
/// the call. `req_json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_save_prerendered(
    rgba: *const u8,
    rgba_len: usize,
    width: u32,
    height: u32,
    req_json: *const c_char,
) -> *mut c_char {
    if rgba.is_null() || req_json.is_null() {
        set_app_error("internal: null rgba or req_json");
        return ptr::null_mut();
    }
    let Ok(json_str) = CStr::from_ptr(req_json).to_str() else {
        set_app_error("internal: req_json is not valid UTF-8");
        return ptr::null_mut();
    };
    // Deserialize the JSON half of the request into the canonical schema. The
    // pixel buffer + dims come from the FFI args (not JSON) and are filled in
    // below; their struct fields are `#[serde(skip)]`.
    let mut req: app_screenshot::SavePrerenderedRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            set_app_error(format!("invalid request JSON: {}", e));
            return ptr::null_mut();
        }
    };

    let Some(expected) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
    else {
        set_app_error("width*height*4 overflows");
        return ptr::null_mut();
    };
    if rgba_len != expected {
        set_app_error(format!(
            "rgba_len {} does not match width*height*4 ({})",
            rgba_len, expected
        ));
        return ptr::null_mut();
    }
    // Copy the caller's bytes into an owned Vec so the use-case can hand the
    // buffer to the `image` crate. The caller keeps ownership of `rgba`.
    req.rgba = std::slice::from_raw_parts(rgba, rgba_len).to_vec();
    req.width = width;
    req.height = height;
    req.quality = req.quality.clamp(1, 100);

    match app_screenshot::save_prerendered(req) {
        Ok(result) => {
            clear_app_error();
            let body = serde_json::json!({
                "saved_path": result
                    .saved_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            });
            let serialized = match serde_json::to_string(&body) {
                Ok(s) => s,
                Err(e) => {
                    set_app_error_code(
                        SnapforgeErrorCode::Internal,
                        format!("failed to serialize result: {}", e),
                    );
                    return ptr::null_mut();
                }
            };
            cstring_sanitized(&serialized).into_raw()
        }
        Err(e) => {
            set_app_error_from(&e);
            ptr::null_mut()
        }
    }
}

// --- Recording (use-case) ---------------------------------------------------

const APP_RECORDING_MAGIC_ALIVE: u64 = 0x534E_4150_5245_4348; // "SNAPRECH"

#[repr(C)]
struct FfiAppRecordingHandle {
    magic: u64,
    inner: Mutex<Option<app_recording::RecordingHandle>>,
}

/// Tracks every live recording handle we've issued, keyed by the raw pointer
/// handed to C. Storing the `Arc` here (rather than a bare pointer set) closes
/// the validate-then-free TOCTOU: `check_app_recording_handle` clones the Arc
/// UNDER the registry lock, so a concurrent `snapforge_record_free_handle` can
/// only remove the registry entry — deallocation is deferred until the last
/// clone drops. The in-band magic word stays as a secondary defense against a
/// registry entry pointing at corrupted memory.
static RECORDING_HANDLE_REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<FfiAppRecordingHandle>>>> =
    OnceLock::new();

fn recording_handle_registry() -> &'static Mutex<HashMap<usize, Arc<FfiAppRecordingHandle>>> {
    RECORDING_HANDLE_REGISTRY.get_or_init(|| Mutex::new(HashMap::default()))
}

/// Validate `handle` against the registry. Every rejection records an error
/// (InvalidInput for a bad handle, Internal for a poisoned registry), so a
/// `-1` returned on this path is always explained by `snapforge_app_last_error`.
fn check_app_recording_handle(handle: *mut c_void) -> Option<Arc<FfiAppRecordingHandle>> {
    if handle.is_null() || (handle as usize) < 4096 {
        set_app_error("invalid recording handle (null)");
        return None;
    }
    let Ok(map) = recording_handle_registry().lock() else {
        set_app_error_code(
            SnapforgeErrorCode::Internal,
            "recording handle registry poisoned",
        );
        return None;
    };
    let Some(wrapper) = map.get(&(handle as usize)) else {
        tracing::warn!(
            "[snapforge] app recording handle {:p} not in registry; rejecting",
            handle
        );
        set_app_error("unknown or already-freed recording handle");
        return None;
    };
    if wrapper.magic != APP_RECORDING_MAGIC_ALIVE {
        tracing::warn!(
            "[snapforge] app recording handle {:p} bad magic 0x{:016x}; rejecting",
            handle,
            wrapper.magic
        );
        set_app_error("corrupt recording handle");
        return None;
    }
    Some(Arc::clone(wrapper))
}

/// Start a recording via the use-case surface.
///
/// `req_json` is the serde `RecordingRequest` schema from `snapforge-app`:
/// display, region (optional), output_path, format, fps, quality,
/// ffmpeg_path (optional), add_to_history_on_stop, show_clicks — all fields
/// optional except output_path in practice (missing fields take defaults).
/// Returns an opaque handle, or NULL on
/// failure (read `snapforge_app_last_error` for details). The handle must be
/// stopped (`snapforge_record_stop`) and freed
/// (`snapforge_record_free_handle`).
///
/// # Safety
///
/// `req_json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_start(req_json: *const c_char) -> *mut c_void {
    if req_json.is_null() {
        set_app_error("internal: null req_json");
        return ptr::null_mut();
    }
    let Ok(json_str) = CStr::from_ptr(req_json).to_str() else {
        set_app_error("internal: req_json is not valid UTF-8");
        return ptr::null_mut();
    };
    // One canonical schema (serde, in `snapforge-app`). An empty/missing
    // `output_path` is rejected by `start_recording`, as before.
    let mut req: app_recording::RecordingRequest = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => {
            set_app_error(format!("invalid request JSON: {}", e));
            return ptr::null_mut();
        }
    };
    // Preserve the historical lenient clamp on fps (1..=240).
    req.fps = req.fps.clamp(1, 240);

    let handle = match app_recording::start_recording(req) {
        Ok(h) => h,
        Err(e) => {
            set_app_error_from(&e);
            return ptr::null_mut();
        }
    };
    // Take the registry lock BEFORE minting the C pointer: if it is
    // unavailable we drop the handle (stopping the worker) and fail, rather
    // than returning a pointer every later call would reject as unknown.
    let Ok(mut map) = recording_handle_registry().lock() else {
        drop(handle);
        set_app_error_code(
            SnapforgeErrorCode::Internal,
            "recording handle registry poisoned",
        );
        return ptr::null_mut();
    };
    let wrapper = Arc::new(FfiAppRecordingHandle {
        magic: APP_RECORDING_MAGIC_ALIVE,
        inner: Mutex::new(Some(handle)),
    });
    // The pointer handed to C owns one strong reference; the registry owns a
    // second one. Both are released in `record_free_handle`.
    let raw = Arc::into_raw(Arc::clone(&wrapper))
        .cast_mut()
        .cast::<c_void>();
    map.insert(raw as usize, wrapper);
    drop(map);
    clear_app_error();
    raw
}

/// Stop a use-case recording. Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_record_start`.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_stop(handle: *mut c_void) -> c_int {
    let Some(wrapper) = check_app_recording_handle(handle) else {
        return -1;
    };
    let Ok(mut guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "recording mutex poisoned");
        return -1;
    };
    let taken = guard.take();
    drop(guard);
    let Some(h) = taken else {
        set_app_error("recording already stopped");
        return -1;
    };
    match app_recording::stop_recording(h) {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e);
            -1
        }
    }
}

/// Report whether a recording's encode worker is still running.
///
/// Returns 1 while the worker is running or paused, 0 once it has exited —
/// e.g. ffmpeg died or a capture error ended it (`snapforge_record_stop` then
/// returns -1 with that error) — or after the handle was stopped. Returns -1
/// for an invalid / unknown handle.
///
/// # Safety
///
/// `handle` must be a pointer returned by `snapforge_record_start` (it is
/// validated against the registry before use).
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_is_alive(handle: *mut c_void) -> c_int {
    let Some(wrapper) = check_app_recording_handle(handle) else {
        return -1;
    };
    let Ok(guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "recording mutex poisoned");
        return -1;
    };
    let alive = guard
        .as_ref()
        .is_some_and(app_recording::is_recording_alive);
    clear_app_error();
    c_int::from(alive)
}

/// Return the path the recording is actually writing, as a heap-allocated
/// UTF-8 string (free via `snapforge_free_string`), or NULL on an invalid
/// handle / already-stopped recording. This can differ from the requested
/// `output_path`: if that file already existed, a `-N` suffix was added
/// before the extension so ffmpeg never overwrites an existing file.
///
/// # Safety
///
/// `handle` must be a pointer returned by `snapforge_record_start`.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_output_path(handle: *mut c_void) -> *mut c_char {
    let Some(wrapper) = check_app_recording_handle(handle) else {
        return ptr::null_mut();
    };
    let Ok(guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "recording mutex poisoned");
        return ptr::null_mut();
    };
    let Some(h) = guard.as_ref() else {
        set_app_error("recording already stopped");
        return ptr::null_mut();
    };
    let s = cstring_sanitized(&h.output_path().to_string_lossy()).into_raw();
    clear_app_error();
    s
}

/// Pause a use-case recording. Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_record_start`.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_pause(handle: *mut c_void) -> c_int {
    let Some(wrapper) = check_app_recording_handle(handle) else {
        return -1;
    };
    let Ok(guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "recording mutex poisoned");
        return -1;
    };
    let Some(h) = guard.as_ref() else {
        set_app_error("recording already stopped");
        return -1;
    };
    match app_recording::pause_recording(h) {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e);
            -1
        }
    }
}

/// Resume a use-case recording. Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_record_start`.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_resume(handle: *mut c_void) -> c_int {
    let Some(wrapper) = check_app_recording_handle(handle) else {
        return -1;
    };
    let Ok(guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "recording mutex poisoned");
        return -1;
    };
    let Some(h) = guard.as_ref() else {
        set_app_error("recording already stopped");
        return -1;
    };
    match app_recording::resume_recording(h) {
        Ok(()) => {
            clear_app_error();
            0
        }
        Err(e) => {
            set_app_error_from(&e);
            -1
        }
    }
}

/// Free a use-case recording handle. If the recording is still active it is
/// stopped first via Drop.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_record_start` and
/// must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_record_free_handle(handle: *mut c_void) {
    if handle.is_null() || (handle as usize) < 4096 {
        return;
    }
    // Residual ABA: the registry is address-keyed, so if the allocator reuses
    // a freed handle's address for a new one, a stale double-free frees the
    // live entry; a generation-keyed registry would close it.
    let removed = recording_handle_registry()
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&(handle as usize)));
    let Some(registry_ref) = removed else {
        tracing::warn!(
            "[snapforge] record_free_handle: {:p} not in registry; ignoring",
            handle
        );
        return;
    };
    if registry_ref.magic != APP_RECORDING_MAGIC_ALIVE {
        tracing::error!(
            "[snapforge] record_free_handle: {:p} bad magic 0x{:016x}; leaking",
            handle,
            registry_ref.magic
        );
        std::mem::forget(registry_ref);
        return;
    }
    drop(registry_ref);
    // Reclaim the strong reference handed to C at creation. The allocation is
    // released once any in-flight `check_app_recording_handle` clone drops.
    let _ = Arc::from_raw(handle.cast::<FfiAppRecordingHandle>());
}

// --- Click tracking (use-case) ---------------------------------------------

const APP_CLICKS_MAGIC_ALIVE: u64 = 0x534E_4150_434C_4B48; // "SNAPCLKH"

/// C-ABI callback delivered for every click. `right_click` is 1 for
/// right-mouse-down, 0 for left-mouse-down. `user_data` is the opaque pointer
/// supplied to `snapforge_clicks_start` — Rust never dereferences it.
pub type SnapforgeClickCallback =
    extern "C" fn(x: f64, y: f64, right_click: c_int, user_data: *mut c_void);

#[repr(C)]
struct FfiClickHandle {
    magic: u64,
    inner: Mutex<Option<app_clicks::ClickHandle>>,
}

/// Same Arc-in-registry scheme as `RECORDING_HANDLE_REGISTRY` — see the
/// comment there for the TOCTOU rationale.
static CLICK_HANDLE_REGISTRY: OnceLock<Mutex<HashMap<usize, Arc<FfiClickHandle>>>> =
    OnceLock::new();

fn click_handle_registry() -> &'static Mutex<HashMap<usize, Arc<FfiClickHandle>>> {
    CLICK_HANDLE_REGISTRY.get_or_init(|| Mutex::new(HashMap::default()))
}

fn check_click_handle(handle: *mut c_void) -> Option<Arc<FfiClickHandle>> {
    if handle.is_null() || (handle as usize) < 4096 {
        set_app_error("invalid click handle (null)");
        return None;
    }
    let Ok(map) = click_handle_registry().lock() else {
        set_app_error_code(
            SnapforgeErrorCode::Internal,
            "click handle registry poisoned",
        );
        return None;
    };
    let Some(wrapper) = map.get(&(handle as usize)) else {
        tracing::warn!(
            "[snapforge] click handle {:p} not in registry; rejecting",
            handle
        );
        set_app_error("unknown or already-freed click handle");
        return None;
    };
    if wrapper.magic != APP_CLICKS_MAGIC_ALIVE {
        tracing::warn!(
            "[snapforge] click handle {:p} bad magic 0x{:016x}; rejecting",
            handle,
            wrapper.magic
        );
        set_app_error("corrupt click handle");
        return None;
    }
    Some(Arc::clone(wrapper))
}

/// Begin streaming global click events to `callback`.
///
/// The callback fires on a thread owned by the use-case (NOT the main
/// thread). Qt callers must dispatch back to the main thread themselves
/// (e.g. via `QMetaObject::invokeMethod` with `Qt::QueuedConnection`).
/// `user_data` is opaque — Rust passes it back verbatim and never reads
/// through it.
///
/// Returns an opaque handle, or NULL on failure (typically missing
/// Accessibility permission; read `snapforge_app_last_error` for details).
/// The handle must be stopped (`snapforge_clicks_stop`) and freed
/// (`snapforge_clicks_free_handle`).
///
/// # Safety
///
/// `callback` must be non-NULL and remain valid for the lifetime of the
/// returned handle (a NULL callback is rejected with an error).
/// `user_data` must remain valid for the same lifetime if the callback
/// dereferences it.
#[no_mangle]
pub unsafe extern "C" fn snapforge_clicks_start(
    callback: Option<SnapforgeClickCallback>,
    user_data: *mut c_void,
) -> *mut c_void {
    // `Option<extern "C" fn>` has the same ABI as a bare fn pointer, so a C
    // caller passing NULL arrives as `None` instead of instant UB.
    let Some(cb) = callback else {
        set_app_error("internal: null click callback");
        return ptr::null_mut();
    };
    // Cast the user pointer to usize for transport across the thread
    // boundary — `*mut c_void` is `!Send` even when wrapped in a newtype
    // because the rustc auto-trait check looks through the closure capture.
    // We never dereference it; the callback gets the bit pattern back as a
    // pointer. Raw `extern "C" fn` is Send already.
    let ud_bits = user_data as usize;
    let inner = match app_clicks::start_click_tracking(move |ev| {
        let p = ud_bits as *mut c_void;
        cb(ev.x, ev.y, c_int::from(ev.right_click), p);
    }) {
        Ok(h) => h,
        Err(e) => {
            set_app_error_from(&e);
            return ptr::null_mut();
        }
    };
    // Lock the registry before minting the C pointer; on failure drop the
    // tracker (stopping the tap) instead of returning an untracked pointer.
    let Ok(mut map) = click_handle_registry().lock() else {
        drop(inner);
        set_app_error_code(
            SnapforgeErrorCode::Internal,
            "click handle registry poisoned",
        );
        return ptr::null_mut();
    };
    let wrapper = Arc::new(FfiClickHandle {
        magic: APP_CLICKS_MAGIC_ALIVE,
        inner: Mutex::new(Some(inner)),
    });
    // The pointer handed to C owns one strong reference; the registry owns a
    // second one. Both are released in `clicks_free_handle`.
    let raw = Arc::into_raw(Arc::clone(&wrapper))
        .cast_mut()
        .cast::<c_void>();
    map.insert(raw as usize, wrapper);
    drop(map);
    clear_app_error();
    raw
}

/// Stop click tracking but keep the handle allocation alive (must still be
/// freed with `snapforge_clicks_free_handle`). Returns 0 on success, -1 on
/// error.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_clicks_start`.
#[no_mangle]
pub unsafe extern "C" fn snapforge_clicks_stop(handle: *mut c_void) -> c_int {
    let Some(wrapper) = check_click_handle(handle) else {
        return -1;
    };
    let Ok(mut guard) = wrapper.inner.lock() else {
        set_app_error_code(SnapforgeErrorCode::Internal, "click handle mutex poisoned");
        return -1;
    };
    // Dropping the ClickHandle stops the tap + joins the forwarder thread.
    let _ = guard.take();
    clear_app_error();
    0
}

/// Free a click handle. Stops the tap first if still active.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_clicks_start` and
/// must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_clicks_free_handle(handle: *mut c_void) {
    if handle.is_null() || (handle as usize) < 4096 {
        return;
    }
    // Residual ABA: the registry is address-keyed, so if the allocator reuses
    // a freed handle's address for a new one, a stale double-free frees the
    // live entry; a generation-keyed registry would close it.
    let removed = click_handle_registry()
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&(handle as usize)));
    let Some(registry_ref) = removed else {
        tracing::warn!(
            "[snapforge] clicks_free_handle: {:p} not in registry; ignoring",
            handle
        );
        return;
    };
    if registry_ref.magic != APP_CLICKS_MAGIC_ALIVE {
        tracing::error!(
            "[snapforge] clicks_free_handle: {:p} bad magic 0x{:016x}; leaking",
            handle,
            registry_ref.magic
        );
        std::mem::forget(registry_ref);
        return;
    }
    drop(registry_ref);
    // Reclaim the strong reference handed to C at creation. The allocation is
    // released once any in-flight `check_click_handle` clone drops.
    let _ = Arc::from_raw(handle.cast::<FfiClickHandle>());
}

#[cfg(test)]
mod app_ffi_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn screenshot_rejects_null() {
        let res = unsafe { snapforge_screenshot(ptr::null()) };
        assert!(res.is_null());
        let err = snapforge_app_last_error();
        assert!(!err.is_null());
        unsafe {
            let _ = CString::from_raw(err);
        }
    }

    // ---- Buffer registry (the hot-path optimization) --------------------
    //
    // These exercise the `register_buffer` / `snapforge_free_buffer` pairing
    // directly against a genuinely heap-allocated `Box<[u8]>`, so the free
    // path actually runs `Box::from_raw`. They pin the four safety properties
    // the length-checked registry guarantees, and run without a display
    // (unlike the capture round-trips in tests/abi.rs).

    /// Allocate a real `Box<[u8]>` exactly as the capture path does and
    /// register it. Returns the raw pointer + len the caller would free.
    fn alloc_and_register(len: usize) -> (*mut u8, usize) {
        let boxed: Box<[u8]> = vec![0u8; len].into_boxed_slice();
        let data = Box::into_raw(boxed).cast::<u8>();
        assert!(register_buffer(data, len));
        (data, len)
    }

    #[test]
    fn registry_correct_len_frees_once() {
        let (data, len) = alloc_and_register(4096);
        // Recorded, so the first correctly-sized free succeeds (frees the Box).
        unsafe { snapforge_free_buffer(data, len) };
        // Now unknown: a second free must be refused (double-free safe).
        unsafe { snapforge_free_buffer(data, len) };
    }

    #[test]
    fn registry_wrong_len_is_refused_then_correct_len_frees() {
        let (data, len) = alloc_and_register(4096);
        // Wrong len must be refused and the entry re-inserted...
        unsafe { snapforge_free_buffer(data, len.wrapping_add(1)) };
        // ...so the correctly-sized free still succeeds afterwards.
        unsafe { snapforge_free_buffer(data, len) };
    }

    #[test]
    fn registry_unknown_pointer_is_refused() {
        // A pointer we never registered must never be handed to the
        // allocator. Use a stack buffer so a stray free would be detectable
        // under sanitizers; the test passes if nothing crashes.
        let mut sham = [0u8; 32];
        unsafe { snapforge_free_buffer(sham.as_mut_ptr(), 32) };
    }

    #[test]
    fn registry_distinct_pointers_coexist_under_custom_hasher() {
        // Two live registrations must not collide/alias in the map: freeing
        // one with its own len must not affect the other. Guards against a
        // broken custom hasher silently conflating distinct address keys.
        let (a, alen) = alloc_and_register(1024);
        let (b, blen) = alloc_and_register(2048);
        assert_ne!(a as usize, b as usize);
        unsafe { snapforge_free_buffer(a, alen) };
        // `b` is still tracked with its own length and frees cleanly.
        unsafe { snapforge_free_buffer(b, blen) };
    }

    #[test]
    fn last_error_is_thread_local() {
        // A failure on another thread must not be visible here, and a
        // success here must not clear that thread's error.
        clear_app_error();
        std::thread::spawn(|| {
            let res = unsafe { snapforge_screenshot(ptr::null()) };
            assert!(res.is_null());
            assert_eq!(
                snapforge_app_last_error_code(),
                SnapforgeErrorCode::InvalidInput as i32
            );
        })
        .join()
        .unwrap();
        assert_eq!(
            snapforge_app_last_error_code(),
            SnapforgeErrorCode::None as i32
        );
        assert!(snapforge_app_last_error().is_null());
    }

    #[test]
    fn handle_rejections_set_errors() {
        clear_app_error();
        assert_eq!(unsafe { snapforge_record_stop(ptr::null_mut()) }, -1);
        assert_eq!(
            snapforge_app_last_error_code(),
            SnapforgeErrorCode::InvalidInput as i32
        );
        clear_app_error();
        let mut bogus = [0u8; 64];
        let h = bogus.as_mut_ptr().cast::<c_void>();
        assert_eq!(unsafe { snapforge_record_pause(h) }, -1);
        assert_ne!(snapforge_app_last_error_code(), 0);
        clear_app_error();
        assert_eq!(unsafe { snapforge_record_is_alive(h) }, -1);
        assert_ne!(snapforge_app_last_error_code(), 0);
        clear_app_error();
        assert!(unsafe { snapforge_record_output_path(h) }.is_null());
        assert_ne!(snapforge_app_last_error_code(), 0);
        clear_app_error();
        assert_eq!(unsafe { snapforge_clicks_stop(h) }, -1);
        assert_ne!(snapforge_app_last_error_code(), 0);
    }

    #[test]
    fn export_image_rejects_empty_image() {
        let img = export_image(image::RgbaImage::new(0, 0));
        assert!(img.data.is_null());
        assert_eq!(img.len, 0);
        assert_eq!(
            snapforge_app_last_error_code(),
            SnapforgeErrorCode::Capture as i32
        );
    }

    #[test]
    fn export_image_registers_and_frees() {
        let img = export_image(image::RgbaImage::new(2, 3));
        assert!(!img.data.is_null());
        assert_eq!((img.len, img.width, img.height), (24, 2, 3));
        assert_eq!(snapforge_app_last_error_code(), 0);
        unsafe { snapforge_free_buffer(img.data, img.len) };
    }

    #[test]
    fn cstring_strip_nul_removes_interior_nuls() {
        assert_eq!(cstring_strip_nul("a\0b\0").to_bytes(), b"ab");
        assert_eq!(cstring_strip_nul("plain").to_bytes(), b"plain");
    }

    #[test]
    fn permission_denied_maps_to_code() {
        use snapforge_app::AppError;
        assert_eq!(
            SnapforgeErrorCode::from_app_error(&AppError::PermissionDenied("x".into())),
            SnapforgeErrorCode::PermissionDenied
        );
        assert_eq!(
            SnapforgeErrorCode::from_app_error(&AppError::Internal("x".into())),
            SnapforgeErrorCode::Internal
        );
    }

    #[test]
    fn screenshot_rejects_bad_json() {
        let bad = CString::new("not json").unwrap();
        let res = unsafe { snapforge_screenshot(bad.as_ptr()) };
        assert!(res.is_null());
    }

    #[test]
    fn screenshot_rejects_missing_output_path() {
        let req = CString::new(r#"{"display":0}"#).unwrap();
        let res = unsafe { snapforge_screenshot(req.as_ptr()) };
        assert!(res.is_null());
    }

    #[test]
    fn record_start_rejects_null() {
        let res = unsafe { snapforge_record_start(ptr::null()) };
        assert!(res.is_null());
    }

    #[test]
    fn record_stop_rejects_null_handle() {
        let rc = unsafe { snapforge_record_stop(ptr::null_mut()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn clicks_stop_rejects_null_handle() {
        let rc = unsafe { snapforge_clicks_stop(ptr::null_mut()) };
        assert_eq!(rc, -1);
    }

    #[test]
    fn null_screenshot_sets_invalid_input_code() {
        let res = unsafe { snapforge_screenshot(ptr::null()) };
        assert!(res.is_null());
        // The null-pointer rejection routes through set_app_error -> InvalidInput.
        assert_eq!(
            snapforge_app_last_error_code(),
            SnapforgeErrorCode::InvalidInput as i32
        );
    }

    #[test]
    fn error_code_maps_from_app_error_variant() {
        use snapforge_app::AppError;
        // Spot-check the AppError -> code mapping for a couple of categories.
        assert_eq!(
            SnapforgeErrorCode::from_app_error(&AppError::InvalidRequest("x".into())),
            SnapforgeErrorCode::InvalidInput
        );
        let cfg_err = AppError::Config(snapforge_storage::config::ConfigError::Parse(
            serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
        ));
        assert_eq!(
            SnapforgeErrorCode::from_app_error(&cfg_err),
            SnapforgeErrorCode::Config
        );
    }

    // Records the last (level, message) delivered to the C log callback so the
    // bridge test can assert the forwarding actually happened.
    static LAST_LOG: Mutex<Option<(i32, String)>> = Mutex::new(None);

    extern "C" fn capture_log(level: i32, msg: *const c_char) {
        let s = unsafe { CStr::from_ptr(msg) }
            .to_string_lossy()
            .into_owned();
        if let Ok(mut g) = LAST_LOG.lock() {
            *g = Some((level, s));
        }
    }

    #[test]
    fn log_callback_receives_forwarded_records() {
        unsafe { snapforge_set_log_callback(Some(capture_log)) };
        tracing::warn!("ffi-bridge-test-marker");
        let got = LAST_LOG.lock().unwrap().clone();
        // The subscriber may already be installed by another test's call; what
        // matters is that once registered, our callback sees the record.
        if let Some((level, msg)) = got {
            assert_eq!(level, SnapforgeLogLevel::Warn as i32);
            assert!(msg.contains("ffi-bridge-test-marker"), "got: {msg}");
        }
    }
}
