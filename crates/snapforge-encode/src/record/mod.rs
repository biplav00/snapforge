pub mod ffmpeg;

use snapforge_domain::Rect;
use snapforge_storage::config::{RecordingFormat, RecordingQuality};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecordError {
    #[error("ffmpeg not found — rebuild the app (qt/scripts/bundle-ffmpeg.sh runs automatically) or install ffmpeg on PATH")]
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
    /// When true, mouse-click ripples are composited into the recorded frames.
    /// Requires Accessibility permission for the CGEventTap; if that's denied
    /// the tap silently no-ops and no ripples are drawn.
    pub show_clicks: bool,
}

/// Find the FFmpeg binary. Checks:
/// 1. Provided path (from Tauri resource resolution)
/// 2. Adjacent to current executable (bundled resource)
/// 3. System PATH
pub fn find_ffmpeg(provided_path: Option<&PathBuf>) -> Result<PathBuf, RecordError> {
    // Check provided path. Surfaced separately in the log so a non-system
    // ffmpeg binary path (which can be set via config and would let an
    // attacker with config-write access run arbitrary code) is at least
    // visible during incident response.
    if let Some(path) = provided_path {
        if path.exists() {
            if !is_trusted_ffmpeg_location(path) {
                tracing::warn!(
                    "[snapforge] WARNING: using non-system ffmpeg path: {} \
                     (set via config; verify intent)",
                    path.display()
                );
            }
            return Ok(path.clone());
        }
    }

    // Platform-suffixed name used by Tauri's externalBin sidecars
    let platform_name = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "ffmpeg-aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "ffmpeg-x86_64-apple-darwin"
    } else if cfg!(target_os = "windows") {
        "ffmpeg-x86_64-pc-windows-msvc.exe"
    } else {
        "ffmpeg"
    };

    // Check adjacent to current executable (bundled in app)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // Tauri places externalBin sidecars directly next to the main binary
            // in Contents/MacOS/ with the platform suffix
            let sidecar = exe_dir.join(platform_name);
            if sidecar.exists() {
                return Ok(sidecar);
            }

            // Also check in a binaries/ subfolder (dev build copies)
            let bundled_in_subdir = exe_dir.join("binaries").join(platform_name);
            if bundled_in_subdir.exists() {
                return Ok(bundled_in_subdir);
            }

            // Plain "ffmpeg" next to exe (fallback)
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

/// Heuristic for "is this an ffmpeg binary in a location we'd expect?". Used
/// only to gate a warning log — not as an authorization boundary.
fn is_trusted_ffmpeg_location(path: &Path) -> bool {
    use std::path::Component;
    let Ok(abs) = path.canonicalize() else {
        return false;
    };
    // Adjacent to current executable (bundled in app)
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(exe_abs) = exe.canonicalize() {
            if let Some(exe_dir) = exe_abs.parent() {
                if abs.starts_with(exe_dir) {
                    return true;
                }
            }
        }
    }
    // Conservative allowlist of canonical system locations.
    const TRUSTED_PREFIXES: &[&str] = &[
        "/usr/bin",
        "/usr/local/bin",
        "/usr/local/Cellar",
        "/opt/homebrew",
        "/opt/local/bin",
        "/snap/bin",
    ];
    let abs_str = abs.to_string_lossy();
    if TRUSTED_PREFIXES.iter().any(|p| abs_str.starts_with(p)) {
        return true;
    }
    // Treat anything under the user's home that isn't in a hidden tmp-ish
    // directory as suspicious — a tighter heuristic is brittle across distros.
    let _ = Component::Normal; // silence unused import on some platforms
    false
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
            format: snapforge_storage::config::RecordingFormat::Mp4,
            fps: 30,
            quality: snapforge_storage::config::RecordingQuality::Medium,
            ffmpeg_path: None,
            show_clicks: false,
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
            format: snapforge_storage::config::RecordingFormat::Mp4,
            fps: 30,
            quality: snapforge_storage::config::RecordingQuality::High,
            ffmpeg_path: None,
            show_clicks: false,
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
    fn test_find_ffmpeg_returns_result() {
        // Just verify find_ffmpeg doesn't panic — returns Some or None.
        let _ = find_ffmpeg(None);
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
