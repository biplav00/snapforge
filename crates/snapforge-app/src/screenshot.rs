//! Screenshot use case — capture, optionally copy to clipboard, save, optionally
//! add to history.

use crate::AppError;
use snapforge_capture::capture;
use snapforge_domain::{CaptureFormat, Rect};
use snapforge_encode::format;
use snapforge_storage::{clipboard, history::ScreenshotHistory};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ScreenshotRequest {
    pub display: usize,
    /// None = fullscreen capture, Some = capture this rect.
    pub region: Option<Rect>,
    pub output_path: PathBuf,
    pub format: CaptureFormat,
    pub quality: u8,
    pub copy_to_clipboard: bool,
    pub add_to_history: bool,
}

#[derive(Debug, Clone)]
pub struct ScreenshotResult {
    pub saved_path: PathBuf,
}

/// Take a screenshot end-to-end: capture → (optional) clipboard → save →
/// (optional) history index. Returns the path that was written.
pub fn take_screenshot(req: ScreenshotRequest) -> Result<ScreenshotResult, AppError> {
    if req.output_path.as_os_str().is_empty() {
        return Err(AppError::InvalidRequest("output_path is empty".into()));
    }

    let image = match req.region {
        Some(r) => {
            if r.width == 0 || r.height == 0 {
                return Err(AppError::InvalidRequest(
                    "region has zero width or height".into(),
                ));
            }
            capture::capture_region(req.display, r)?
        }
        None => capture::capture_fullscreen(req.display)?,
    };

    if req.copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    format::save_image(&image, &req.output_path, req.format, req.quality)?;

    if req.add_to_history {
        // History indexing is best-effort — if loading or writing the index
        // fails, the screenshot is already on disk and the caller still cares
        // about the saved_path more than the index. Surface the error anyway
        // so the FFI can log it; the caller decides what to do.
        let mut hist = ScreenshotHistory::load()?;
        hist.add_entry(&req.output_path.to_string_lossy())?;
    }

    Ok(ScreenshotResult {
        saved_path: req.output_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_output_path() {
        let req = ScreenshotRequest {
            display: 0,
            region: None,
            output_path: PathBuf::new(),
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let err = take_screenshot(req).unwrap_err();
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_zero_size_region() {
        let req = ScreenshotRequest {
            display: 0,
            region: Some(Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }),
            output_path: PathBuf::from("/tmp/snapforge-zero.png"),
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let err = take_screenshot(req).unwrap_err();
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }

    // Happy-path screenshot needs a real display + Screen Recording permission,
    // so it's gated behind `#[ignore]` for CI but useful as a manual smoke test.
    #[ignore]
    #[test]
    fn happy_path_fullscreen_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("shot.png");
        let req = ScreenshotRequest {
            display: 0,
            region: None,
            output_path: out.clone(),
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let res = take_screenshot(req).expect("capture should succeed on a real display");
        assert_eq!(res.saved_path, out);
        assert!(out.exists());
    }
}
