use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_foundation::NSArray;
use objc2_screen_capture_kit::{
    SCContentFilter, SCDisplay, SCScreenshotManager, SCShareableContent, SCStreamConfiguration,
    SCWindow,
};

use crate::types::Rect;

use super::CaptureError;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPointRaw {
    x: f64,
    y: f64,
}

type CGDisplayReconfigurationCallBack =
    unsafe extern "C" fn(display: u32, flags: u32, user_info: *mut std::ffi::c_void);

extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGDisplayCopyDisplayMode(display: u32) -> *const std::ffi::c_void;
    fn CGDisplayModeGetPixelWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeGetWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeRelease(mode: *const std::ffi::c_void);
    fn CGMainDisplayID() -> u32;
    /// Returns the number of displays that contain `point` and fills `displays` with their IDs.
    fn CGGetDisplaysWithPoint(
        point: CGPointRaw,
        max_displays: u32,
        displays: *mut u32,
        matching_count: *mut u32,
    ) -> i32;
    fn CGDisplayRegisterReconfigurationCallback(
        callback: CGDisplayReconfigurationCallBack,
        user_info: *mut std::ffi::c_void,
    ) -> i32;
}

/// Bumped whenever CoreGraphics tells us the display topology changed. Any
/// cache keyed on `CGDirectDisplayID` has to invalidate itself when this moves.
static DISPLAY_TOPOLOGY_VERSION: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" fn display_reconfig_callback(
    _display: u32,
    _flags: u32,
    _user_info: *mut std::ffi::c_void,
) {
    DISPLAY_TOPOLOGY_VERSION.fetch_add(1, Ordering::SeqCst);
}

fn ensure_display_reconfig_registered() {
    // SAFETY/NOTE: CGDisplayRegisterReconfigurationCallback delivers its
    // callback on the main run loop. When snapforge runs without a main run
    // loop spinning (e.g. a headless CLI invocation), the callback will never
    // fire, DISPLAY_TOPOLOGY_VERSION stays at 0, and the capture cache falls
    // back to its TTL-based invalidation. That's acceptable — the TTL exists
    // precisely for this case.
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        unsafe {
            let err = CGDisplayRegisterReconfigurationCallback(
                display_reconfig_callback,
                std::ptr::null_mut(),
            );
            if err != 0 {
                eprintln!(
                    "[snapforge] CGDisplayRegisterReconfigurationCallback failed: {}",
                    err
                );
            }
        }
    });
}

/// Get the point-to-pixel scale factor of the primary display.
/// Returns 1.0 on non-Retina, 2.0 on Retina (typically).
pub fn primary_display_scale_factor() -> f64 {
    unsafe {
        let display_id = CGMainDisplayID();
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return 2.0;
        }
        let pixel_w = CGDisplayModeGetPixelWidth(mode);
        let point_w = CGDisplayModeGetWidth(mode);
        CGDisplayModeRelease(mode);
        if point_w == 0 {
            2.0
        } else {
            pixel_w as f64 / point_w as f64
        }
    }
}

/// Get the native pixel width of the primary display.
pub fn primary_display_pixel_width() -> usize {
    unsafe {
        let display_id = CGMainDisplayID();
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return 0;
        }
        let w = CGDisplayModeGetPixelWidth(mode);
        CGDisplayModeRelease(mode);
        w
    }
}

/// Get the native pixel height of the primary display.
pub fn primary_display_pixel_height() -> usize {
    unsafe {
        let display_id = CGMainDisplayID();
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return 0;
        }
        let h = CGDisplayModeGetPixelHeight(mode);
        CGDisplayModeRelease(mode);
        h
    }
}

/// Get the true backing pixel dimensions for a display (Retina-aware).
/// Returns Err if CGDisplayCopyDisplayMode fails.
fn display_pixel_size(display_id: u32) -> Result<(usize, usize), CaptureError> {
    unsafe {
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return Err(CaptureError::CaptureFailed);
        }
        let w = CGDisplayModeGetPixelWidth(mode);
        let h = CGDisplayModeGetPixelHeight(mode);
        CGDisplayModeRelease(mode);
        Ok((w, h))
    }
}

/// Cached permission state — once granted, skip re-checking.
static PERMISSION_GRANTED: AtomicBool = AtomicBool::new(false);

/// Latches once we've established this process can't see screen-recording
/// permission. macOS only applies a freshly-granted Screen Recording grant to
/// processes started *after* the grant; a running process keeps getting the
/// system prompt every time it touches SCK. Latching means we stop pinging
/// SCK after the first denial of this session, so the user isn't spammed with
/// repeated TCC dialogs. Cleared if `CGPreflightScreenCaptureAccess` ever
/// flips to true (e.g. some external recovery).
static PERMISSION_DENIED_LATCH: AtomicBool = AtomicBool::new(false);

/// Set after `request_screen_capture_permission` triggers the system dialog
/// once. Subsequent requests in the same process are no-ops to avoid stacking
/// dialogs that the user can't dismiss into a usable state without a restart.
static PERMISSION_REQUESTED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

/// Check if screen recording permission is granted.
/// Uses cached result after first success, falls back to a *single* SCK
/// content query per session if preflight is stale-false.
pub fn has_screen_capture_permission() -> bool {
    if PERMISSION_GRANTED.load(Ordering::Relaxed) {
        return true;
    }
    if unsafe { CGPreflightScreenCaptureAccess() } {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        PERMISSION_DENIED_LATCH.store(false, Ordering::Relaxed);
        return true;
    }
    if PERMISSION_DENIED_LATCH.load(Ordering::Relaxed) {
        // Already determined denied this session — don't ping SCK again.
        return false;
    }
    // First check this session with preflight false. Try SCK once as proof.
    // Failure here will set the latch via get_shareable_displays.
    if get_shareable_displays().is_some() {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

/// Request screen recording permission. Triggers the system dialog the first
/// time it's called per session and returns whatever Core Graphics reports.
/// Subsequent calls in the same process are no-ops (return false) — macOS
/// applies the grant only to *new* processes, so re-prompting the same
/// process doesn't help and just spams the user.
pub fn request_screen_capture_permission() -> bool {
    if PERMISSION_GRANTED.load(Ordering::Relaxed) {
        return true;
    }
    if PERMISSION_REQUESTED_THIS_SESSION.swap(true, Ordering::Relaxed) {
        return false;
    }
    let result = unsafe { CGRequestScreenCaptureAccess() };
    if result {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        PERMISSION_DENIED_LATCH.store(false, Ordering::Relaxed);
    }
    result
}

/// 2-second TTL cache for the display ID list. The SCK `Retained<NSArray<...>>`
/// isn't `Send`, so we cache only what's safely shareable: the CGDirectDisplayIDs
/// in their SCK ordering. Callers that need the NSArray itself still re-query.
///
/// Entry tuple: (timestamp, topology_version_at_capture, ids).
static DISPLAY_IDS_CACHE: Mutex<Option<(Instant, u64, Vec<u32>)>> = Mutex::new(None);
const DISPLAYS_CACHE_TTL: Duration = Duration::from_secs(2);

/// Fetch available displays via SCShareableContent (async → sync bridge).
/// Returns `None` if SCK doesn't respond within 2s. An empty/None result
/// means callers should surface a permission or daemon error — the SCK
/// daemon is the first thing to wedge on permission loss.
///
/// Skips the SCK call entirely once the per-session permission-denied latch
/// is set: SCShareableContent triggers the macOS TCC dialog every time it's
/// called from a process that hasn't been granted Screen Recording, so
/// repeating the call after a known denial just spams the user.
fn get_shareable_displays() -> Option<Retained<NSArray<SCDisplay>>> {
    if PERMISSION_DENIED_LATCH.load(Ordering::Relaxed) {
        return None;
    }
    ensure_display_reconfig_registered();

    let (tx, rx) = mpsc::channel();
    let block = block2::RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut objc2_foundation::NSError| {
            if error.is_null() && !content.is_null() {
                let displays = unsafe { (*content).displays() };
                let _ = tx.send(Some(displays));
            } else {
                let _ = tx.send(None::<Retained<NSArray<SCDisplay>>>);
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
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .ok()
        .flatten();

    // Refresh the ID cache whenever we do a live query.
    if let Some(ref displays) = result {
        let mut ids = Vec::with_capacity(displays.len());
        for i in 0..displays.len() {
            let sc = displays.objectAtIndex(i);
            ids.push(unsafe { sc.displayID() });
        }
        if let Ok(mut guard) = DISPLAY_IDS_CACHE.lock() {
            *guard = Some((
                Instant::now(),
                DISPLAY_TOPOLOGY_VERSION.load(Ordering::SeqCst),
                ids,
            ));
        }
    } else if !unsafe { CGPreflightScreenCaptureAccess() } {
        // SCK gave us nothing and the system says permission isn't granted.
        // Latch so we don't trigger another TCC prompt on the next call.
        // The latch clears if preflight ever flips back to true.
        PERMISSION_DENIED_LATCH.store(true, Ordering::Relaxed);
    }

    result
}

/// Fast display ID list — uses cached IDs if fresh AND the display topology
/// version matches, else a live SCK query. Safe across threads (unlike
/// `Retained<NSArray<...>>`).
fn get_display_ids_cached() -> Vec<u32> {
    ensure_display_reconfig_registered();
    let current_ver = DISPLAY_TOPOLOGY_VERSION.load(Ordering::SeqCst);
    if let Ok(guard) = DISPLAY_IDS_CACHE.lock() {
        if let Some((ts, ver, ids)) = guard.as_ref() {
            if *ver == current_ver && ts.elapsed() < DISPLAYS_CACHE_TTL {
                return ids.clone();
            }
        }
    }
    // Miss → trigger a live query which refreshes the cache as a side-effect.
    match get_shareable_displays() {
        Some(arr) => {
            let mut ids = Vec::with_capacity(arr.len());
            for i in 0..arr.len() {
                let sc = arr.objectAtIndex(i);
                ids.push(unsafe { sc.displayID() });
            }
            ids
        }
        None => Vec::new(),
    }
}

pub fn display_count() -> usize {
    let ids = get_display_ids_cached();
    if ids.is_empty() {
        // Empty list most likely means Screen Recording permission has been
        // revoked (SCK then stalls/refuses). Surface a specific hint so users
        // don't chase a ghost capture bug.
        if unsafe { !CGPreflightScreenCaptureAccess() } {
            eprintln!("[capture] screen recording permission not granted");
        } else {
            eprintln!("[capture] display_count: no displays found (SCK query failed?)");
        }
    }
    ids.len()
}

/// Map a screen point (in global point coordinates) to the display index
/// in our `get_shareable_displays()` ordering. Returns None if no display
/// contains the point or the mapping fails.
pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    unsafe {
        let mut matches: [u32; 8] = [0; 8];
        let mut count: u32 = 0;
        let status = CGGetDisplaysWithPoint(
            CGPointRaw {
                x: f64::from(x),
                y: f64::from(y),
            },
            matches.len() as u32,
            matches.as_mut_ptr(),
            &mut count,
        );
        if status != 0 || count == 0 {
            return Some(0);
        }
        let target_id = matches[0];
        let ids = get_display_ids_cached();
        for (i, id) in ids.iter().enumerate() {
            if *id == target_id {
                return Some(i);
            }
        }
        Some(0)
    }
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
    let cached_ids = get_display_ids_cached();
    let target_id = cached_ids
        .get(display)
        .copied()
        .ok_or(CaptureError::NoDisplay(display))?;

    let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
    let mut sc_display_opt = None;
    for i in 0..displays.len() {
        let d = displays.objectAtIndex(i);
        if unsafe { d.displayID() } == target_id {
            sc_display_opt = Some(d);
            break;
        }
    }
    let sc_display = sc_display_opt.ok_or(CaptureError::NoDisplay(display))?;
    let display_id = target_id;

    let filter = unsafe {
        let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &sc_display,
            &excluded,
        )
    };

    let (w, h) = display_pixel_size(display_id)?;
    let config = unsafe {
        let config = SCStreamConfiguration::new();
        if w > 0 && h > 0 {
            config.setWidth(w);
            config.setHeight(h);
        }
        config.setShowsCursor(false);
        config
    };

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
    // Resolve display index → CGDirectDisplayID via cached SCK query.
    let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
    if display >= displays.len() {
        return Err(CaptureError::NoDisplay(display));
    }
    let sc_display = displays.objectAtIndex(display);
    let display_id = unsafe { sc_display.displayID() };

    let (native_w, native_h) = display_pixel_size(display_id)?;
    let (out_w, out_h) = if let Some(max_dim) = max_dimension {
        if native_w > 0 && native_h > 0 {
            let scale = (max_dim as f64 / native_w.max(native_h) as f64).min(1.0);
            (
                ((native_w as f64 * scale) as usize) & !1,
                ((native_h as f64 * scale) as usize) & !1,
            )
        } else {
            (0, 0)
        }
    } else {
        (native_w & !1, native_h & !1)
    };
    Ok((out_w as u32, out_h as u32))
}

/// Capture a single frame using a pre-built filter and config.
fn capture_with_filter(
    filter: &SCContentFilter,
    config: &SCStreamConfiguration,
) -> Result<RgbaImage, CaptureError> {
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

    rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)
}

/// A reusable capture context that holds pre-built SCK filter and config.
/// Create once, call `capture_frame()` repeatedly for recording.
/// IMPORTANT: Must be used on the same thread it was created on (ObjC objects are not Send).
pub struct CaptureContext {
    filter: Retained<SCContentFilter>,
    config: Retained<SCStreamConfiguration>,
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
        let cached_ids = get_display_ids_cached();
        let target_id = cached_ids
            .get(display)
            .copied()
            .ok_or(CaptureError::NoDisplay(display))?;

        let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
        let mut sc_display_opt = None;
        for i in 0..displays.len() {
            let d = displays.objectAtIndex(i);
            if unsafe { d.displayID() } == target_id {
                sc_display_opt = Some(d);
                break;
            }
        }
        let sc_display = sc_display_opt.ok_or(CaptureError::NoDisplay(display))?;
        let display_id = target_id;

        let filter = unsafe {
            let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
            SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &sc_display,
                &excluded,
            )
        };

        // Compute output dimensions
        let (native_w, native_h) = display_pixel_size(display_id)?;
        let (out_w, out_h) = if let Some(max_dim) = max_dimension {
            if native_w > 0 && native_h > 0 {
                let scale = (max_dim as f64 / native_w.max(native_h) as f64).min(1.0);
                (
                    ((native_w as f64 * scale) as usize) & !1,
                    ((native_h as f64 * scale) as usize) & !1,
                )
            } else {
                (0, 0)
            }
        } else {
            (native_w & !1, native_h & !1)
        };

        let config = unsafe {
            let config = SCStreamConfiguration::new();
            if out_w > 0 && out_h > 0 {
                config.setWidth(out_w);
                config.setHeight(out_h);
            }
            config.setShowsCursor(false);
            config
        };

        Ok(Self {
            filter,
            config,
            output_width: out_w as u32,
            output_height: out_h as u32,
        })
    }

    /// Capture a single frame as an RGBA image (slower — performs BGRA→RGBA conversion).
    pub fn capture_frame(&self) -> Result<RgbaImage, CaptureError> {
        capture_with_filter(self.filter.as_ref(), self.config.as_ref())
    }

    /// Capture a single frame and return raw BGRA bytes — fast path for recording.
    /// No per-pixel swap. Output is tightly packed `width * height * 4` bytes.
    pub fn capture_frame_raw_bgra(&self) -> Result<RawFrame, CaptureError> {
        capture_bgra_with_filter(self.filter.as_ref(), self.config.as_ref())
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

    rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)
}

/// Extract BGRA bytes from a CGImage without channel swapping.
fn cg_image_to_bgra(cg_image: &objc2_core_graphics::CGImage) -> Result<RawFrame, CaptureError> {
    use objc2_core_graphics::{CGDataProvider, CGImage};

    let width = CGImage::width(Some(cg_image)) as u32;
    let height = CGImage::height(Some(cg_image)) as u32;

    if width == 0 || height == 0 {
        return Err(CaptureError::ImageDataFailed);
    }

    let bytes_per_row = CGImage::bytes_per_row(Some(cg_image));

    let provider = CGImage::data_provider(Some(cg_image)).ok_or(CaptureError::ImageDataFailed)?;
    let cf_data = CGDataProvider::data(Some(&provider)).ok_or(CaptureError::ImageDataFailed)?;
    let raw_bytes: &[u8] = unsafe { cf_data.as_bytes_unchecked() };

    let row_bytes = (width as usize) * 4;
    let last_row_start = (height as usize - 1) * bytes_per_row;
    let min_required = last_row_start + row_bytes;
    if raw_bytes.len() < min_required {
        return Err(CaptureError::ImageDataFailed);
    }

    // If the source has no padding, we can copy the whole buffer in one shot.
    let total_bytes = (width as usize) * (height as usize) * 4;
    let mut bytes = vec![0u8; total_bytes];
    if bytes_per_row == row_bytes {
        bytes.copy_from_slice(&raw_bytes[..total_bytes]);
    } else {
        for y in 0..height as usize {
            let src_off = y * bytes_per_row;
            let dst_off = y * row_bytes;
            bytes[dst_off..dst_off + row_bytes]
                .copy_from_slice(&raw_bytes[src_off..src_off + row_bytes]);
        }
    }

    Ok(RawFrame {
        bytes,
        width,
        height,
    })
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    let full = capture_fullscreen(display)?;

    let x = region.x.max(0) as u32;
    let y = region.y.max(0) as u32;
    let w = region.width.min(full.width().saturating_sub(x));
    let h = region.height.min(full.height().saturating_sub(y));

    if w == 0 || h == 0 {
        return Err(CaptureError::CaptureFailed);
    }

    let cropped = image::imageops::crop_imm(&full, x, y, w, h).to_image();
    Ok(cropped)
}

fn cg_image_to_rgba(cg_image: &objc2_core_graphics::CGImage) -> Result<RgbaImage, CaptureError> {
    use objc2_core_graphics::{CGDataProvider, CGImage};

    let width = CGImage::width(Some(cg_image)) as u32;
    let height = CGImage::height(Some(cg_image)) as u32;

    if width == 0 || height == 0 {
        return Err(CaptureError::ImageDataFailed);
    }

    let bytes_per_row = CGImage::bytes_per_row(Some(cg_image));

    let provider = CGImage::data_provider(Some(cg_image)).ok_or(CaptureError::ImageDataFailed)?;
    let cf_data = CGDataProvider::data(Some(&provider)).ok_or(CaptureError::ImageDataFailed)?;
    let raw_bytes: &[u8] = unsafe { cf_data.as_bytes_unchecked() };

    let expected_pixels = (width * height) as usize;
    let expected_bytes = expected_pixels * 4;

    let last_row_start = (height as usize - 1) * bytes_per_row;
    let min_required = last_row_start + (width as usize) * 4;
    if raw_bytes.len() < min_required {
        return Err(CaptureError::ImageDataFailed);
    }

    let mut rgba_buf = vec![0u8; expected_bytes];

    for y in 0..height as usize {
        let row_start = y * bytes_per_row;
        let dst_row_start = y * (width as usize) * 4;
        let src_row = &raw_bytes[row_start..row_start + (width as usize) * 4];
        let dst_row = &mut rgba_buf[dst_row_start..dst_row_start + (width as usize) * 4];

        // Swap B and R channels: BGRA → RGBA
        for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            dst[0] = src[2]; // R
            dst[1] = src[1]; // G
            dst[2] = src[0]; // B
            dst[3] = src[3]; // A
        }
    }

    RgbaImage::from_raw(width, height, rgba_buf).ok_or(CaptureError::ImageDataFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count_nonzero() {
        assert!(display_count() > 0, "should detect at least one display");
    }

    #[test]
    fn test_permission_cache() {
        // Reset cache
        PERMISSION_GRANTED.store(false, Ordering::Relaxed);
        let first = has_screen_capture_permission();
        if first {
            // Should be cached now
            assert!(PERMISSION_GRANTED.load(Ordering::Relaxed));
            // Second call should use cache
            assert!(has_screen_capture_permission());
        }
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
        let region = crate::types::Rect {
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
        let displays = get_shareable_displays();
        if let Some(displays) = displays {
            assert!(displays.len() > 0);
        }
    }
}
