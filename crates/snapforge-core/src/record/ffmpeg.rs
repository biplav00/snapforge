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
        let mut writer =
            std::io::BufWriter::with_capacity((width * height * 4) as usize, &mut stdin);

        writer
            .write_all(test_frame.as_raw())
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        let start = Instant::now();
        let mut next_frame_time = start + frame_interval;

        while !stop_clone.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now < next_frame_time {
                std::thread::sleep(next_frame_time - now);
            }
            next_frame_time += frame_interval;

            let frame = if let Some(r) = &region {
                capture::capture_region(display, *r)
            } else {
                capture::capture_fullscreen(display)
            };

            match frame {
                Ok(img) => {
                    if writer.write_all(img.as_raw()).is_err() {
                        break;
                    }
                }
                Err(_) => continue,
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
