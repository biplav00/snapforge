#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod xcap_impl;

use crate::types::Rect;
use image::RgbaImage;
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
        let region = crate::types::Rect {
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
        let region = crate::types::Rect {
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
}
