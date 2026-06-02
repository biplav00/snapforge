//! Recording use case — wraps the ffmpeg pipeline and optionally indexes the
//! finished file in the history store.

use crate::AppError;
use snapforge_domain::Rect;
use snapforge_encode::record::{ffmpeg, RecordConfig};
use snapforge_storage::config::{RecordingFormat, RecordingQuality};
use snapforge_storage::history::{is_incomplete_mp4, ScreenshotHistory};
use std::path::PathBuf;

#[derive(Debug, Clone)]
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
    pub fn is_running(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(snapforge_encode::record::ffmpeg::RecordingHandle::is_running)
    }

    pub fn pause(&self) -> Result<(), AppError> {
        match self.inner.as_ref() {
            Some(h) => {
                h.pause();
                Ok(())
            }
            None => Err(AppError::InvalidRequest("recording already stopped".into())),
        }
    }

    pub fn resume(&self) -> Result<(), AppError> {
        match self.inner.as_ref() {
            Some(h) => {
                h.resume();
                Ok(())
            }
            None => Err(AppError::InvalidRequest("recording already stopped".into())),
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

    let cfg = RecordConfig {
        display: req.display,
        region: req.region,
        output_path: req.output_path.clone(),
        format: req.format,
        fps: req.fps,
        quality: req.quality,
        ffmpeg_path: req.ffmpeg_path,
    };
    let inner = ffmpeg::start_recording(cfg)?;
    Ok(RecordingHandle {
        inner: Some(inner),
        output_path: req.output_path,
        add_to_history_on_stop: req.add_to_history_on_stop,
    })
}

/// Stop a recording and wait for the encoder to flush. Consumes the handle.
/// If `add_to_history_on_stop` was set on the original request, the output
/// file is added to the history index (incomplete mp4s are silently skipped).
pub fn stop_recording(mut handle: RecordingHandle) -> Result<(), AppError> {
    let inner = handle
        .inner
        .take()
        .ok_or_else(|| AppError::InvalidRequest("recording already stopped".into()))?;
    inner.stop()?;

    if handle.add_to_history_on_stop {
        let path_str = handle.output_path.to_string_lossy().into_owned();
        if !is_incomplete_mp4(&path_str) {
            let mut hist = ScreenshotHistory::load()?;
            hist.add_entry(&path_str)?;
        }
    }
    Ok(())
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
        };
        let res = start_recording(req);
        let Err(err) = res else {
            panic!("expected InvalidRequest");
        };
        assert!(matches!(err, AppError::InvalidRequest(_)));
    }
}
