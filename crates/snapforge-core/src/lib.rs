pub mod types;
pub mod capture;
pub mod format;
pub mod clipboard;
pub mod config;
pub mod record;

use std::path::PathBuf;
use thiserror::Error;
use types::{CaptureFormat, Rect};

#[derive(Debug, Error)]
pub enum ScreenError {
    #[error("capture error: {0}")]
    Capture(#[from] capture::CaptureError),
    #[error("format error: {0}")]
    Format(#[from] format::FormatError),
    #[error("clipboard error: {0}")]
    Clipboard(#[from] clipboard::ClipboardError),
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("recording error: {0}")]
    Recording(#[from] record::RecordError),
}

/// Capture fullscreen screenshot, save to file, optionally copy to clipboard.
/// Returns the path where the file was saved.
pub fn screenshot_fullscreen(
    display: usize,
    save_path: &PathBuf,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_fullscreen(display)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.clone())
}

/// Capture a specific region, save to file, optionally copy to clipboard.
/// Returns the path where the file was saved.
pub fn screenshot_region(
    display: usize,
    region: Rect,
    save_path: &PathBuf,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_region(display, region)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.clone())
}
