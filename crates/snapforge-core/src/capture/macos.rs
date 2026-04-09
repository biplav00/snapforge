use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

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

extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGDisplayCopyDisplayMode(display: u32) -> *const std::ffi::c_void;
    fn CGDisplayModeGetPixelWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeGetWidth(mode: *const std::ffi::c_void) -> usize;
    fn CGDisplayModeRelease(mode: *const std::ffi::c_void);
    fn CGMainDisplayID() -> u32;
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
fn display_pixel_size(display_id: u32) -> (usize, usize) {
    unsafe {
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            // Fallback: use SCDisplay point dimensions * 2
            return (0, 0);
        }
        let w = CGDisplayModeGetPixelWidth(mode);
        let h = CGDisplayModeGetPixelHeight(mode);
        CGDisplayModeRelease(mode);
        (w, h)
    }
}

/// Cached permission state — once granted, skip re-checking.
static PERMISSION_GRANTED: AtomicBool = AtomicBool::new(false);

/// Check if screen recording permission is granted.
/// Uses cached result after first success, falls back to SCK content query.
pub fn has_screen_capture_permission() -> bool {
    if PERMISSION_GRANTED.load(Ordering::Relaxed) {
        return true;
    }
    if unsafe { CGPreflightScreenCaptureAccess() } {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        return true;
    }
    // Preflight can return stale false. Try fetching SCK content as proof of permission.
    if get_shareable_displays().is_some() {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

/// Request screen recording permission. Returns true if granted.
pub fn request_screen_capture_permission() -> bool {
    let result = unsafe { CGRequestScreenCaptureAccess() };
    if result {
        PERMISSION_GRANTED.store(true, Ordering::Relaxed);
    }
    result
}

/// Fetch available displays via SCShareableContent (async → sync bridge).
fn get_shareable_displays() -> Option<Retained<NSArray<SCDisplay>>> {
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
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .ok()
        .flatten()
}

pub fn display_count() -> usize {
    get_shareable_displays().map_or(0, |d| d.len())
}

/// Capture the full screen for a given display index.
/// IMPORTANT: Must be called from a background thread, not the main thread.
/// SCK completion handlers need the main RunLoop to be free.
pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
    if display >= displays.len() {
        return Err(CaptureError::NoDisplay(display));
    }
    let sc_display = displays.objectAtIndex(display);
    let display_id = unsafe { sc_display.displayID() };

    let filter = unsafe {
        let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &sc_display,
            &excluded,
        )
    };

    let config = unsafe {
        let config = SCStreamConfiguration::new();
        let (w, h) = display_pixel_size(display_id);
        if w > 0 && h > 0 {
            config.setWidth(w);
            config.setHeight(h);
        }
        config.setShowsCursor(false);
        config
    };

    capture_with_filter(filter.as_ref(), config.as_ref())
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
        let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
        if display >= displays.len() {
            return Err(CaptureError::NoDisplay(display));
        }
        let sc_display = displays.objectAtIndex(display);
        let display_id = unsafe { sc_display.displayID() };

        let filter = unsafe {
            let excluded: Retained<NSArray<SCWindow>> = NSArray::new();
            SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &sc_display,
                &excluded,
            )
        };

        // Compute output dimensions
        let (native_w, native_h) = display_pixel_size(display_id);
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
