//! Screenshot use case — capture, optionally copy to clipboard, save, optionally
//! add to history.

use crate::AppError;
use image::RgbaImage;
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

/// Request for [`save_prerendered`] — caller already has fully-composited RGBA
/// pixel bytes (e.g. Qt has rendered annotations on top of a captured
/// backdrop) and wants to save / copy / index that result. We do **not**
/// re-capture; the bytes are taken at face value.
#[derive(Debug, Clone)]
pub struct SavePrerenderedRequest {
    /// RGBA8 byte buffer, length must equal `width * height * 4`.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// `None` means clipboard-only — nothing is written to disk and
    /// `add_to_history` is ignored.
    pub output_path: Option<PathBuf>,
    /// Encoding format used when `output_path` is `Some`. Ignored otherwise.
    pub format: CaptureFormat,
    pub quality: u8,
    pub copy_to_clipboard: bool,
    /// Only honoured when a file was actually written.
    pub add_to_history: bool,
}

#[derive(Debug, Clone)]
pub struct SavePrerenderedResult {
    /// The file that was written, or `None` for a clipboard-only request.
    pub saved_path: Option<PathBuf>,
}

/// Save / copy / index a caller-supplied RGBA bitmap.
///
/// This is the use-case for callers that have already composited their final
/// image (Qt annotation flow) — re-capturing inside Rust would drop the
/// annotations, so we accept the bytes as-is. If `output_path` is `None` the
/// function is clipboard-only and no file is touched.
pub fn save_prerendered(
    req: SavePrerenderedRequest,
) -> Result<SavePrerenderedResult, AppError> {
    let expected = (req.width as usize)
        .checked_mul(req.height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| AppError::InvalidRequest("width*height*4 overflows".into()))?;
    if req.width == 0 || req.height == 0 {
        return Err(AppError::InvalidRequest(
            "width or height is zero".into(),
        ));
    }
    if req.rgba.len() != expected {
        return Err(AppError::InvalidRequest(format!(
            "rgba length {} does not match width*height*4 ({})",
            req.rgba.len(),
            expected
        )));
    }
    let image = RgbaImage::from_raw(req.width, req.height, req.rgba)
        .ok_or_else(|| AppError::InvalidRequest("RgbaImage::from_raw failed".into()))?;

    if req.copy_to_clipboard {
        clipboard::copy_image_to_clipboard(&image)?;
    }

    let saved_path = if let Some(path) = req.output_path {
        if path.as_os_str().is_empty() {
            return Err(AppError::InvalidRequest("output_path is empty".into()));
        }
        format::save_image(&image, &path, req.format, req.quality)?;
        if req.add_to_history {
            let mut hist = ScreenshotHistory::load()?;
            hist.add_entry(&path.to_string_lossy())?;
        }
        Some(path)
    } else {
        None
    };

    Ok(SavePrerenderedResult { saved_path })
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

    fn solid_rgba(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            v.extend_from_slice(&[255, 0, 0, 255]);
        }
        v
    }

    #[test]
    fn save_prerendered_rejects_length_mismatch() {
        let req = SavePrerenderedRequest {
            rgba: vec![0u8; 10],
            width: 4,
            height: 4,
            output_path: None,
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let err = save_prerendered(req).unwrap_err();
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }

    #[test]
    fn save_prerendered_rejects_zero_dim() {
        let req = SavePrerenderedRequest {
            rgba: Vec::new(),
            width: 0,
            height: 0,
            output_path: None,
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let err = save_prerendered(req).unwrap_err();
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }

    #[test]
    fn save_prerendered_writes_file_when_path_given() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("pre.png");
        let req = SavePrerenderedRequest {
            rgba: solid_rgba(8, 8),
            width: 8,
            height: 8,
            output_path: Some(out.clone()),
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let res = save_prerendered(req).expect("save");
        assert_eq!(res.saved_path.as_deref(), Some(out.as_path()));
        assert!(out.exists());
    }

    #[test]
    fn save_prerendered_clipboard_only_writes_nothing() {
        // copy_to_clipboard is false so this never touches NSPasteboard; we
        // just verify the None path returns None and writes no file.
        let req = SavePrerenderedRequest {
            rgba: solid_rgba(2, 2),
            width: 2,
            height: 2,
            output_path: None,
            format: CaptureFormat::Png,
            quality: 90,
            copy_to_clipboard: false,
            add_to_history: false,
        };
        let res = save_prerendered(req).expect("noop");
        assert!(res.saved_path.is_none());
    }
}
