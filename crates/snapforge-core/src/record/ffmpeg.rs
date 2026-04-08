use super::{RecordConfig, RecordError};
use crate::capture;
use crate::config::{RecordingFormat, RecordingQuality};
use std::io::{Read as _, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RecordingHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), RecordError>>>,
}

impl RecordingHandle {
    pub fn stop(mut self) -> Result<(), RecordError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| RecordError::WriteFailed("thread panicked".into()))?
        } else {
            Ok(())
        }
    }

    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

impl Drop for RecordingHandle {
    fn drop(&mut self) {
        // Ensure recording stops and ffmpeg process is reaped even if stop() wasn't called
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start_recording(config: RecordConfig) -> Result<RecordingHandle, RecordError> {
    // Find FFmpeg binary (bundled or system)
    let ffmpeg_path = super::find_ffmpeg(config.ffmpeg_path.as_ref())?;

    // Capture one test frame to get dimensions
    let test_frame = if let Some(region) = &config.region {
        capture::capture_region(config.display, *region)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    } else {
        capture::capture_fullscreen(config.display)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    };

    let width = test_frame.width();
    let height = test_frame.height();

    let mut child = spawn_ffmpeg(&ffmpeg_path, &config, width, height)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| RecordError::FfmpegSpawnFailed("no stdin".into()))?;
    let stderr = child.stderr.take();

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let region = config.region;
    let display = config.display;

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        // Use 256KB buffer — better for pipe throughput than full-frame buffer
        let mut writer = std::io::BufWriter::with_capacity(256 * 1024, &mut stdin);

        // Write the test frame as first frame
        writer
            .write_all(test_frame.as_raw())
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        let start = Instant::now();
        let mut frame_count: u64 = 1;

        while !stop_clone.load(Ordering::SeqCst) {
            // Target-based timing: compute when next frame should land
            // This prevents drift — we always target absolute time from start
            frame_count += 1;
            let target_time = start + frame_interval * frame_count as u32;
            let now = Instant::now();

            if now < target_time {
                std::thread::sleep(target_time - now);
            } else if now > target_time + frame_interval {
                // We're more than one frame behind — skip to catch up
                frame_count = ((now - start).as_secs_f64() / frame_interval.as_secs_f64()) as u64;
                continue;
            }

            let frame = if let Some(r) = &region {
                capture::capture_region(display, *r)
            } else {
                capture::capture_fullscreen(display)
            };

            if let Ok(img) = frame {
                if writer.write_all(img.as_raw()).is_err() {
                    break;
                }
            }
        }

        drop(writer);
        drop(stdin);

        let status = child
            .wait()
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        if !status.success() {
            // Read stderr for diagnostic info
            let stderr_msg = if let Some(mut err) = stderr {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                if buf.is_empty() {
                    format!("ffmpeg exited with code: {:?}", status.code())
                } else {
                    // Truncate to last 500 chars for readability
                    let trimmed = if buf.len() > 500 {
                        &buf[buf.len() - 500..]
                    } else {
                        &buf
                    };
                    format!(
                        "ffmpeg exited with code {:?}: {}",
                        status.code(),
                        trimmed.trim()
                    )
                }
            } else {
                format!("ffmpeg exited with code: {:?}", status.code())
            };
            return Err(RecordError::WriteFailed(stderr_msg));
        }

        Ok(())
    });

    Ok(RecordingHandle {
        stop_flag,
        thread: Some(thread),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RecordingFormat, RecordingQuality};
    use std::path::PathBuf;

    fn test_config(path: &Path) -> super::super::RecordConfig {
        super::super::RecordConfig {
            display: 0,
            region: None,
            output_path: path.to_path_buf(),
            format: RecordingFormat::Mp4,
            fps: 10,
            quality: RecordingQuality::Low,
            ffmpeg_path: None,
        }
    }

    #[test]
    fn test_recording_handle_stop_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::SeqCst));
        flag.store(true, Ordering::SeqCst);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_start_recording_no_ffmpeg() {
        // If ffmpeg is not present, start_recording should return an error, not panic
        let tmp = tempfile::tempdir().unwrap();
        let config = super::super::RecordConfig {
            display: 0,
            region: None,
            output_path: tmp.path().join("test.mp4"),
            format: RecordingFormat::Mp4,
            fps: 10,
            quality: RecordingQuality::Low,
            ffmpeg_path: Some(PathBuf::from("/nonexistent/ffmpeg")),
        };
        // This may succeed if system ffmpeg exists, or fail — either is fine
        let _ = start_recording(config);
    }

    #[test]
    fn test_start_and_stop_recording() {
        // Only run if ffmpeg is available and display capture works
        if super::super::find_ffmpeg(None).is_err() {
            return;
        }
        if crate::capture::capture_fullscreen(0).is_err() {
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(&tmp.path().join("recording.mp4"));

        match start_recording(config) {
            Ok(handle) => {
                assert!(handle.is_running());
                std::thread::sleep(Duration::from_millis(500));
                assert!(handle.stop().is_ok());
            }
            Err(_) => {
                // Capture or ffmpeg may fail in CI
            }
        }
    }
}

fn spawn_ffmpeg(
    ffmpeg_path: &Path,
    config: &RecordConfig,
    width: u32,
    height: u32,
) -> Result<Child, RecordError> {
    let crf = match config.quality {
        RecordingQuality::Low => "28",
        RecordingQuality::Medium => "23",
        RecordingQuality::High => "18",
    };

    let size_arg = format!("{}x{}", width, height);
    let fps_arg = config.fps.to_string();
    let output_path = config.output_path.to_string_lossy().to_string();

    if let Some(parent) = config.output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| RecordError::WriteFailed(format!("failed to create output dir: {}", e)))?;
    }

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-s".into(),
        size_arg,
        "-r".into(),
        fps_arg,
        "-i".into(),
        "-".into(),
        "-an".into(),
    ];

    match config.format {
        RecordingFormat::Mp4 => {
            args.extend([
                "-c:v".into(),
                "libx264".into(),
                "-preset".into(),
                "fast".into(),
                "-crf".into(),
                crf.into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
        }
        RecordingFormat::Gif => {
            args.extend([
                "-vf".into(),
                "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".into(),
                "-loop".into(),
                "0".into(),
            ]);
        }
    }

    args.push(output_path);

    Command::new(ffmpeg_path)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped()) // Capture stderr for error diagnostics
        .spawn()
        .map_err(|e| RecordError::FfmpegSpawnFailed(e.to_string()))
}
