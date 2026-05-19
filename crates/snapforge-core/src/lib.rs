//! Backward-compatible facade over the split snapforge crates.
//!
//! Phase 2A of the architecture migration relocated the source modules into
//! four focused crates (`snapforge-domain`, `snapforge-capture`,
//! `snapforge-encode`, `snapforge-storage`). This crate keeps the old
//! `snapforge_core::*` import paths working so the FFI and any external
//! callers don't need to change. The thin convenience orchestrators
//! (`screenshot_fullscreen`, `screenshot_region`) still live here because
//! they glue capture + encode + clipboard together.

// Re-export the sub-crates' top-level modules under their original paths.
// `snapforge_core::capture::foo` → `snapforge_capture::capture::foo`, etc.
pub use snapforge_capture::capture;
pub use snapforge_capture::clicks;
pub use snapforge_encode::format;
pub use snapforge_encode::record;
pub use snapforge_storage::clipboard;
pub use snapforge_storage::config;
pub use snapforge_storage::history;

// Domain types were previously `snapforge_core::types::*`. Keep a `types`
// module alias so any `snapforge_core::types::Rect` import resolves.
pub use snapforge_domain as types;

use snapforge_domain::{CaptureFormat, Rect};
use std::path::{Path, PathBuf};
use thiserror::Error;

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
pub fn screenshot_fullscreen(
    display: usize,
    save_path: &Path,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_fullscreen(display)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    self::format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.to_path_buf())
}

/// Capture a specific region, save to file, optionally copy to clipboard.
pub fn screenshot_region(
    display: usize,
    region: Rect,
    save_path: &Path,
    format: CaptureFormat,
    quality: u8,
    copy_to_clipboard: bool,
) -> Result<PathBuf, ScreenError> {
    let image = capture::capture_region(display, region)?;

    if copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    self::format::save_image(&image, save_path, format, quality)?;
    Ok(save_path.to_path_buf())
}
