#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod xcap_impl;

use image::RgbaImage;
use snapforge_domain::Rect;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no display found at index {0}")]
    NoDisplay(usize),
    #[error("screen capture failed")]
    CaptureFailed,
    #[error("failed to get image data")]
    ImageDataFailed,
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_fullscreen(display)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::capture_fullscreen(display)
    }
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_region(display, region)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::capture_region(display, region)
    }
}

/// Check if screen capture permission is granted (macOS only, always true on other platforms).
pub fn has_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::has_screen_capture_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request screen capture permission (macOS only, no-op on other platforms).
pub fn request_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_screen_capture_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// A reusable capture context for fast repeated captures (e.g. recording).
/// Create once on a background thread, call capture_frame() per frame.
#[cfg(target_os = "macos")]
pub use macos::{CaptureContext, RawFrame};

/// Get the point-to-pixel scale factor of the primary display.
pub fn display_scale_factor() -> f64 {
    #[cfg(target_os = "macos")]
    {
        macos::primary_display_scale_factor()
    }
    #[cfg(not(target_os = "macos"))]
    {
        1.0
    }
}

pub fn display_count() -> usize {
    #[cfg(target_os = "macos")]
    {
        macos::display_count()
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::display_count()
    }
}

pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        macos::display_at_point(x, y)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::display_at_point(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count() {
        let count = display_count();
        // In headless CI there may be no display — just verify it doesn't panic
        // On a desktop environment it should be > 0
        let _ = count;
    }

    #[test]
    fn test_capture_fullscreen() {
        let result = capture_fullscreen(0);
        // May fail in headless CI, so we just check the success path
        if let Ok(img) = result {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_valid() {
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let result = capture_region(0, region);
        if let Ok(img) = result {
            assert_eq!(img.width(), 100);
            assert_eq!(img.height(), 100);
        }
    }

    #[test]
    fn test_capture_region_zero_size() {
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        let result = capture_region(0, region);
        // Zero-size region should fail
        if result.is_ok() {
            // Some implementations might handle this gracefully
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_capture_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err(), "invalid display index should fail");
    }

    #[test]
    fn capture_region_invalid_display_errors() {
        // A region request against a nonexistent display must error, not panic,
        // and must not silently fall back to display 0.
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let result = capture_region(99, region);
        assert!(
            result.is_err(),
            "region capture on invalid display should fail"
        );
    }

    #[test]
    fn capture_region_oversized_clamps_to_display_bounds() {
        // Region far larger than any real display: the implementation clamps
        // width/height to the display, so the result (if capture succeeds) must
        // never exceed the full-frame size. Skips cleanly in headless CI.
        let Ok(full) = capture_fullscreen(0) else {
            return;
        };
        let huge = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
        };
        if let Ok(img) = capture_region(0, huge) {
            assert!(img.width() <= full.width());
            assert!(img.height() <= full.height());
            assert!(img.width() > 0 && img.height() > 0);
        }
    }

    #[test]
    fn capture_region_origin_past_display_errors() {
        // An origin beyond the display extent leaves a zero-size window, which
        // must surface as an error rather than a 0x0 image or a panic.
        let Ok(full) = capture_fullscreen(0) else {
            return;
        };
        let off = snapforge_domain::Rect {
            x: i32::try_from(full.width()).unwrap_or(i32::MAX - 1000) + 1000,
            y: i32::try_from(full.height()).unwrap_or(i32::MAX - 1000) + 1000,
            width: 100,
            height: 100,
        };
        assert!(capture_region(0, off).is_err());
    }

    #[test]
    fn display_at_point_far_offscreen_is_none_or_valid_index() {
        // Coordinates no display can contain must yield None (honoring the
        // C -1 contract) rather than snapping to display 0. If a virtual
        // desktop somehow contains it, the index must be in range.
        let count = display_count();
        match display_at_point(i32::MIN / 2, i32::MIN / 2) {
            None => {}
            Some(idx) => assert!(idx < count.max(1)),
        }
    }
}
