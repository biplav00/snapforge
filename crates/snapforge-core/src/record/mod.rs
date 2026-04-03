pub mod ffmpeg;

use crate::config::{RecordingFormat, RecordingQuality};
use crate::types::Rect;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("ffmpeg not found — run scripts/download-ffmpeg.sh or install ffmpeg")]
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
    /// Optional path to the FFmpeg binary (e.g., bundled resource path).
    /// If None, will search common locations and system PATH.
    pub ffmpeg_path: Option<PathBuf>,
}

/// Find the FFmpeg binary. Checks:
/// 1. Provided path (from Tauri resource resolution)
/// 2. Adjacent to current executable (bundled resource)
/// 3. System PATH
pub fn find_ffmpeg(provided_path: Option<&PathBuf>) -> Result<PathBuf, RecordError> {
    // Check provided path
    if let Some(path) = provided_path {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // Check adjacent to current executable (bundled in app)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Check in binaries/ subfolder (Tauri resources)
            let bundled = exe_dir.join("binaries").join("ffmpeg");
            if bundled.exists() {
                return Ok(bundled);
            }

            // Check with platform suffix
            let target = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
                "ffmpeg-aarch64-apple-darwin"
            } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
                "ffmpeg-x86_64-apple-darwin"
            } else if cfg!(target_os = "windows") {
                "ffmpeg-x86_64-pc-windows-msvc.exe"
            } else {
                "ffmpeg"
            };
            let bundled_platform = exe_dir.join("binaries").join(target);
            if bundled_platform.exists() {
                return Ok(bundled_platform);
            }

            // Check directly next to exe
            let beside_exe = exe_dir.join("ffmpeg");
            if beside_exe.exists() {
                return Ok(beside_exe);
            }
        }
    }

    // Check system PATH (cross-platform)
    let check_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
    if let Ok(output) = std::process::Command::new(check_cmd)
        .arg("ffmpeg")
        .output()
    {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !path_str.is_empty() {
                return Ok(PathBuf::from(path_str));
            }
        }
    }

    Err(RecordError::FfmpegNotFound)
}

/// Check if FFmpeg is available (bundled or system).
pub fn check_ffmpeg() -> Result<(), RecordError> {
    find_ffmpeg(None).map(|_| ())
}
