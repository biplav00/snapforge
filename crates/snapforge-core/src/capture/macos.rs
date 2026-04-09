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
    rx.recv().ok().flatten()
}

/// Get the SCDisplay at the given index.
fn get_sc_display(display: usize) -> Result<Retained<SCDisplay>, CaptureError> {
    let displays = get_shareable_displays().ok_or(CaptureError::CaptureFailed)?;
    if display >= displays.len() {
        return Err(CaptureError::NoDisplay(display));
    }
    Ok(displays.objectAtIndex(display))
}

pub fn display_count() -> usize {
    get_shareable_displays().map_or(0, |d| d.len())
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let sc_display = get_sc_display(display)?;

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
        let w = sc_display.width() as usize;
        let h = sc_display.height() as usize;
        config.setWidth(w);
        config.setHeight(h);
        config.setShowsCursor(false);
        config
    };

    let (tx, rx) = mpsc::channel::<Option<RgbaImage>>();
    unsafe {
        SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
            &filter,
            &config,
            Some(&*block2::RcBlock::new(
                move |cg_image: *mut objc2_core_graphics::CGImage,
                      error: *mut objc2_foundation::NSError| {
                    if error.is_null() && !cg_image.is_null() {
                        let img = &*cg_image;
                        let _ = tx.send(cg_image_to_rgba(img).ok());
                    } else {
                        let _ = tx.send(None);
                    }
                },
            )),
        );
    }

    rx.recv()
        .map_err(|_| CaptureError::CaptureFailed)?
        .ok_or(CaptureError::CaptureFailed)
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
    let bytes_per_row = CGImage::bytes_per_row(Some(cg_image));

    let provider = CGImage::data_provider(Some(cg_image)).ok_or(CaptureError::ImageDataFailed)?;
    let cf_data = CGDataProvider::data(Some(&provider)).ok_or(CaptureError::ImageDataFailed)?;
    let raw_bytes: &[u8] = unsafe { cf_data.as_bytes_unchecked() };

    let expected_pixels = (width * height) as usize;
    let expected_bytes = expected_pixels * 4;

    if height == 0 || width == 0 {
        return Err(CaptureError::ImageDataFailed);
    }

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
    fn test_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err());
    }
}
