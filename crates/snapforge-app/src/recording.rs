//! Recording use case — wraps the ffmpeg pipeline and optionally indexes the
//! finished file in the history store.

use crate::AppError;
use serde::Deserialize;
use snapforge_domain::Rect;
use snapforge_encode::record::{ffmpeg, RecordConfig};
use snapforge_storage::config::{RecordingFormat, RecordingQuality};
use snapforge_storage::history::is_incomplete_mp4;
use std::path::{Path, PathBuf};

/// The canonical recording-request schema — the FFI deserializes its JSON
/// straight into this. One serde definition, shared by every wire path.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RecordingRequest {
    pub display: usize,
    pub region: Option<Rect>,
    pub output_path: PathBuf,
    pub format: RecordingFormat,
    pub fps: u32,
    pub quality: RecordingQuality,
    pub ffmpeg_path: Option<PathBuf>,
    /// If true, after a successful `stop_recording` the output file is added
    /// to the screenshot history index (skipped if it's an incomplete mp4).
    pub add_to_history_on_stop: bool,
    /// If true, composite mouse-click ripples into the recording.
    pub show_clicks: bool,
}

impl Default for RecordingRequest {
    fn default() -> Self {
        Self {
            display: 0,
            region: None,
            output_path: PathBuf::new(),
            format: RecordingFormat::default(),
            fps: 30,
            quality: RecordingQuality::default(),
            ffmpeg_path: None,
            add_to_history_on_stop: false,
            show_clicks: false,
        }
    }
}

/// Opaque handle returned by [`start_recording`]. Keep it alive for the
/// duration of the recording, then pass it (by value or by reference) to the
/// lifecycle calls.
pub struct RecordingHandle {
    inner: Option<ffmpeg::RecordingHandle>,
    output_path: PathBuf,
    add_to_history_on_stop: bool,
}

impl RecordingHandle {
    /// The file this recording is actually writing. May differ from the
    /// requested `output_path`: if that name already existed, `start_recording`
    /// picked a `-N`-suffixed sibling instead of letting ffmpeg overwrite it.
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    /// `true` while the encode worker is running or paused; `false` once it
    /// has exited (including on error — `stop_recording` then returns that
    /// error) or after the handle was stopped.
    pub fn is_alive(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(ffmpeg::RecordingHandle::is_alive)
    }

    /// Pause the recording. Errors if the handle was stopped or the encode
    /// worker has already exited (the encode layer reports `false`).
    pub fn pause(&self) -> Result<(), AppError> {
        match self.inner.as_ref() {
            Some(h) if h.pause() => Ok(()),
            _ => Err(AppError::InvalidRequest("recording already stopped".into())),
        }
    }

    /// Resume the recording. Errors if the handle was stopped or the encode
    /// worker has already exited.
    pub fn resume(&self) -> Result<(), AppError> {
        match self.inner.as_ref() {
            Some(h) if h.resume() => Ok(()),
            _ => Err(AppError::InvalidRequest("recording already stopped".into())),
        }
    }
}

/// Begin a screen recording. Returns immediately — the encode pipeline runs on
/// a background thread inside the encode crate.
pub fn start_recording(req: RecordingRequest) -> Result<RecordingHandle, AppError> {
    if req.output_path.as_os_str().is_empty() {
        return Err(AppError::InvalidRequest("output_path is empty".into()));
    }
    if let Some(r) = req.region.as_ref() {
        if r.width == 0 || r.height == 0 {
            return Err(AppError::InvalidRequest(
                "region has zero width or height".into(),
            ));
        }
    }

    // Never let ffmpeg (`-y`) clobber an existing file: claim a unique name
    // up front (an empty placeholder that ffmpeg overwrites) and remove the
    // placeholder if the pipeline fails to start.
    let output_path = crate::claim_unique_path(&req.output_path)?;

    let cfg = RecordConfig {
        display: req.display,
        region: req.region,
        output_path: output_path.clone(),
        format: req.format,
        fps: req.fps,
        quality: req.quality,
        ffmpeg_path: req.ffmpeg_path,
        show_clicks: req.show_clicks,
    };
    let inner = match ffmpeg::start_recording(cfg) {
        Ok(h) => h,
        Err(e) => {
            remove_if_empty(&output_path);
            return Err(e.into());
        }
    };
    Ok(RecordingHandle {
        inner: Some(inner),
        output_path,
        add_to_history_on_stop: req.add_to_history_on_stop,
    })
}

/// Delete `path` if it exists and is zero bytes (an unused placeholder).
fn remove_if_empty(path: &Path) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() == 0) {
        let _ = std::fs::remove_file(path);
    }
}

/// Stop a recording and wait for the encoder to flush. Consumes the handle.
/// If `add_to_history_on_stop` was set on the original request, the output
/// file is added to the history index — best-effort: an empty output (any
/// format) or an incomplete mp4 is skipped, and an indexing failure is logged
/// rather than turned into an error (the recording itself succeeded).
pub fn stop_recording(mut handle: RecordingHandle) -> Result<(), AppError> {
    let inner = handle
        .inner
        .take()
        .ok_or_else(|| AppError::InvalidRequest("recording already stopped".into()))?;
    if let Err(e) = inner.stop() {
        remove_if_empty(&handle.output_path);
        return Err(e.into());
    }

    if handle.add_to_history_on_stop {
        let non_empty = std::fs::metadata(&handle.output_path).is_ok_and(|m| m.len() > 0);
        let path_str = handle.output_path.to_string_lossy().into_owned();
        if non_empty && !is_incomplete_mp4(&path_str) {
            crate::screenshot::index_best_effort(&handle.output_path);
        } else {
            tracing::warn!(
                "[recording] not indexing {}: output is empty or incomplete",
                handle.output_path.display()
            );
        }
    }
    Ok(())
}

/// See [`RecordingHandle::is_alive`].
pub fn is_recording_alive(handle: &RecordingHandle) -> bool {
    handle.is_alive()
}

pub fn pause_recording(handle: &RecordingHandle) -> Result<(), AppError> {
    handle.pause()
}

pub fn resume_recording(handle: &RecordingHandle) -> Result<(), AppError> {
    handle.resume()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_lowercase_and_pascalcase_enums() {
        // Request path sends "gif"/"high"; config path uses "Gif"/"High". One
        // schema, both accepted, same variants.
        let lower: RecordingRequest =
            serde_json::from_str(r#"{"output_path":"/tmp/x.mp4","format":"gif","quality":"high"}"#)
                .unwrap();
        let pascal: RecordingRequest =
            serde_json::from_str(r#"{"output_path":"/tmp/x.mp4","format":"Gif","quality":"High"}"#)
                .unwrap();
        assert!(matches!(lower.format, RecordingFormat::Gif));
        assert!(matches!(lower.quality, RecordingQuality::High));
        assert!(matches!(pascal.format, RecordingFormat::Gif));
        assert!(matches!(pascal.quality, RecordingQuality::High));
    }

    #[test]
    fn deserialize_defaults_missing_fields() {
        let req: RecordingRequest =
            serde_json::from_str(r#"{"output_path":"/tmp/x.mp4"}"#).unwrap();
        assert_eq!(req.fps, 30);
        assert!(matches!(req.format, RecordingFormat::Mp4));
        assert!(matches!(req.quality, RecordingQuality::Medium));
        assert!(!req.add_to_history_on_stop);
    }

    #[test]
    fn rejects_empty_output_path() {
        let req = RecordingRequest {
            display: 0,
            region: None,
            output_path: PathBuf::new(),
            format: RecordingFormat::Mp4,
            fps: 30,
            quality: RecordingQuality::Medium,
            ffmpeg_path: None,
            add_to_history_on_stop: false,
            show_clicks: false,
        };
        let res = start_recording(req);
        let Err(err) = res else {
            panic!("expected InvalidRequest");
        };
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }

    #[test]
    fn rejects_zero_size_region() {
        let req = RecordingRequest {
            display: 0,
            region: Some(Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            }),
            output_path: PathBuf::from("/tmp/snapforge-zero.mp4"),
            format: RecordingFormat::Mp4,
            fps: 30,
            quality: RecordingQuality::Medium,
            ffmpeg_path: None,
            add_to_history_on_stop: false,
            show_clicks: false,
        };
        let res = start_recording(req);
        let Err(err) = res else {
            panic!("expected InvalidRequest");
        };
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }
}
