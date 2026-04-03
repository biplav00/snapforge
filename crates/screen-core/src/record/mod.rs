pub mod ffmpeg;

use crate::config::{RecordingFormat, RecordingQuality};
use crate::types::Rect;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("ffmpeg not found — install with: brew install ffmpeg")]
    FfmpegNotFound,
    #[error("ffmpeg failed to start: {0}")]
    FfmpegSpawnFailed(String),
    #[error("recording write failed: {0}")]
    WriteFailed(String),
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("recording not active")]
    NotActive,
}

#[derive(Debug, Clone)]
pub struct RecordConfig {
    pub display: usize,
    pub region: Option<Rect>,
    pub output_path: PathBuf,
    pub format: RecordingFormat,
    pub fps: u32,
    pub quality: RecordingQuality,
}

/// Check if FFmpeg is available on the system.
pub fn check_ffmpeg() -> Result<(), RecordError> {
    match std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => Err(RecordError::FfmpegNotFound),
    }
}
