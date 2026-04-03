#[cfg(target_os = "macos")]
pub mod macos;

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
        let _ = display;
        Err(CaptureError::CaptureFailed)
    }
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_region(display, region)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (display, region);
        Err(CaptureError::CaptureFailed)
    }
}

pub fn display_count() -> usize {
    #[cfg(target_os = "macos")]
    {
        macos::display_count()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
