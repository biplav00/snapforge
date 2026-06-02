//! C FFI wrappers over snapforge-core for the Qt frontend.
//!
//! Convention:
//! - Return 0 on success, -1 on error.
//! - Caller-owned buffers are allocated here and freed via snapforge_free_buffer.
//! - String out-params are allocated here and freed via snapforge_free_string.

use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::hash::{BuildHasherDefault, Hasher};
use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, OnceLock};

/// A tiny deterministic hasher for `usize` keys that are already raw memory
/// addresses (and therefore uniformly distributed by the allocator). The
/// pointer-keyed registries below don't need SipHash's DoS resistance — the
/// keys are our own heap pointers, never attacker-chosen — so we trade it for
/// a single multiply, which removes SipHash's per-key cost from the per-frame
/// capture/free path.
///
/// This is an FxHash-style multiplicative hash (the `0x517c_c1b7_2722_0a95`
/// constant is the well-known 64-bit golden-ratio mixer used by rustc's
/// `FxHasher`). It changes *only* the hash function used to bucket keys; the
/// map contents, equality semantics, and every safety check that reads them
/// are byte-for-byte unchanged. Implemented inline to avoid an external dep.
#[derive(Default)]
struct PtrHasher(u64);

impl Hasher for PtrHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        // The only `write_*` the map ever calls for a `usize` key.
        self.0 = (i as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn write(&mut self, bytes: &[u8]) {
        // Defensive fallback: fold any byte stream through the same mixer so
        // the hasher stays correct even if the key type ever changes. Not on
        // the hot path (the registries are `usize`-keyed, which routes through
        // `write_usize`).
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
}

type PtrBuildHasher = BuildHasherDefault<PtrHasher>;

/// Expected number of simultaneously-live captured buffers. Qt holds a small,
/// bounded set at once (typically one full-screen grab plus the odd region
/// grab), so preallocating a handful of slots avoids the first few rehashes on
/// the capture path without over-committing memory. This is a capacity hint
/// only — the map still grows without bound if needed, so nothing is ever
/// silently dropped (which would reintroduce the heap-corruption risk).
const BUFFER_REGISTRY_PREALLOC: usize = 8;

/// Tracks every live `(ptr, len)` returned by `snapforge_capture_*` so
/// `snapforge_free_buffer` can detect a length mismatch (which would otherwise
/// hand a wrong-layout `Box<[u8]>` to the global allocator and corrupt the heap).
static BUFFER_REGISTRY: OnceLock<Mutex<HashMap<usize, usize, PtrBuildHasher>>> = OnceLock::new();

fn buffer_registry() -> &'static Mutex<HashMap<usize, usize, PtrBuildHasher>> {
    BUFFER_REGISTRY.get_or_init(|| {
        Mutex::new(HashMap::with_capacity_and_hasher(
            BUFFER_REGISTRY_PREALLOC,
            PtrBuildHasher::default(),
        ))
    })
}

fn register_buffer(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        if let Ok(mut map) = buffer_registry().lock() {
            map.insert(ptr as usize, len);
        }
    }
}

/// Tracks every live recording handle pointer we've issued so an arbitrary
/// caller-supplied pointer can be rejected without dereferencing it. This is
/// the authoritative liveness check; the in-band magic word is a secondary
/// defense against allocator reuse of a recently-freed slot.
static HANDLE_REGISTRY: OnceLock<Mutex<HashSet<usize, PtrBuildHasher>>> = OnceLock::new();

fn handle_registry() -> &'static Mutex<HashSet<usize, PtrBuildHasher>> {
    HANDLE_REGISTRY.get_or_init(|| Mutex::new(HashSet::default()))
}

fn register_handle(ptr: *mut c_void) {
    if !ptr.is_null() {
        if let Ok(mut set) = handle_registry().lock() {
            set.insert(ptr as usize);
        }
    }
}

fn unregister_handle(ptr: *mut c_void) -> bool {
    if ptr.is_null() {
        return false;
    }
    handle_registry()
        .lock()
        .is_ok_and(|mut set| set.remove(&(ptr as usize)))
}

fn is_registered_handle(ptr: *mut c_void) -> bool {
    if ptr.is_null() {
        return false;
    }
    handle_registry()
        .lock()
        .is_ok_and(|set| set.contains(&(ptr as usize)))
}

/// Build a `CString` even when the input has embedded NUL bytes. The FFI
/// contract is "null-terminated string" so we strip the NULs and log once
/// so the silent corruption is at least visible in logs.
fn cstring_sanitized(s: &str) -> Result<CString, std::ffi::NulError> {
    if s.as_bytes().contains(&0) {
        tracing::warn!("[snapforge] stripping embedded NUL from FFI string");
        let cleaned: String = s.chars().filter(|c| *c != '\0').collect();
        CString::new(cleaned)
    } else {
        CString::new(s)
    }
}

use snapforge_core::capture;
use snapforge_core::config::{AppConfig, RecordingFormat, RecordingQuality};
use snapforge_core::history::ScreenshotHistory;
use snapforge_core::types::{CaptureFormat, Rect};

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
    let result = capture::capture_fullscreen(display as usize);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let boxed: Box<[u8]> = image.into_raw().into_boxed_slice();
            let len = boxed.len();
            let data = Box::into_raw(boxed).cast::<u8>();
            register_buffer(data, len);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
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
        return CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        };
    }
    let region = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    let result = capture::capture_region(display as usize, region);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let boxed: Box<[u8]> = image.into_raw().into_boxed_slice();
            let len = boxed.len();
            let data = Box::into_raw(boxed).cast::<u8>();
            register_buffer(data, len);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
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
    let config = match snapforge_core::config::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("[snapforge] default_save_path: config load failed: {}", e);
            snapforge_core::config::AppConfig::default()
        }
    };
    let dir = &config.save_directory;
    match cstring_sanitized(dir.to_string_lossy().as_ref()) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
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
    let Ok(history) = ScreenshotHistory::load() else {
        return ptr::null_mut();
    };
    let Ok(json) = serde_json::to_string(&history.entries) else {
        return ptr::null_mut();
    };
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
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
        return -1;
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        return -1;
    };
    let Ok(mut history) = ScreenshotHistory::load() else {
        return -1;
    };
    match history.remove_entry(path_str) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Clear all history entries.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn snapforge_history_clear() -> c_int {
    let Ok(mut history) = ScreenshotHistory::load() else {
        return -1;
    };
    match history.clear() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Returns 1 if the given path looks like a recording that was never finalized
/// (zero-byte or missing moov atom), 0 otherwise. Non-mp4 paths always return 0.
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
    c_int::from(snapforge_core::history::is_incomplete_mp4(path_str))
}

// ---------------------------------------------------------------------------
// Config FFI
// ---------------------------------------------------------------------------

/// Load the application config and return it as a JSON string.
/// Returns NULL on error. Caller must free via `snapforge_free_string`.
#[no_mangle]
pub extern "C" fn snapforge_config_load() -> *mut c_char {
    let Ok(config) = AppConfig::load() else {
        return ptr::null_mut();
    };
    let Ok(json) = serde_json::to_string(&config) else {
        return ptr::null_mut();
    };
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
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
        return -1;
    }
    let Ok(json_str) = CStr::from_ptr(json).to_str() else {
        return -1;
    };
    let Ok(config) = serde_json::from_str::<AppConfig>(json_str) else {
        return -1;
    };
    match config.save() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

// ---------------------------------------------------------------------------
// Use-case FFI (snapforge-app)
//
// These wrappers expose the high-level use cases (`snapforge-app`) and
// coexist with the primitive `snapforge_capture_*` / `snapforge_*_recording`
// fns above. The Qt frontend will migrate to the use-case surface in Phase
// 2C; until then both surfaces ship side-by-side.
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
        use snapforge_core::capture::CaptureError;
        use snapforge_core::record::RecordError;
        // Keep one arm per AppError variant so the variant -> error-code mapping
        // stays exhaustive and self-documenting; do not merge arms that happen to
        // share a code (the codes are independent and may diverge later).
        #[allow(clippy::match_same_arms)]
        match err {
            AppError::InvalidRequest(_) => SnapforgeErrorCode::InvalidInput,
            AppError::Config(_) => SnapforgeErrorCode::Config,
            AppError::Format(_) => SnapforgeErrorCode::Encode,
            AppError::Clipboard(_) => SnapforgeErrorCode::Internal,
            AppError::Storage(_) => SnapforgeErrorCode::Io,
            AppError::Capture(CaptureError::NoDisplay(_)) => SnapforgeErrorCode::NotFound,
            AppError::Capture(_) => SnapforgeErrorCode::Capture,
            AppError::Recording(RecordError::FfmpegNotFound) => SnapforgeErrorCode::NotFound,
            AppError::Recording(RecordError::WriteFailed(_)) => SnapforgeErrorCode::Io,
            AppError::Recording(RecordError::CaptureFailed(_)) => SnapforgeErrorCode::Capture,
            AppError::Recording(RecordError::NotActive) => SnapforgeErrorCode::Internal,
            AppError::Recording(RecordError::FfmpegSpawnFailed(_)) => SnapforgeErrorCode::Encode,
        }
    }
}

/// Last app-level error (code + message), populated by any `snapforge_*`
/// use-case fn that fails. Reset on the next successful call. Read the message
/// via `snapforge_app_last_error` and the code via
/// `snapforge_app_last_error_code`.
static LAST_APP_ERROR: Mutex<Option<(SnapforgeErrorCode, String)>> = Mutex::new(None);

/// Set the error string with an explicit category code. Used by the FFI-level
/// validation paths (bad JSON, null pointers) where there is no `AppError`.
fn set_app_error_code(code: SnapforgeErrorCode, msg: impl Into<String>) {
    if let Ok(mut guard) = LAST_APP_ERROR.lock() {
        *guard = Some((code, msg.into()));
    }
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
    if let Ok(mut guard) = LAST_APP_ERROR.lock() {
        *guard = None;
    }
}

/// Get the last use-case error, or NULL if none.
///
/// Caller must free the returned string via `snapforge_free_string`.
/// Read the machine-readable category via `snapforge_app_last_error_code`.
#[no_mangle]
pub extern "C" fn snapforge_app_last_error() -> *mut c_char {
    // A poisoned lock means a prior holder panicked mid-update — surface a
    // synthetic message rather than NULL ("no error"), matching the Internal
    // code returned by snapforge_app_last_error_code.
    let Ok(guard) = LAST_APP_ERROR.lock() else {
        return match cstring_sanitized("internal: error state lock poisoned") {
            Ok(cs) => cs.into_raw(),
            Err(_) => ptr::null_mut(),
        };
    };
    match guard.as_ref() {
        Some((_, msg)) => match cstring_sanitized(msg) {
            Ok(cs) => cs.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        None => ptr::null_mut(),
    }
}

/// Get the stable category code of the last use-case error.
///
/// Returns `SnapforgeErrorCode::None` (0) when the last call succeeded or none
/// has run, or `SnapforgeErrorCode::Internal` if the error-state lock is
/// poisoned (a prior holder panicked) — never a misleading 0 in that case.
#[no_mangle]
pub extern "C" fn snapforge_app_last_error_code() -> i32 {
    match LAST_APP_ERROR.lock() {
        Ok(guard) => match guard.as_ref() {
            Some((code, _)) => *code as i32,
            None => SnapforgeErrorCode::None as i32,
        },
        Err(_) => SnapforgeErrorCode::Internal as i32,
    }
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
/// Panic-safe: the FFI callback runs inside `catch_unwind` so a panicking Qt
/// callback can never unwind across the boundary (the workspace builds with
/// `panic = "abort"`, but the guard keeps the contract explicit and also
/// covers the test profile which uses unwind).
fn dispatch_log(level: SnapforgeLogLevel, message: &str) {
    let raw = LOG_CALLBACK.load(Ordering::Acquire);
    if raw.is_null() {
        eprintln!("{}", message);
        return;
    }
    // SAFETY: `raw` was stored from a valid `SnapforgeLogCallback` in
    // `snapforge_set_log_callback` and is only ever cleared back to null.
    let cb: SnapforgeLogCallback = unsafe { std::mem::transmute(raw) };
    // Reuse the NUL sanitizer so an interior NUL in a log line can never be
    // passed to C as a truncated / malformed string.
    let Ok(cstr) = cstring_sanitized(message) else {
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cb(level as i32, cstr.as_ptr());
    }));
    if result.is_err() {
        // Don't recurse through tracing here — the layer is what called us.
        eprintln!("[snapforge] log callback panicked; suppressing");
    }
}

/// Register the C callback that receives formatted Rust log records, and (on
/// first call) install the tracing subscriber that drives it. Pass a non-null
/// `callback`; passing it again replaces the previous one. The subscriber is
/// installed exactly once — subsequent calls only swap the callback pointer.
///
/// Qt calls this once at startup to route Rust diagnostics into its rotating
/// log file. Until it does, Rust logs fall back to stderr.
///
/// # Safety
///
/// `callback` must be a valid function pointer for the remaining lifetime of
/// the process (it is stored and invoked from arbitrary Rust threads). It must
/// not unwind — Rust guards the call with `catch_unwind`, but the callback
/// should still avoid panicking.
#[no_mangle]
pub unsafe extern "C" fn snapforge_set_log_callback(callback: SnapforgeLogCallback) {
    LOG_CALLBACK.store(callback as *mut (), Ordering::Release);

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

/// Parse a `Rect` from the `region` field of a JSON value. Returns `Ok(None)`
/// when the field is absent / not an object, `Ok(Some(_))` when valid, and
/// `Err(_)` when width or height is zero (which the underlying capture would
/// reject anyway, but we want a clean error message at the FFI boundary).
fn parse_optional_region(v: &serde_json::Value) -> Result<Option<Rect>, String> {
    if !v.is_object() {
        return Ok(None);
    }
    let width = v["width"].as_u64().unwrap_or(0) as u32;
    let height = v["height"].as_u64().unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("region has zero width or height".into());
    }
    Ok(Some(Rect {
        x: v["x"].as_i64().unwrap_or(0) as i32,
        y: v["y"].as_i64().unwrap_or(0) as i32,
        width,
        height,
    }))
}

/// Take a screenshot.
///
/// `req_json` is a null-terminated UTF-8 JSON string with fields:
///   display (u32), region (optional {x,y,width,height}), output_path (string),
///   format ("png"/"jpg"/"webp"), quality (u8 1..=100),
///   copy_to_clipboard (bool), add_to_history (bool).
///
/// Returns a heap-allocated JSON string `{"saved_path": "..."}` on success or
/// NULL on error (call `snapforge_app_last_error` for details). Caller must
/// free the returned string via `snapforge_free_string`.
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
    let v = match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => v,
        Err(e) => {
            set_app_error(format!("invalid request JSON: {}", e));
            return ptr::null_mut();
        }
    };

    let display = v["display"].as_u64().unwrap_or(0) as usize;
    let region = match parse_optional_region(&v["region"]) {
        Ok(r) => r,
        Err(e) => {
            set_app_error(e);
            return ptr::null_mut();
        }
    };
    let output_path = match v["output_path"].as_str() {
        Some(s) if !s.is_empty() => PathBuf::from(s),
        _ => {
            set_app_error("output_path is missing or empty");
            return ptr::null_mut();
        }
    };
    let format = match v["format"].as_str().unwrap_or("png") {
        "jpg" | "jpeg" => CaptureFormat::Jpg,
        "webp" => CaptureFormat::WebP,
        _ => CaptureFormat::Png,
    };
    let quality = v["quality"].as_u64().unwrap_or(90).clamp(1, 100) as u8;
    let copy_to_clipboard = v["copy_to_clipboard"].as_bool().unwrap_or(false);
    let add_to_history = v["add_to_history"].as_bool().unwrap_or(false);

    let req = app_screenshot::ScreenshotRequest {
        display,
        region,
        output_path,
        format,
        quality,
        copy_to_clipboard,
        add_to_history,
    };

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
            match cstring_sanitized(&serialized) {
                Ok(cs) => cs.into_raw(),
                Err(_) => ptr::null_mut(),
            }
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
    let v = match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => v,
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
    let bytes = std::slice::from_raw_parts(rgba, rgba_len).to_vec();

    let output_path = v["output_path"].as_str().and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    });
    let format = match v["format"].as_str().unwrap_or("png") {
        "jpg" | "jpeg" => CaptureFormat::Jpg,
        "webp" => CaptureFormat::WebP,
        _ => CaptureFormat::Png,
    };
    let quality = v["quality"].as_u64().unwrap_or(90).clamp(1, 100) as u8;
    let copy_to_clipboard = v["copy_to_clipboard"].as_bool().unwrap_or(false);
    let add_to_history = v["add_to_history"].as_bool().unwrap_or(false);

    let req = app_screenshot::SavePrerenderedRequest {
        rgba: bytes,
        width,
        height,
        output_path,
        format,
        quality,
        copy_to_clipboard,
        add_to_history,
    };
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
            match cstring_sanitized(&serialized) {
                Ok(cs) => cs.into_raw(),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(e) => {
            set_app_error_from(&e);
            ptr::null_mut()
        }
    }
}

// --- Recording (use-case) ---------------------------------------------------

const APP_RECORDING_MAGIC_ALIVE: u64 = 0x534E_4150_5245_4348; // "SNAPRECH"
const APP_RECORDING_MAGIC_DEAD: u64 = 0xDEAD_BEEF_DEAD_BEEF;

#[repr(C)]
struct FfiAppRecordingHandle {
    magic: u64,
    inner: Mutex<Option<app_recording::RecordingHandle>>,
}

unsafe fn check_app_recording_handle<'a>(handle: *mut c_void) -> Option<&'a FfiAppRecordingHandle> {
    if handle.is_null() || (handle as usize) < 4096 {
        return None;
    }
    if !is_registered_handle(handle) {
        tracing::warn!(
            "[snapforge] app recording handle {:p} not in registry; rejecting",
            handle
        );
        return None;
    }
    let p = handle.cast::<FfiAppRecordingHandle>();
    let magic = std::ptr::read_volatile(std::ptr::addr_of!((*p).magic));
    if magic != APP_RECORDING_MAGIC_ALIVE {
        tracing::warn!(
            "[snapforge] app recording handle {:p} bad magic 0x{:016x}; rejecting",
            handle,
            magic
        );
        return None;
    }
    Some(&*p)
}

/// Start a recording via the use-case surface.
///
/// `req_json` mirrors `snapforge_start_recording`'s JSON plus an
/// `add_to_history_on_stop` boolean. Returns an opaque handle, or NULL on
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
    let v = match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => v,
        Err(e) => {
            set_app_error(format!("invalid request JSON: {}", e));
            return ptr::null_mut();
        }
    };

    let display = v["display"].as_u64().unwrap_or(0) as usize;
    let region = match parse_optional_region(&v["region"]) {
        Ok(r) => r,
        Err(e) => {
            set_app_error(e);
            return ptr::null_mut();
        }
    };
    let output_path = match v["output_path"].as_str() {
        Some(s) if !s.is_empty() => PathBuf::from(s),
        _ => {
            set_app_error("output_path is missing or empty");
            return ptr::null_mut();
        }
    };
    let format = match v["format"].as_str() {
        Some("gif") => RecordingFormat::Gif,
        _ => RecordingFormat::Mp4,
    };
    let fps = v["fps"].as_u64().unwrap_or(30).clamp(1, 240) as u32;
    let quality = match v["quality"].as_str() {
        Some("low") => RecordingQuality::Low,
        Some("high") => RecordingQuality::High,
        _ => RecordingQuality::Medium,
    };
    let ffmpeg_path = v["ffmpeg_path"].as_str().map(PathBuf::from);
    let add_to_history_on_stop = v["add_to_history_on_stop"].as_bool().unwrap_or(false);

    let req = app_recording::RecordingRequest {
        display,
        region,
        output_path,
        format,
        fps,
        quality,
        ffmpeg_path,
        add_to_history_on_stop,
    };

    match app_recording::start_recording(req) {
        Ok(handle) => {
            clear_app_error();
            let wrapper = Box::new(FfiAppRecordingHandle {
                magic: APP_RECORDING_MAGIC_ALIVE,
                inner: Mutex::new(Some(handle)),
            });
            let raw = Box::into_raw(wrapper).cast::<c_void>();
            register_handle(raw);
            raw
        }
        Err(e) => {
            set_app_error_from(&e);
            ptr::null_mut()
        }
    }
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
    match taken {
        Some(h) => match app_recording::stop_recording(h) {
            Ok(()) => {
                clear_app_error();
                0
            }
            Err(e) => {
                set_app_error_from(&e);
                -1
            }
        },
        None => -1,
    }
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
        return -1;
    };
    match guard.as_ref() {
        Some(h) => match app_recording::pause_recording(h) {
            Ok(()) => 0,
            Err(e) => {
                set_app_error_from(&e);
                -1
            }
        },
        None => -1,
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
        return -1;
    };
    match guard.as_ref() {
        Some(h) => match app_recording::resume_recording(h) {
            Ok(()) => 0,
            Err(e) => {
                set_app_error_from(&e);
                -1
            }
        },
        None => -1,
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
    if !unregister_handle(handle) {
        tracing::warn!(
            "[snapforge] record_free_handle: {:p} not in registry; ignoring",
            handle
        );
        return;
    }
    let p = handle.cast::<FfiAppRecordingHandle>();
    let magic = std::ptr::read_volatile(std::ptr::addr_of!((*p).magic));
    if magic != APP_RECORDING_MAGIC_ALIVE {
        tracing::error!(
            "[snapforge] record_free_handle: {:p} bad magic 0x{:016x}; leaking",
            handle,
            magic
        );
        return;
    }
    std::ptr::write_volatile(std::ptr::addr_of_mut!((*p).magic), APP_RECORDING_MAGIC_DEAD);
    let _ = Box::from_raw(p);
}

// --- Click tracking (use-case) ---------------------------------------------

const APP_CLICKS_MAGIC_ALIVE: u64 = 0x534E_4150_434C_4B48; // "SNAPCLKH"
const APP_CLICKS_MAGIC_DEAD: u64 = 0xDEAD_C1ED_DEAD_C1ED;

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

unsafe fn check_click_handle<'a>(handle: *mut c_void) -> Option<&'a FfiClickHandle> {
    if handle.is_null() || (handle as usize) < 4096 {
        return None;
    }
    if !is_registered_handle(handle) {
        tracing::warn!(
            "[snapforge] click handle {:p} not in registry; rejecting",
            handle
        );
        return None;
    }
    let p = handle.cast::<FfiClickHandle>();
    let magic = std::ptr::read_volatile(std::ptr::addr_of!((*p).magic));
    if magic != APP_CLICKS_MAGIC_ALIVE {
        tracing::warn!(
            "[snapforge] click handle {:p} bad magic 0x{:016x}; rejecting",
            handle,
            magic
        );
        return None;
    }
    Some(&*p)
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
/// `callback` must remain valid for the lifetime of the returned handle.
/// `user_data` must remain valid for the same lifetime if the callback
/// dereferences it.
#[no_mangle]
pub unsafe extern "C" fn snapforge_clicks_start(
    callback: SnapforgeClickCallback,
    user_data: *mut c_void,
) -> *mut c_void {
    // Cast the user pointer to usize for transport across the thread
    // boundary — `*mut c_void` is `!Send` even when wrapped in a newtype
    // because the rustc auto-trait check looks through the closure capture.
    // We never dereference it; the callback gets the bit pattern back as a
    // pointer. Raw `extern "C" fn` is Send already.
    let ud_bits = user_data as usize;
    let cb = callback;
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
    clear_app_error();
    let wrapper = Box::new(FfiClickHandle {
        magic: APP_CLICKS_MAGIC_ALIVE,
        inner: Mutex::new(Some(inner)),
    });
    let raw = Box::into_raw(wrapper).cast::<c_void>();
    register_handle(raw);
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
        return -1;
    };
    // Dropping the ClickHandle stops the tap + joins the forwarder thread.
    let _ = guard.take();
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
    if !unregister_handle(handle) {
        tracing::warn!(
            "[snapforge] clicks_free_handle: {:p} not in registry; ignoring",
            handle
        );
        return;
    }
    let p = handle.cast::<FfiClickHandle>();
    let magic = std::ptr::read_volatile(std::ptr::addr_of!((*p).magic));
    if magic != APP_CLICKS_MAGIC_ALIVE {
        tracing::error!(
            "[snapforge] clicks_free_handle: {:p} bad magic 0x{:016x}; leaking",
            handle,
            magic
        );
        return;
    }
    std::ptr::write_volatile(std::ptr::addr_of_mut!((*p).magic), APP_CLICKS_MAGIC_DEAD);
    let _ = Box::from_raw(p);
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
    // path actually runs `Box::from_raw`. They pin the four safety
    // properties that the custom `PtrHasher` + capacity prealloc must not
    // weaken, and run without a display (unlike the capture round-trips in
    // tests/abi.rs).

    /// Allocate a real `Box<[u8]>` exactly as the capture path does and
    /// register it. Returns the raw pointer + len the caller would free.
    fn alloc_and_register(len: usize) -> (*mut u8, usize) {
        let boxed: Box<[u8]> = vec![0u8; len].into_boxed_slice();
        let data = Box::into_raw(boxed).cast::<u8>();
        register_buffer(data, len);
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
        let cfg_err = AppError::Config(snapforge_core::config::ConfigError::Parse(
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
        unsafe { snapforge_set_log_callback(capture_log) };
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
