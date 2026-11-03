use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::Duration;

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_core_foundation::CFString;
use objc2_foundation::{NSArray, NSOperatingSystemVersion, NSProcessInfo};
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
    SCWindow,
};
use rayon::prelude::*;

use snapforge_domain::Rect;

use super::CaptureError;

mod stream;
pub use stream::StreamCapture;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPointRaw {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSizeRaw {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRectRaw {
    origin: CGPointRaw,
    size: CGSizeRaw,
}

extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGDisplayCopyDisplayMode(display: u32) -> *const std::ffi::c_void;
    fn CGDisplayModeGetPixelWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeGetWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeRelease(mode: *const std::ffi::c_void);
    fn CGMainDisplayID() -> u32;
    /// Legacy display capture API. Deprecated in macOS 14 in favor of SCK,
    /// but still functional and the only path that reliably captures
    /// fullscreen-Space content (SCK returns a black frame for many
    /// fullscreen apps on macOS 14+). Returns NULL on failure.
    fn CGDisplayCreateImage(display: u32) -> *mut objc2_core_graphics::CGImage;
    /// Returns the number of displays that contain `point` and fills `displays` with their IDs.
    fn CGGetDisplaysWithPoint(
        point: CGPointRaw,
        max_displays: u32,
        displays: *mut u32,
        matching_count: *mut u32,
    ) -> i32;
    /// Synchronous, in-process display list. The first entry is the main
    /// (menu-bar) display. Needs no TCC grant, unlike SCShareableContent.
    fn CGGetActiveDisplayList(max_displays: u32, displays: *mut u32, count: *mut u32) -> i32;
    /// Primary display of the mirror set `display` belongs to, or 0
    /// (kCGNullDirectDisplay) when it isn't mirroring another display.
    fn CGDisplayMirrorsDisplay(display: u32) -> u32;
    /// Non-zero while the display is connected (stays true while asleep).
    fn CGDisplayIsOnline(display: u32) -> u32;
    /// Returns the display's bounds in the global desktop (point) coordinate
    /// space. The origin is non-zero for any display that isn't the primary.
    fn CGDisplayBounds(display: u32) -> CGRectRaw;
}

// ---------------------------------------------------------------------------
// OS version guards
// ---------------------------------------------------------------------------

fn os_at_least(major: isize, minor: isize) -> bool {
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
        majorVersion: major,
        minorVersion: minor,
        patchVersion: 0,
    })
}

/// SCShareableContent exists from macOS 12.3. Calling it on an older system
/// would hit objc2's missing-class panic, so every SCK path checks first.
fn sck_content_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| os_at_least(12, 3))
}

/// SCScreenshotManager (used for every SCK frame we grab) needs macOS 14.
fn ensure_sck_screenshot_available() -> Result<(), CaptureError> {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    let ok = *AVAILABLE.get_or_init(|| {
        let ok = os_at_least(14, 0);
        if !ok {
            tracing::warn!(
                "[capture] ScreenCaptureKit screenshots need macOS 14+; SCK capture disabled"
            );
        }
        ok
    });
    if ok {
        Ok(())
    } else {
        Err(CaptureError::CaptureFailed)
    }
}

// ---------------------------------------------------------------------------
// Display enumeration (CoreGraphics — synchronous, no TCC)
// ---------------------------------------------------------------------------

/// Active, drawable displays in CoreGraphics order: index 0 is the main
/// (menu-bar) display. Displays that merely mirror another display are
/// dropped — capturing them would duplicate their mirror-set primary.
///
/// This is the single source of truth for "display index". It replaces the
/// previous SCShareableContent-based list, which could block the calling
/// (often the Qt main) thread for up to 2s, returned an empty list on timeout
/// or without Screen Recording permission, and made even the CoreGraphics
/// screenshot path depend on SCK. CGGetActiveDisplayList is an in-process
/// call, so no cache is needed (and none can go stale across a topology
/// change).
fn active_display_ids() -> Vec<u32> {
    let mut count: u32 = 0;
    // SAFETY: a null buffer with max 0 asks only for the count.
    if unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &raw mut count) } != 0 {
        return Vec::new();
    }
    if count == 0 {
        return Vec::new();
    }
    let mut ids = vec![0u32; count as usize];
    // SAFETY: `ids` has room for `count` entries; CG writes at most that many.
    let err = unsafe { CGGetActiveDisplayList(count, ids.as_mut_ptr(), &raw mut count) };
    if err != 0 {
        return Vec::new();
    }
    ids.truncate(count as usize);
    ids.retain(|&id| unsafe { CGDisplayMirrorsDisplay(id) } == 0);
    ids
}

/// Resolve a display index (CoreGraphics active-display order, see
/// [`active_display_ids`]) to its CGDirectDisplayID.
fn display_id_for_index(display: usize) -> Result<u32, CaptureError> {
    active_display_ids()
        .get(display)
        .copied()
        .ok_or(CaptureError::NoDisplay(display))
}

/// Point-to-pixel scale of a display's current mode (pixel width / point
/// width). Falls back to 1.0 when the mode can't be read: assuming Retina
/// would double every coordinate on a non-Retina display.
fn display_scale(display_id: u32) -> Option<f64> {
    unsafe {
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return None;
        }
        let pixel_w = CGDisplayModeGetPixelWidth(mode);
        let point_w = CGDisplayModeGetWidth(mode);
        CGDisplayModeRelease(mode);
        (point_w != 0 && pixel_w != 0).then(|| pixel_w as f64 / point_w as f64)
    }
}

/// Get the point-to-pixel scale factor of the primary display.
/// Returns 1.0 on non-Retina, 2.0 on Retina (typically); 1.0 if the display
/// mode can't be read.
pub fn primary_display_scale_factor() -> f64 {
    display_scale(unsafe { CGMainDisplayID() }).unwrap_or(1.0)
}

/// Get the native pixel size of a display by index (Retina-aware).
pub fn display_pixel_size_for_index(display: usize) -> Result<(usize, usize), CaptureError> {
    let display_id = display_id_for_index(display)?;
    display_pixel_size(display_id)
}

/// Get the point-to-pixel scale factor of a display by index.
/// Returns 1.0 on non-Retina, 2.0 on Retina (typically); falls back to 1.0
/// if the display mode can't be read (matching `primary_display_scale_factor`).
pub fn display_scale_factor_for(display: usize) -> Result<f64, CaptureError> {
    let display_id = display_id_for_index(display)?;
    Ok(display_scale(display_id).unwrap_or(1.0))
}

/// Get a display's origin in global desktop point coordinates by index.
/// (0, 0) for the primary display; non-zero for secondary displays.
pub fn display_origin_points(display: usize) -> Result<(f64, f64), CaptureError> {
    let display_id = display_id_for_index(display)?;
    let bounds = unsafe { CGDisplayBounds(display_id) };
    Ok((bounds.origin.x, bounds.origin.y))
}

/// Get the true backing pixel dimensions for a display (Retina-aware).
///
/// Derived from `CGDisplayBounds` (which reflects rotation) times the mode's
/// pixel/point ratio. `CGDisplayModeGetPixelWidth/Height` alone describe the
/// *unrotated* mode, so a 90°-rotated display would get width/height swapped.
/// Returns Err if the display mode can't be read or the bounds are empty.
fn display_pixel_size(display_id: u32) -> Result<(usize, usize), CaptureError> {
    let scale = display_scale(display_id).ok_or(CaptureError::CaptureFailed)?;
    let bounds = unsafe { CGDisplayBounds(display_id) };
    let w = (bounds.size.width * scale).round();
    let h = (bounds.size.height * scale).round();
    if w < 1.0 || h < 1.0 {
        return Err(CaptureError::CaptureFailed);
    }
    Ok((w as usize, h as usize))
}

/// Downscale `(native_w, native_h)` so the longer side is at most
/// `max_dimension` (never upscaling), then round both down to even numbers
/// (libx264 yuv420p requirement). `None` keeps the native size.
fn scaled_even_size(native_w: usize, native_h: usize, max_dimension: Option<u32>) -> (u32, u32) {
    let (w, h) = match max_dimension {
        Some(_) if native_w == 0 || native_h == 0 => (0, 0),
        Some(max_dim) => {
            let scale = (f64::from(max_dim) / native_w.max(native_h) as f64).min(1.0);
            (
                (native_w as f64 * scale) as usize,
                (native_h as f64 * scale) as usize,
            )
        }
        None => (native_w, native_h),
    };
    ((w & !1) as u32, (h & !1) as u32)
}

// ---------------------------------------------------------------------------
// Permission
// ---------------------------------------------------------------------------

/// Latches once we've established this process can't see screen-recording
/// permission. macOS only applies a freshly-granted Screen Recording grant to
/// processes started *after* the grant; a running process keeps getting the
/// system prompt every time it touches SCK. Latching means we stop pinging
/// SCK after the first denial of this session, so the user isn't spammed with
/// repeated TCC dialogs. Cleared whenever `CGPreflightScreenCaptureAccess`
/// reports true.
static PERMISSION_DENIED_LATCH: AtomicBool = AtomicBool::new(false);

/// Set after `request_screen_capture_permission` triggers the system dialog
/// once. Subsequent requests in the same process are no-ops to avoid stacking
/// dialogs that the user can't dismiss into a usable state without a restart.
static PERMISSION_REQUESTED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

/// Live preflight check. Never cached: it's cheap and the grant can be
/// revoked mid-session, so a latched "granted" would lie. Clears the denied
/// latch on success so SCK may be tried again.
fn preflight_screen_capture() -> bool {
    let granted = unsafe { CGPreflightScreenCaptureAccess() };
    if granted {
        PERMISSION_DENIED_LATCH.store(false, Ordering::Relaxed);
    }
    granted
}

/// Check if screen recording permission is granted.
///
/// A pure status query: it never touches ScreenCaptureKit, because an SCK
/// content query from an ungranted process raises the TCC prompt — doing that
/// here and again in `request_screen_capture_permission` produced a double
/// prompt.
pub fn has_screen_capture_permission() -> bool {
    preflight_screen_capture()
}

/// Request screen recording permission. Triggers the system dialog the first
/// time it's called per session and returns whatever Core Graphics reports.
/// Subsequent calls in the same process are no-ops (return the live preflight
/// result) — macOS applies the grant only to *new* processes, so re-prompting
/// the same process doesn't help and just spams the user.
pub fn request_screen_capture_permission() -> bool {
    if preflight_screen_capture() {
        return true;
    }
    if PERMISSION_REQUESTED_THIS_SESSION.swap(true, Ordering::Relaxed) {
        return false;
    }
    let result = unsafe { CGRequestScreenCaptureAccess() };
    if result {
        PERMISSION_DENIED_LATCH.store(false, Ordering::Relaxed);
    }
    result
}

// ---------------------------------------------------------------------------
// SCK display lookup (only for SCK-specific capture paths)
// ---------------------------------------------------------------------------

/// SCShareableContent can take seconds to answer when the SCK daemon is busy
/// (first call after login, wake from sleep). Recording setup runs on a
/// worker thread, so it gets a generous timeout instead of failing spuriously.
const SCK_CONTENT_TIMEOUT: Duration = Duration::from_secs(6);

/// Screenshot budget per SCK step (display lookup, then the capture itself).
/// Screenshots are interactive, so on a slow/hung SCK daemon we'd rather fall
/// back to `CGDisplayCreateImage` quickly than keep the user waiting.
const SCK_SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(3);

/// Per-frame budget for the screenshot-per-frame recording fallback.
const SCK_FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// `kCGColorSpaceSRGB`. Every capture path produces sRGB pixels: SCK is asked
/// for it via `SCStreamConfiguration.colorSpaceName`, and CoreGraphics images
/// in any other colour space are converted (see [`with_cg_pixels`]). Saved
/// PNG/JPEG files and the BT.709-tagged MP4 are untagged/sRGB-assumed, so
/// handing them display-space (e.g. Display P3) bytes shifts colours.
fn srgb_color_space_name() -> &'static CFString {
    // SAFETY: immutable CoreGraphics constant.
    unsafe { objc2_core_graphics::kCGColorSpaceSRGB }
}

/// Fetch available displays via SCShareableContent (async → sync bridge).
/// Returns `None` if SCK is unavailable on this OS, doesn't respond within
/// `timeout`, or errors.
///
/// Skips the SCK call entirely while the per-session permission-denied latch
/// is set (and preflight still says no): SCShareableContent triggers the
/// macOS TCC dialog every time it's called from a process that hasn't been
/// granted Screen Recording, so repeating the call after a known denial just
/// spams the user.
fn get_shareable_displays(timeout: Duration) -> Option<Retained<NSArray<SCDisplay>>> {
    if !sck_content_available() {
        return None;
    }
    if PERMISSION_DENIED_LATCH.load(Ordering::Relaxed) && !preflight_screen_capture() {
        return None;
    }

    /// Moves the SCK display array from the completion-handler queue back to
    /// the calling thread. `Retained<NSArray<SCDisplay>>` is not `Send` in
    /// objc2's conservative typing, but this specific transfer is sound: the
    /// array and its SCDisplay entries are immutable snapshot objects that
    /// ScreenCaptureKit itself hands across queues, and we MOVE the sole
    /// retained reference (never share it between threads).
    // ponytail: consumers (SCContentFilter creation) genuinely need the
    // SCDisplay object on their own thread, so an ObjC object must cross —
    // this wrapper makes that crossing explicit instead of eliminating it.
    struct SendableDisplays(Retained<NSArray<SCDisplay>>);
    unsafe impl Send for SendableDisplays {}

    let (tx, rx) = mpsc::channel::<Option<SendableDisplays>>();
    let block = block2::RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut objc2_foundation::NSError| {
            if error.is_null() && !content.is_null() {
                let displays = unsafe { (*content).displays() };
                let _ = tx.send(Some(SendableDisplays(displays)));
            } else {
                let _ = tx.send(None);
            }
        },
    );
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            true,
            true,
            &block,
        );
    }
    let result = rx.recv_timeout(timeout).ok().flatten().map(|d| d.0);

    if result.is_none() && !unsafe { CGPreflightScreenCaptureAccess() } {
        // SCK gave us nothing and the system says permission isn't granted.
        // Latch so we don't trigger another TCC prompt on the next call.
        // The latch clears if preflight ever flips back to true.
        PERMISSION_DENIED_LATCH.store(true, Ordering::Relaxed);
    }

    result
}

/// Find the SCDisplay object for a CGDirectDisplayID. `display` is only used
/// for the error value.
fn sc_display_for_id(
    display: usize,
    target_id: u32,
    timeout: Duration,
) -> Result<Retained<SCDisplay>, CaptureError> {
    let displays = get_shareable_displays(timeout).ok_or(CaptureError::CaptureFailed)?;
    (0..displays.len())
        .map(|i| displays.objectAtIndex(i))
        .find(|d| unsafe { d.displayID() } == target_id)
        .ok_or(CaptureError::NoDisplay(display))
}

pub fn display_count() -> usize {
    let n = active_display_ids().len();
    if n == 0 {
        tracing::warn!("[capture] display_count: CGGetActiveDisplayList returned no displays");
    }
    n
}

/// Map a screen point (in global point coordinates) to the display index
/// in [`active_display_ids`] order. Returns None if no display contains the
/// point or the mapping fails.
pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    let mut matches: [u32; 16] = [0; 16];
    let mut count: u32 = 0;
    let status = unsafe {
        CGGetDisplaysWithPoint(
            CGPointRaw {
                x: f64::from(x),
                y: f64::from(y),
            },
            matches.len() as u32,
            matches.as_mut_ptr(),
            &raw mut count,
        )
    };
    if status != 0 || count == 0 {
        return None;
    }
    let ids = active_display_ids();
    // With mirroring, several displays contain the point; take the first one
    // that's in our (mirror-filtered) list, i.e. the mirror-set primary.
    // A display not in our list honors the C contract (-1 == None) instead of
    // silently snapping to 0.
    matches[..(count as usize).min(matches.len())]
        .iter()
        .find_map(|m| ids.iter().position(|id| id == m))
}

/// Capture the full screen for a given display index.
///
/// SCK completion handlers need the main RunLoop to be free, so when called
/// from the main thread we off-load the capture to a short-lived worker
/// thread and join it.
pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    if is_main_thread() {
        // Spawn+join: SCK ObjC objects are not Send, so we build them on the
        // worker thread. Only the resulting RgbaImage (Send) crosses back.
        std::thread::spawn(move || capture_fullscreen_inner(display))
            .join()
            .map_err(|_| CaptureError::CaptureFailed)?
    } else {
        capture_fullscreen_inner(display)
    }
}

fn capture_fullscreen_inner(display: usize) -> Result<RgbaImage, CaptureError> {
    let target_id = display_id_for_index(display)?;
    // Primary: ScreenCaptureKit (`SCScreenshotManager`, sRGB). Fallback:
    // the deprecated `CGDisplayCreateImage`, only reached when SCK fails
    // (timeout, daemon trouble, denied permission) or returns an all-black
    // frame — SCK can do that for some fullscreen-Space apps (commit
    // `6982101`), where the legacy API still sees the content.
    screenshot_with_fallback(
        || capture_fullscreen_via_sck(display, target_id),
        || capture_via_cgdisplay(target_id),
    )
}

/// Pick a screenshot from the primary (SCK) and fallback (CG) paths: the
/// first non-black image wins; a black image is still better than an error
/// (the screen may really be black); the fallback runs only when the primary
/// failed or came back black.
fn screenshot_with_fallback(
    primary: impl FnOnce() -> Result<RgbaImage, CaptureError>,
    fallback: impl FnOnce() -> Option<RgbaImage>,
) -> Result<RgbaImage, CaptureError> {
    let primary = match primary() {
        Ok(img) if !is_all_zero(&img) => return Ok(img),
        Ok(img) => {
            tracing::debug!("[capture] SCK screenshot is all black; trying CGDisplayCreateImage");
            Ok(img)
        }
        Err(e) => {
            tracing::debug!("[capture] SCK screenshot failed ({e}); trying CGDisplayCreateImage");
            Err(e)
        }
    };
    match (primary, fallback()) {
        (_, Some(img)) if !is_all_zero(&img) => Ok(img),
        (Ok(black), _) | (Err(_), Some(black)) => Ok(black),
        (Err(e), None) => Err(e),
    }
}

/// Capture a display via CGDisplayCreateImage and convert to sRGB RGBA.
/// Returns None on any failure.
fn capture_via_cgdisplay(display_id: u32) -> Option<RgbaImage> {
    use objc2_core_graphics::CGImage;
    let cg_ptr = unsafe { CGDisplayCreateImage(display_id) };
    if cg_ptr.is_null() {
        return None;
    }
    // CGImageRef is owned by the caller per Create rule — we must release it.
    // Rust binding's Retained::from_raw takes ownership and releases on drop.
    let cg_image: Retained<CGImage> = unsafe { Retained::from_raw(cg_ptr)? };
    cg_image_to_rgba(&cg_image).ok()
}

fn is_all_zero(img: &RgbaImage) -> bool {
    // Cheap check: scan only every Nth pixel. A real screenshot will have
    // some non-zero pixel within the first few hundred samples.
    let (pixels, _) = img.as_raw().as_chunks::<4>();
    pixels
        .iter()
        .step_by(64)
        .all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0)
}

/// Build a display-wide SCK content filter (no excluded windows).
fn display_filter(sc_display: &SCDisplay) -> Retained<SCContentFilter> {
    unsafe {
        let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            sc_display,
            &excluded,
        )
    }
}

/// `SCStreamConfiguration` for a single BGRA, sRGB screenshot of `w`×`h`
/// pixels (both 0 = SCK's default size).
fn screenshot_config(w: usize, h: usize, shows_cursor: bool) -> Retained<SCStreamConfiguration> {
    // SAFETY: plain property setters on a fresh configuration object.
    unsafe {
        let config = SCStreamConfiguration::new();
        if w > 0 && h > 0 {
            config.setWidth(w);
            config.setHeight(h);
        }
        config.setPixelFormat(objc2_core_video::kCVPixelFormatType_32BGRA);
        config.setColorSpaceName(srgb_color_space_name());
        config.setShowsCursor(shows_cursor);
        config
    }
}

fn capture_fullscreen_via_sck(display: usize, target_id: u32) -> Result<RgbaImage, CaptureError> {
    ensure_sck_screenshot_available()?;
    // Without the Screen Recording grant SCK can only fail (and would raise
    // the TCC prompt doing so); go straight to the CoreGraphics fallback,
    // which is what an ungranted capture always used.
    if !preflight_screen_capture() {
        return Err(CaptureError::CaptureFailed);
    }
    let sc_display = sc_display_for_id(display, target_id, SCK_SCREENSHOT_TIMEOUT)?;
    // Whole display, no excluded windows: same content CGDisplayCreateImage
    // returned (menu bar, Dock, every window). Snapforge's region overlay is
    // only shown after the capture completes, so it can't end up in the shot.
    let filter = display_filter(&sc_display);
    let (w, h) = display_pixel_size(target_id)?;
    let config = screenshot_config(w, h, false);
    capture_with_filter(filter.as_ref(), config.as_ref())
}

/// Check if we're on the main thread. Uses pthread — Apple guarantees the
/// process's first thread is the main/UI thread, which matches what Cocoa
/// considers "main".
fn is_main_thread() -> bool {
    extern "C" {
        fn pthread_main_np() -> i32;
    }
    unsafe { pthread_main_np() != 0 }
}

/// Compute the downscaled recording output size WITHOUT building SCK objects.
/// Safe to call on the main thread. Returns the even-aligned (width, height).
pub fn compute_recording_output_size(
    display: usize,
    max_dimension: Option<u32>,
) -> Result<(u32, u32), CaptureError> {
    let display_id = display_id_for_index(display)?;
    let (native_w, native_h) = display_pixel_size(display_id)?;
    Ok(scaled_even_size(native_w, native_h, max_dimension))
}

/// Capture a single frame using a pre-built filter and config.
fn capture_with_filter(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
) -> Result<RgbaImage, CaptureError> {
    ensure_sck_screenshot_available()?;
    let (tx, rx) = mpsc::channel::<Option<RgbaImage>>();
    let block = block2::RcBlock::new(
        move |cg_image: *mut objc2_core_graphics::CGImage,
              error: *mut objc2_foundation::NSError| {
            if error.is_null() && !cg_image.is_null() {
                let img = unsafe { &*cg_image };
                let _ = tx.send(cg_image_to_rgba(img).ok());
            } else {
                let _ = tx.send(None);
            }
        },
    );
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&*block),
        );
    }

    rx.recv_timeout(SCK_SCREENSHOT_TIMEOUT)
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)
}

/// A reusable capture context that holds pre-built SCK filter and config.
/// Create once, call `capture_frame()` repeatedly for recording.
/// IMPORTANT: Must be used on the same thread it was created on (ObjC objects are not Send).
pub struct CaptureContext {
    filter: Retained<SCContentFilter>,
    config: Retained<SCStreamConfiguration>,
    /// Index the context was created for — reported in `NoDisplay` errors.
    display: usize,
    /// CGDirectDisplayID the filter targets, for the per-frame liveness check.
    display_id: u32,
    pub output_width: u32,
    pub output_height: u32,
}

/// A raw captured frame in tightly-packed BGRA format (what SCK returns natively).
pub struct RawFrame {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl CaptureContext {
    /// Create a capture context for a given display.
    /// `max_dimension` optionally limits the output size (e.g. 1920 to cap at ~1080p
    /// while preserving aspect ratio). Pass None for native resolution.
    pub fn new(display: usize, max_dimension: Option<u32>) -> Result<Self, CaptureError> {
        ensure_sck_screenshot_available()?;
        let display_id = display_id_for_index(display)?;
        let sc_display = sc_display_for_id(display, display_id, SCK_CONTENT_TIMEOUT)?;
        let filter = display_filter(&sc_display);

        let (native_w, native_h) = display_pixel_size(display_id)?;
        let (out_w, out_h) = scaled_even_size(native_w, native_h, max_dimension);

        // Recordings show the live cursor; screenshots stay cursor-free.
        let config = screenshot_config(out_w as usize, out_h as usize, true);

        Ok(Self {
            filter,
            config,
            display,
            display_id,
            output_width: out_w,
            output_height: out_h,
        })
    }

    /// Capture a single frame and return raw BGRA bytes — fast path for recording.
    /// No per-pixel swap. Output is tightly packed `width * height * 4` bytes.
    ///
    /// Returns `CaptureError::NoDisplay` once the target display has been
    /// disconnected: the pre-built filter still references it and SCK would
    /// otherwise keep failing (or timing out) every frame. Callers should
    /// treat `NoDisplay` as fatal and stop the recording.
    pub fn capture_frame_raw_bgra(&self) -> Result<RawFrame, CaptureError> {
        // CGDisplayIsOnline (not IsActive) so a display that's merely asleep
        // doesn't abort the recording.
        if unsafe { CGDisplayIsOnline(self.display_id) } == 0 {
            return Err(CaptureError::NoDisplay(self.display));
        }
        capture_bgra_with_filter(self.filter.as_ref(), self.config.as_ref())
    }
}

/// Frame source for a recording: an [`StreamCapture`] (`SCStream`, the normal
/// path) or, if the stream can't be started, the older
/// [`CaptureContext`] that takes one `SCScreenshotManager` screenshot per
/// frame. Both deliver sRGB BGRA and report `NoDisplay` once the display is
/// disconnected. Not `Send`: create and use it on the recording worker.
pub enum RecordingSource {
    Stream(StreamCapture),
    Screenshots(CaptureContext),
}

/// Try `primary`; on failure log and try `fallback`. A `NoDisplay` from the
/// primary is returned as-is — the fallback targets the same display.
fn start_with_fallback<T>(
    primary: impl FnOnce() -> Result<T, CaptureError>,
    fallback: impl FnOnce() -> Result<T, CaptureError>,
) -> Result<T, CaptureError> {
    match primary() {
        Ok(v) => Ok(v),
        Err(e @ CaptureError::NoDisplay(_)) => Err(e),
        Err(e) => {
            tracing::warn!(
                "[capture] SCStream unavailable ({e}); recording falls back to one screenshot per frame"
            );
            fallback()
        }
    }
}

impl RecordingSource {
    /// Start capturing `display` at the recording output size
    /// ([`compute_recording_output_size`] with `max_dimension`), with the
    /// cursor visible, at most `fps` frames per second.
    pub fn start(
        display: usize,
        max_dimension: Option<u32>,
        fps: u32,
    ) -> Result<Self, CaptureError> {
        let (out_w, out_h) = compute_recording_output_size(display, max_dimension)?;
        start_with_fallback(
            || StreamCapture::start(display, out_w, out_h, fps, true).map(Self::Stream),
            || CaptureContext::new(display, max_dimension).map(Self::Screenshots),
        )
    }

    /// True when frames come from `SCStream` (not the screenshot fallback).
    pub fn is_stream(&self) -> bool {
        matches!(self, Self::Stream(_))
    }

    /// Hand the current frame to `f`. `Ok(false)` means "no new frame since
    /// last time" (screen idle — repeat the previous frame); only the stream
    /// returns it. Errors follow [`CaptureContext::capture_frame_raw_bgra`].
    pub fn next_frame(&mut self, f: impl FnOnce(&RawFrame)) -> Result<bool, CaptureError> {
        match self {
            Self::Stream(s) => s.latest_frame(f),
            Self::Screenshots(c) => c.capture_frame_raw_bgra().map(|raw| {
                f(&raw);
                true
            }),
        }
    }

    /// [`Self::next_frame`], but the stream waits up to `timeout` for its
    /// first frame (the screenshot fallback captures synchronously anyway).
    pub fn first_frame(
        &mut self,
        timeout: Duration,
        f: impl FnOnce(&RawFrame),
    ) -> Result<bool, CaptureError> {
        match self {
            Self::Stream(s) => s.wait_frame(timeout, f),
            Self::Screenshots(_) => self.next_frame(f),
        }
    }
}

/// Capture a CGImage and return tightly-packed BGRA bytes without channel swap.
fn capture_bgra_with_filter(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
) -> Result<RawFrame, CaptureError> {
    let (tx, rx) = mpsc::channel::<Option<RawFrame>>();
    let block = block2::RcBlock::new(
        move |cg_image: *mut objc2_core_graphics::CGImage,
              error: *mut objc2_foundation::NSError| {
            if error.is_null() && !cg_image.is_null() {
                let img = unsafe { &*cg_image };
                let _ = tx.send(cg_image_to_bgra(img).ok());
            } else {
                let _ = tx.send(None);
            }
        },
    );
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            filter,
            config,
            Some(&*block),
        );
    }

    rx.recv_timeout(SCK_FRAME_TIMEOUT)
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)
}

/// What the BGRA converters need to know about a CGImage's pixel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BgraLayout {
    /// The alpha byte is padding (`NoneSkipFirst`), not real alpha — the
    /// converters must write 255 instead of copying it.
    force_opaque: bool,
}

/// Validate that a CGImage is 32bpp / 8bpc with B,G,R,A byte order in memory
/// (32-bit little-endian, alpha "first" — what both CGDisplayCreateImage and
/// SCK produce). Anything else is rejected so the caller falls back to
/// another capture path instead of emitting channel-swapped garbage.
fn bgra_layout(
    bits_per_pixel: usize,
    bits_per_component: usize,
    alpha: objc2_core_graphics::CGImageAlphaInfo,
    byte_order: objc2_core_graphics::CGImageByteOrderInfo,
) -> Result<BgraLayout, CaptureError> {
    use objc2_core_graphics::{CGImageAlphaInfo as A, CGImageByteOrderInfo as O};
    if bits_per_pixel != 32 || bits_per_component != 8 {
        return Err(CaptureError::ImageDataFailed);
    }
    // 0x7000 == kCGImageByteOrderMask (the binding's constant is deprecated).
    if byte_order.0 & 0x7000 != O::Order32Little.0 {
        return Err(CaptureError::ImageDataFailed);
    }
    match alpha {
        A::PremultipliedFirst | A::First => Ok(BgraLayout {
            force_opaque: false,
        }),
        A::NoneSkipFirst => Ok(BgraLayout { force_opaque: true }),
        _ => Err(CaptureError::ImageDataFailed),
    }
}

/// Borrowed view of a validated CGImage's pixel buffer.
struct CgPixels<'a> {
    width: u32,
    height: u32,
    bytes_per_row: usize,
    layout: BgraLayout,
    bytes: &'a [u8],
}

impl CgPixels<'_> {
    fn row_bytes(&self) -> usize {
        self.width as usize * 4
    }

    fn row(&self, y: usize) -> &[u8] {
        let start = y * self.bytes_per_row;
        &self.bytes[start..start + self.row_bytes()]
    }
}

/// `CFStringGetCStringPtr`-free spelling of `kCGColorSpaceSRGB`'s value, for
/// the pure [`needs_srgb_redraw`] decision (a test pins it to the constant).
const SRGB_NAME: &str = "kCGColorSpaceSRGB";

/// Whether a CGImage must be redrawn into an sRGB BGRA bitmap before its
/// bytes can be used: either its colour space isn't sRGB (e.g. the display's
/// Display P3 or an unnamed ICC profile — `CGDisplayCreateImage` returns
/// those) or its memory layout isn't the BGRA the converters read.
fn needs_srgb_redraw(color_space_name: Option<&str>, layout_ok: bool) -> bool {
    !layout_ok || color_space_name != Some(SRGB_NAME)
}

/// Validate `cg_image` (size, pixel layout, colour space, buffer length) and
/// run `f` over a borrowed sRGB BGRA view of its pixels. Images that aren't
/// already sRGB BGRA are first converted by CoreGraphics (drawn into an sRGB
/// bitmap context), so callers always see sRGB.
fn with_cg_pixels<R>(
    cg_image: &objc2_core_graphics::CGImage,
    f: impl FnOnce(&CgPixels<'_>) -> Result<R, CaptureError>,
) -> Result<R, CaptureError> {
    use objc2_core_graphics::{CGColorSpace, CGImage};
    let img = Some(cg_image);
    let width = CGImage::width(img) as u32;
    let height = CGImage::height(img) as u32;
    if width == 0 || height == 0 {
        return Err(CaptureError::ImageDataFailed);
    }
    let layout = bgra_layout(
        CGImage::bits_per_pixel(img),
        CGImage::bits_per_component(img),
        CGImage::alpha_info(img),
        CGImage::byte_order_info(img),
    );
    let space_name = CGImage::color_space(img)
        .and_then(|cs| CGColorSpace::name(Some(&cs)))
        .map(|n| n.to_string());
    match layout {
        Ok(layout) if !needs_srgb_redraw(space_name.as_deref(), true) => {
            with_direct_pixels(cg_image, width, height, layout, f)
        }
        _ => {
            tracing::debug!(
                "[capture] converting {width}x{height} capture from {} to sRGB",
                space_name.as_deref().unwrap_or("an unnamed colour space")
            );
            with_srgb_redraw(cg_image, width, height, f)
        }
    }
}

/// Borrow the image's own pixel buffer (already sRGB BGRA).
fn with_direct_pixels<R>(
    cg_image: &objc2_core_graphics::CGImage,
    width: u32,
    height: u32,
    layout: BgraLayout,
    f: impl FnOnce(&CgPixels<'_>) -> Result<R, CaptureError>,
) -> Result<R, CaptureError> {
    use objc2_core_graphics::{CGDataProvider, CGImage};
    let img = Some(cg_image);
    let bytes_per_row = CGImage::bytes_per_row(img);
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or(CaptureError::ImageDataFailed)?;
    if bytes_per_row < row_bytes {
        return Err(CaptureError::ImageDataFailed);
    }
    let provider = CGImage::data_provider(img).ok_or(CaptureError::ImageDataFailed)?;
    let data = CGDataProvider::data(Some(&provider)).ok_or(CaptureError::ImageDataFailed)?;
    // SAFETY: the CFData is an immutable copy kept alive by `data` for the
    // whole borrow below.
    let bytes: &[u8] = unsafe { data.as_bytes_unchecked() };
    let min_required = (height as usize - 1)
        .checked_mul(bytes_per_row)
        .and_then(|v| v.checked_add(row_bytes))
        .ok_or(CaptureError::ImageDataFailed)?;
    if bytes.len() < min_required {
        return Err(CaptureError::ImageDataFailed);
    }
    f(&CgPixels {
        width,
        height,
        bytes_per_row,
        layout,
        bytes,
    })
}

/// Draw the image into a tightly-packed sRGB bitmap context laid out as
/// BGRA in memory (`PremultipliedFirst | ByteOrder32Little`) — CoreGraphics
/// does the colour-space conversion — and expose that buffer. Screen
/// captures are opaque, so alpha is forced to 255.
fn with_srgb_redraw<R>(
    cg_image: &objc2_core_graphics::CGImage,
    width: u32,
    height: u32,
    f: impl FnOnce(&CgPixels<'_>) -> Result<R, CaptureError>,
) -> Result<R, CaptureError> {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGImageByteOrderInfo,
    };
    let (w, h) = (width as usize, height as usize);
    let row_bytes = w.checked_mul(4).ok_or(CaptureError::ImageDataFailed)?;
    let len = row_bytes
        .checked_mul(h)
        .ok_or(CaptureError::ImageDataFailed)?;
    let mut buf = vec![0u8; len];
    let space = CGColorSpace::with_name(Some(srgb_color_space_name()))
        .ok_or(CaptureError::ImageDataFailed)?;
    let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    {
        // SAFETY: `buf` is `row_bytes * h` bytes, outlives the context (which
        // is dropped at the end of this block) and isn't touched meanwhile.
        let ctx = unsafe {
            CGBitmapContextCreate(
                buf.as_mut_ptr().cast(),
                w,
                h,
                8,
                row_bytes,
                Some(&space),
                info,
            )
        }
        .ok_or(CaptureError::ImageDataFailed)?;
        let rect = CGRect::new(
            CGPoint::ZERO,
            CGSize::new(f64::from(width), f64::from(height)),
        );
        CGContext::draw_image(Some(&ctx), rect, Some(cg_image));
    }
    f(&CgPixels {
        width,
        height,
        bytes_per_row: row_bytes,
        layout: BgraLayout { force_opaque: true },
        bytes: &buf,
    })
}

/// Extract BGRA bytes from a CGImage without channel swapping.
fn cg_image_to_bgra(cg_image: &objc2_core_graphics::CGImage) -> Result<RawFrame, CaptureError> {
    with_cg_pixels(cg_image, |px| {
        let row_bytes = px.row_bytes();
        let total_bytes = row_bytes * px.height as usize;
        let mut bytes = vec![0u8; total_bytes];
        // If the source has no padding, we can copy the whole buffer in one shot.
        if px.bytes_per_row == row_bytes {
            bytes.copy_from_slice(&px.bytes[..total_bytes]);
        } else {
            for (y, dst) in bytes.chunks_mut(row_bytes).enumerate() {
                dst.copy_from_slice(px.row(y));
            }
        }
        if px.layout.force_opaque {
            let (pixels, _) = bytes.as_chunks_mut::<4>();
            for p in pixels {
                p[3] = 255;
            }
        }
        Ok(RawFrame {
            bytes,
            width: px.width,
            height: px.height,
        })
    })
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    let full = capture_fullscreen(display)?;
    super::crop_to_region(&full, region)
}

fn cg_image_to_rgba(cg_image: &objc2_core_graphics::CGImage) -> Result<RgbaImage, CaptureError> {
    with_cg_pixels(cg_image, |px| {
        let row_bytes = px.row_bytes();
        let expected_bytes = row_bytes
            .checked_mul(px.height as usize)
            .ok_or(CaptureError::ImageDataFailed)?;
        let mut rgba_buf = vec![0u8; expected_bytes];
        let force_opaque = px.layout.force_opaque;

        // Parallelize across rows: for 4K (~33MB) the channel swap is a
        // memory-bound serial pass, so we split the tightly-packed destination
        // into per-row chunks and mirror each to the matching source row. The
        // source may carry row padding (`bytes_per_row` != width*4), so the
        // source offset is derived per row from its global row index.
        rgba_buf
            .par_chunks_exact_mut(row_bytes)
            .enumerate()
            .for_each(|(y, dst_row)| {
                let (src_px, _) = px.row(y).as_chunks::<4>();
                let (dst_px, _) = dst_row.as_chunks_mut::<4>();
                // Swap B and R channels: BGRA → RGBA
                for (src, dst) in src_px.iter().zip(dst_px) {
                    *dst = [
                        src[2],
                        src[1],
                        src[0],
                        if force_opaque { 255 } else { src[3] },
                    ];
                }
            });

        RgbaImage::from_raw(px.width, px.height, rgba_buf).ok_or(CaptureError::ImageDataFailed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count_nonzero() {
        assert!(display_count() > 0, "should detect at least one display");
    }

    #[test]
    fn has_permission_matches_live_preflight() {
        // Status query is uncached and SCK-free: it must equal preflight.
        let live = unsafe { CGPreflightScreenCaptureAccess() };
        assert_eq!(has_screen_capture_permission(), live);
        if live {
            assert!(!PERMISSION_DENIED_LATCH.load(Ordering::Relaxed));
        }
    }

    #[test]
    fn active_display_ids_main_display_first() {
        let ids = active_display_ids();
        assert!(!ids.is_empty(), "at least one active display");
        let main = unsafe { CGMainDisplayID() };
        assert_eq!(ids[0], main, "index 0 must be the main display");
    }

    #[test]
    fn display_at_point_origin_is_main_display() {
        // Global point (0,0) is the main display's top-left corner.
        assert_eq!(display_at_point(0, 0), Some(0));
    }

    #[test]
    fn display_pixel_size_matches_bounds_times_scale() {
        let id = unsafe { CGMainDisplayID() };
        let (w, h) = display_pixel_size(id).unwrap();
        let bounds = unsafe { CGDisplayBounds(id) };
        let scale = primary_display_scale_factor();
        assert_eq!(w, (bounds.size.width * scale).round() as usize);
        assert_eq!(h, (bounds.size.height * scale).round() as usize);
    }

    #[test]
    fn scaled_even_size_native_masks_odd() {
        assert_eq!(scaled_even_size(1921, 1081, None), (1920, 1080));
    }

    #[test]
    fn scaled_even_size_caps_longest_side_without_upscaling() {
        assert_eq!(scaled_even_size(3840, 2160, Some(1920)), (1920, 1080));
        // Portrait (rotated display): the height is the long side.
        assert_eq!(scaled_even_size(2160, 3840, Some(1920)), (1080, 1920));
        assert_eq!(scaled_even_size(1280, 720, Some(100_000)), (1280, 720));
        assert_eq!(scaled_even_size(0, 720, Some(640)), (0, 0));
    }

    #[test]
    fn bgra_layout_accepts_little_endian_alpha_first() {
        use objc2_core_graphics::{CGImageAlphaInfo as A, CGImageByteOrderInfo as O};
        assert_eq!(
            bgra_layout(32, 8, A::PremultipliedFirst, O::Order32Little).unwrap(),
            BgraLayout {
                force_opaque: false
            }
        );
        assert_eq!(
            bgra_layout(32, 8, A::NoneSkipFirst, O::Order32Little).unwrap(),
            BgraLayout { force_opaque: true }
        );
    }

    #[test]
    fn bgra_layout_rejects_other_layouts() {
        use objc2_core_graphics::{CGImageAlphaInfo as A, CGImageByteOrderInfo as O};
        // RGBA in memory (big-endian, alpha last).
        assert!(bgra_layout(32, 8, A::PremultipliedLast, O::Order32Big).is_err());
        // ARGB in memory (default/big-endian, alpha first).
        assert!(bgra_layout(32, 8, A::PremultipliedFirst, O::OrderDefault).is_err());
        // Little-endian alpha-last is ABGR in memory.
        assert!(bgra_layout(32, 8, A::NoneSkipLast, O::Order32Little).is_err());
        // Wide-gamut / HDR formats.
        assert!(bgra_layout(64, 16, A::PremultipliedFirst, O::Order32Little).is_err());
        assert!(bgra_layout(32, 10, A::NoneSkipFirst, O::Order32Little).is_err());
        assert!(bgra_layout(24, 8, A::None, O::Order32Little).is_err());
    }

    #[test]
    fn srgb_name_matches_coregraphics_constant() {
        assert_eq!(srgb_color_space_name().to_string(), SRGB_NAME);
    }

    #[test]
    fn srgb_redraw_decision() {
        // Already sRGB BGRA: use the bytes as-is.
        assert!(!needs_srgb_redraw(Some(SRGB_NAME), true));
        // Display colour spaces / unnamed ICC profiles must be converted.
        assert!(needs_srgb_redraw(Some("kCGColorSpaceDisplayP3"), true));
        assert!(needs_srgb_redraw(Some("kCGColorSpaceExtendedSRGB"), true));
        assert!(needs_srgb_redraw(None, true));
        // Non-BGRA layouts are redrawn even when sRGB.
        assert!(needs_srgb_redraw(Some(SRGB_NAME), false));
    }

    fn solid(byte: u8) -> RgbaImage {
        RgbaImage::from_pixel(4, 4, image::Rgba([byte, byte, byte, 255]))
    }

    #[test]
    fn screenshot_prefers_primary_and_skips_fallback() {
        let img = screenshot_with_fallback(|| Ok(solid(7)), || panic!("fallback must not run"));
        assert_eq!(img.unwrap().get_pixel(0, 0)[0], 7);
    }

    #[test]
    fn screenshot_falls_back_on_error_or_black() {
        let img = screenshot_with_fallback(|| Err(CaptureError::CaptureFailed), || Some(solid(9)));
        assert_eq!(img.unwrap().get_pixel(0, 0)[0], 9);
        let img = screenshot_with_fallback(|| Ok(solid(0)), || Some(solid(9)));
        assert_eq!(img.unwrap().get_pixel(0, 0)[0], 9);
    }

    #[test]
    fn screenshot_black_beats_error() {
        // Both black: keep the primary's image (the screen may be black).
        assert!(screenshot_with_fallback(|| Ok(solid(0)), || Some(solid(0))).is_ok());
        assert!(screenshot_with_fallback(|| Ok(solid(0)), || None).is_ok());
        assert!(
            screenshot_with_fallback(|| Err(CaptureError::CaptureFailed), || Some(solid(0)))
                .is_ok()
        );
        assert!(matches!(
            screenshot_with_fallback(|| Err(CaptureError::NoDisplay(3)), || None),
            Err(CaptureError::NoDisplay(3))
        ));
    }

    #[test]
    fn recording_source_fallback_selection() {
        assert_eq!(
            start_with_fallback(|| Ok("stream"), || panic!("unused")).unwrap(),
            "stream"
        );
        assert_eq!(
            start_with_fallback(|| Err(CaptureError::CaptureFailed), || Ok("shots")).unwrap(),
            "shots"
        );
        // A missing display isn't retried with the fallback.
        let r: Result<&str, _> =
            start_with_fallback(|| Err(CaptureError::NoDisplay(2)), || panic!("unused"));
        assert!(matches!(r, Err(CaptureError::NoDisplay(2))));
        // Both fail: the fallback's error is reported.
        let r: Result<&str, _> = start_with_fallback(
            || Err(CaptureError::CaptureFailed),
            || Err(CaptureError::ImageDataFailed),
        );
        assert!(matches!(r, Err(CaptureError::ImageDataFailed)));
    }

    #[test]
    fn recording_source_invalid_display_errors() {
        assert!(matches!(
            RecordingSource::start(99, Some(1920), 30),
            Err(CaptureError::NoDisplay(99))
        ));
    }

    /// Converting a Display P3 image must yield sRGB BGRA with alpha 255:
    /// pure P3 red is outside sRGB, so it clips to (255, 0, 0)-ish.
    #[test]
    fn p3_image_is_converted_to_srgb() {
        use objc2_core_graphics::{
            CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGImageAlphaInfo,
            CGImageByteOrderInfo,
        };
        let (w, h) = (4usize, 2usize);
        // BGRA, NoneSkipFirst: pure red in Display P3.
        let mut px = [0u8, 0, 255, 0].repeat(w * h);
        let p3_name = objc2_core_foundation::CFString::from_static_str("kCGColorSpaceDisplayP3");
        let p3 = CGColorSpace::with_name(Some(&p3_name)).expect("P3 colour space");
        let info = CGImageAlphaInfo::NoneSkipFirst.0 | CGImageByteOrderInfo::Order32Little.0;
        let ctx = unsafe {
            CGBitmapContextCreate(px.as_mut_ptr().cast(), w, h, 8, w * 4, Some(&p3), info)
        }
        .expect("bitmap context");
        let img = CGBitmapContextCreateImage(Some(&ctx)).expect("image");
        let rgba = cg_image_to_rgba(&img).unwrap();
        let p = rgba.get_pixel(0, 0);
        assert_eq!(p[3], 255, "alpha forced opaque");
        assert!(
            p[0] >= 250 && p[1] <= 10 && p[2] <= 10,
            "P3 red clips to sRGB red: {p:?}"
        );

        let bgra = cg_image_to_bgra(&img).unwrap();
        assert_eq!(bgra.bytes.len(), w * h * 4);
        assert_eq!(bgra.bytes[3], 255);
    }

    #[test]
    fn srgb_image_bytes_pass_through_unchanged() {
        use objc2_core_graphics::{
            CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGImageAlphaInfo,
            CGImageByteOrderInfo,
        };
        let (w, h) = (3usize, 2usize);
        let mut px: Vec<u8> = (0..w * h)
            .flat_map(|i| [i as u8 * 10, 100, 200, 255])
            .collect();
        let srgb = CGColorSpace::with_name(Some(srgb_color_space_name())).unwrap();
        let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
        let ctx = unsafe {
            CGBitmapContextCreate(px.as_mut_ptr().cast(), w, h, 8, w * 4, Some(&srgb), info)
        }
        .unwrap();
        let img = CGBitmapContextCreateImage(Some(&ctx)).unwrap();
        let bgra = cg_image_to_bgra(&img).unwrap();
        assert_eq!(bgra.bytes, px);
    }

    #[test]
    fn test_capture_fullscreen_main() {
        let img = capture_fullscreen(0);
        if let Ok(img) = img {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_main() {
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let img = capture_region(0, region);
        if let Ok(img) = img {
            assert_eq!(img.width(), 100);
            assert_eq!(img.height(), 100);
        }
    }

    #[test]
    fn test_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_shareable_displays() {
        let displays = get_shareable_displays(SCK_CONTENT_TIMEOUT);
        if let Some(displays) = displays {
            assert!(!displays.is_empty());
        }
    }

    #[test]
    fn compute_output_size_invalid_display_errors() {
        // Display index past the end must be NoDisplay, never a panic or a
        // bogus (0,0) that the recorder would then reject downstream.
        let result = compute_recording_output_size(99, Some(1920));
        assert!(matches!(
            result,
            Err(CaptureError::NoDisplay(99) | CaptureError::CaptureFailed)
        ));
    }

    #[test]
    fn compute_output_size_native_dimensions_are_even() {
        // libx264 yuv420p needs even dimensions; the `& !1` masking must hold
        // for the no-cap (native) path. Skips if no display / no permission.
        let Ok((w, h)) = compute_recording_output_size(0, None) else {
            return;
        };
        assert_eq!(w & 1, 0, "native width must be even, got {w}");
        assert_eq!(h & 1, 0, "native height must be even, got {h}");
    }

    #[test]
    fn compute_output_size_respects_max_dimension_cap() {
        // With a small cap, neither dimension may exceed it, and both stay even.
        let cap = 640u32;
        let Ok((w, h)) = compute_recording_output_size(0, Some(cap)) else {
            return;
        };
        assert!(w <= cap, "width {w} exceeded cap {cap}");
        assert!(h <= cap, "height {h} exceeded cap {cap}");
        assert_eq!(w & 1, 0);
        assert_eq!(h & 1, 0);
    }

    #[test]
    fn compute_output_size_huge_cap_does_not_upscale() {
        // A cap larger than the native size must clamp the scale to 1.0 (no
        // upscaling) — output should equal the native (even-masked) path.
        let Ok((capped_w, capped_h)) = compute_recording_output_size(0, Some(100_000)) else {
            return;
        };
        let Ok((native_w, native_h)) = compute_recording_output_size(0, None) else {
            return;
        };
        assert_eq!((capped_w, capped_h), (native_w, native_h));
    }
}
