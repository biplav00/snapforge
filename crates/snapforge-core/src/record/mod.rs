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
    let check_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    if let Ok(output) = std::process::Command::new(check_cmd).arg("ffmpeg").output() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_config_defaults() {
        let config = RecordConfig {
            display: 0,
            region: None,
            output_path: std::path::PathBuf::from("/tmp/test.mp4"),
            format: crate::config::RecordingFormat::Mp4,
            fps: 30,
            quality: crate::config::RecordingQuality::Medium,
            ffmpeg_path: None,
        };
        assert_eq!(config.fps, 30);
        assert!(config.region.is_none());
    }

    #[test]
    fn test_record_config_with_region() {
        let config = RecordConfig {
            display: 0,
            region: Some(Rect {
                x: 100,
                y: 100,
                width: 800,
                height: 600,
            }),
            output_path: std::path::PathBuf::from("/tmp/test.mp4"),
            format: crate::config::RecordingFormat::Mp4,
            fps: 30,
            quality: crate::config::RecordingQuality::High,
            ffmpeg_path: None,
        };
        assert!(config.region.is_some());
        let r = config.region.unwrap();
        assert_eq!(r.width, 800);
        assert_eq!(r.height, 600);
    }

    #[test]
    fn test_find_ffmpeg_system_path() {
        // This may fail in environments without ffmpeg, but should not panic
        let result = find_ffmpeg(None);
        // We just verify it returns a result (Ok or Err), no panic
        let _ = result;
    }

    #[test]
    fn test_find_ffmpeg_nonexistent_path() {
        let bad_path = std::path::PathBuf::from("/nonexistent/ffmpeg");
        let result = find_ffmpeg(Some(&bad_path));
        // Should fall through to system PATH search, not panic
        let _ = result;
    }

    #[test]
    fn test_check_ffmpeg() {
        // Should not panic regardless of whether ffmpeg is installed
        let _ = check_ffmpeg();
    }

    #[test]
    fn test_record_error_display() {
        let err = RecordError::FfmpegNotFound;
        let msg = format!("{}", err);
        assert!(msg.contains("ffmpeg"));

        let err = RecordError::NotActive;
        let msg = format!("{}", err);
        assert!(msg.contains("not active"));
    }
}
