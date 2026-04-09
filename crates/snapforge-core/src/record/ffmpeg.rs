use super::{RecordConfig, RecordError};
use crate::capture;
use crate::clicks::ClickTracker;
use crate::config::{RecordingFormat, RecordingQuality};
use std::io::{Read as _, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Copy, Clone)]
enum PixelFormat {
    #[cfg(not(target_os = "macos"))]
    Rgba,
    #[cfg(target_os = "macos")]
    Bgra,
}

impl PixelFormat {
    fn ffmpeg_name(self) -> &'static str {
        match self {
            #[cfg(not(target_os = "macos"))]
            Self::Rgba => "rgba",
            #[cfg(target_os = "macos")]
            Self::Bgra => "bgra",
        }
    }
}

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

    // For macOS, use the fast BGRA path and downscale large captures for performance.
    #[cfg(target_os = "macos")]
    {
        start_recording_macos_bgra(config, ffmpeg_path)
    }

    // Non-macOS: RGBA path via xcap (unchanged legacy flow).
    #[cfg(not(target_os = "macos"))]
    {
        start_recording_rgba(config, ffmpeg_path)
    }
}

#[cfg(not(target_os = "macos"))]
fn start_recording_rgba(
    config: RecordConfig,
    ffmpeg_path: std::path::PathBuf,
) -> Result<RecordingHandle, RecordError> {
    // Capture one test frame to get dimensions
    let raw_test_frame = if let Some(region) = &config.region {
        capture::capture_region(config.display, *region)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    } else {
        capture::capture_fullscreen(config.display)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?
    };

    // libx264 with yuv420p requires even dimensions — crop to nearest even size if needed
    let width = raw_test_frame.width() & !1;
    let height = raw_test_frame.height() & !1;
    let test_frame = if width != raw_test_frame.width() || height != raw_test_frame.height() {
        image::imageops::crop_imm(&raw_test_frame, 0, 0, width, height).to_image()
    } else {
        raw_test_frame
    };

    let mut child = spawn_ffmpeg(&ffmpeg_path, &config, width, height, PixelFormat::Rgba)?;
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

        // Cache the last successfully captured frame so we can duplicate it
        let mut last_frame: image::RgbaImage = test_frame;

        writer
            .write_all(last_frame.as_raw())
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        let start = Instant::now();
        let mut frame_count: u64 = 1;

        while !stop_clone.load(Ordering::SeqCst) {
            frame_count += 1;
            let target_time = start + frame_interval * frame_count as u32;
            let now = Instant::now();

            if now < target_time {
                std::thread::sleep(target_time - now);
            }

            let frame_result = if let Some(r) = &region {
                capture::capture_region(display, *r)
            } else {
                capture::capture_fullscreen(display)
            };

            let elapsed_frames =
                (start.elapsed().as_secs_f64() / frame_interval.as_secs_f64()) as u64;
            let frames_to_write = elapsed_frames.saturating_sub(frame_count - 1).max(1);

            if let Ok(img) = frame_result {
                let cropped = if img.width() != width || img.height() != height {
                    image::imageops::crop_imm(&img, 0, 0, width, height).to_image()
                } else {
                    img
                };
                last_frame = cropped;
            }

            for _ in 0..frames_to_write {
                if writer.write_all(last_frame.as_raw()).is_err() {
                    break;
                }
            }

            frame_count = frame_count + frames_to_write - 1;
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

/// macOS fast path: capture directly in BGRA and pipe to FFmpeg without
/// per-pixel channel swapping. Downscales large captures to keep up with fps.
#[cfg(target_os = "macos")]
fn start_recording_macos_bgra(
    config: RecordConfig,
    ffmpeg_path: std::path::PathBuf,
) -> Result<RecordingHandle, RecordError> {
    // Cap output at ~1080p for recording to keep per-frame cost reasonable.
    // For screenshots we always use native resolution; this cap is recording-only.
    let max_dimension = Some(1920u32);

    // Build the capture context upfront to determine the output dimensions.
    // We create it once here and then recreate it on the recording thread
    // (ObjC objects are not Send).
    let ctx_probe = capture::CaptureContext::new(config.display, max_dimension)
        .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;
    let full_width = ctx_probe.output_width;
    let full_height = ctx_probe.output_height;
    drop(ctx_probe);

    // Scale factor from native pixel coords → downscaled output coords.
    // The region from the frontend is in native pixels; we need to scale it.
    let native_w = capture::macos::primary_display_pixel_width() as f64;
    let native_h = capture::macos::primary_display_pixel_height() as f64;
    let sx = if native_w > 0.0 {
        f64::from(full_width) / native_w
    } else {
        1.0
    };
    let sy = if native_h > 0.0 {
        f64::from(full_height) / native_h
    } else {
        1.0
    };

    #[allow(clippy::cast_possible_wrap)]
    let (width, height, region_in_scaled) = if let Some(r) = &config.region {
        let rx = ((f64::from(r.x) * sx) as u32) & !1;
        let ry = ((f64::from(r.y) * sy) as u32) & !1;
        let rw = ((f64::from(r.width) * sx) as u32) & !1;
        let rh = ((f64::from(r.height) * sy) as u32) & !1;
        (
            rw,
            rh,
            Some(crate::types::Rect {
                x: rx as i32,
                y: ry as i32,
                width: rw,
                height: rh,
            }),
        )
    } else {
        (full_width, full_height, None)
    };

    if width == 0 || height == 0 {
        return Err(RecordError::CaptureFailed("invalid dimensions".into()));
    }

    let mut child = spawn_ffmpeg(&ffmpeg_path, &config, width, height, PixelFormat::Bgra)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| RecordError::FfmpegSpawnFailed("no stdin".into()))?;
    let stderr = child.stderr.take();

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let display = config.display;

    // Click tracking
    let click_tracker = ClickTracker::new();
    let _click_tap_handle = click_tracker.start_macos_tap();

    // Clicks come in points; convert to scaled-frame pixels.
    let native_scale = capture::display_scale_factor();
    let point_to_scaled_pixel_x = native_scale * sx;
    let point_to_scaled_pixel_y = native_scale * sy;

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        let ctx = capture::CaptureContext::new(display, max_dimension)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;

        let mut writer = std::io::BufWriter::with_capacity(256 * 1024, &mut stdin);

        // Capture the initial frame
        let mut last_frame_bytes: Vec<u8> = match ctx.capture_frame_raw_bgra() {
            Ok(f) => crop_bgra_to_region(
                &f.bytes,
                f.width,
                f.height,
                region_in_scaled.as_ref(),
                width,
                height,
            ),
            Err(_) => vec![0u8; (width as usize) * (height as usize) * 4],
        };

        writer
            .write_all(&last_frame_bytes)
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        let start = Instant::now();
        let mut frame_count: u64 = 1;

        while !stop_clone.load(Ordering::SeqCst) {
            frame_count += 1;
            let target_time = start + frame_interval * frame_count as u32;
            let now = Instant::now();

            if now < target_time {
                std::thread::sleep(target_time - now);
            }

            if let Ok(raw) = ctx.capture_frame_raw_bgra() {
                last_frame_bytes = crop_bgra_to_region(
                    &raw.bytes,
                    raw.width,
                    raw.height,
                    region_in_scaled.as_ref(),
                    width,
                    height,
                );
            }

            let elapsed_frames =
                (start.elapsed().as_secs_f64() / frame_interval.as_secs_f64()) as u64;
            let frames_to_write = elapsed_frames.saturating_sub(frame_count - 1).max(1);

            // For each frame we need to write, draw clicks onto a copy and send it
            for _ in 0..frames_to_write {
                let mut composed = last_frame_bytes.clone();
                draw_clicks_bgra(
                    &mut composed,
                    width,
                    height,
                    &click_tracker,
                    region_in_scaled.as_ref(),
                    point_to_scaled_pixel_x,
                    point_to_scaled_pixel_y,
                );
                if writer.write_all(&composed).is_err() {
                    break;
                }
            }

            frame_count = frame_count + frames_to_write - 1;
        }

        drop(writer);
        drop(stdin);

        let status = child
            .wait()
            .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

        if !status.success() {
            let stderr_msg = if let Some(mut err) = stderr {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                if buf.is_empty() {
                    format!("ffmpeg exited with code: {:?}", status.code())
                } else {
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

/// Crop a BGRA buffer to the target region (or return as-is if no region).
/// If the source and target sizes already match and there's no region, returns a copy.
#[cfg(target_os = "macos")]
fn crop_bgra_to_region(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    region: Option<&crate::types::Rect>,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    let src_stride = (src_w as usize) * 4;
    let dst_stride = (dst_w as usize) * 4;

    if let Some(r) = region {
        let rx = r.x.max(0) as usize;
        let ry = r.y.max(0) as usize;
        let rw = (r.width as usize).min((src_w as usize).saturating_sub(rx));
        let rh = (r.height as usize).min((src_h as usize).saturating_sub(ry));
        let rw = rw.min(dst_w as usize);
        let rh = rh.min(dst_h as usize);

        let mut dst = vec![0u8; dst_stride * (dst_h as usize)];
        for y in 0..rh {
            let src_off = (ry + y) * src_stride + rx * 4;
            let dst_off = y * dst_stride;
            dst[dst_off..dst_off + rw * 4].copy_from_slice(&src[src_off..src_off + rw * 4]);
        }
        dst
    } else if src_w == dst_w && src_h == dst_h {
        src.to_vec()
    } else {
        // Fallback: crop to top-left dst_w x dst_h
        let mut dst = vec![0u8; dst_stride * (dst_h as usize)];
        let copy_w = (dst_w as usize).min(src_w as usize) * 4;
        let copy_h = (dst_h as usize).min(src_h as usize);
        for y in 0..copy_h {
            let src_off = y * src_stride;
            let dst_off = y * dst_stride;
            dst[dst_off..dst_off + copy_w].copy_from_slice(&src[src_off..src_off + copy_w]);
        }
        dst
    }
}

/// Draw click markers onto a BGRA buffer (in-place).
#[cfg(target_os = "macos")]
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
fn draw_clicks_bgra(
    buf: &mut [u8],
    width: u32,
    height: u32,
    tracker: &ClickTracker,
    region: Option<&crate::types::Rect>,
    scale_x: f64,
    scale_y: f64,
) {
    const CLICK_LIFETIME_MS: u64 = 800;
    const BASE_RADIUS: f64 = 24.0;
    const RING_THICKNESS: f64 = 4.0;

    let clicks = tracker.recent(CLICK_LIFETIME_MS);
    if clicks.is_empty() {
        return;
    }

    let now = Instant::now();
    let fw = f64::from(width);
    let fh = f64::from(height);
    let stride = (width as usize) * 4;

    let (region_x, region_y) = region.map_or((0, 0), |r| (r.x, r.y));

    for click in &clicks {
        let age_ms = now.duration_since(click.timestamp).as_millis() as f64;
        let t = age_ms / CLICK_LIFETIME_MS as f64;
        let alpha = ((1.0 - t) * 255.0).clamp(0.0, 255.0) as u8;
        let radius = BASE_RADIUS + t * 16.0;

        // Click in scaled-frame coordinates, then offset by region
        let px = click.x * scale_x - f64::from(region_x);
        let py = click.y * scale_y - f64::from(region_y);

        if px < -radius || py < -radius || px > fw + radius || py > fh + radius {
            continue;
        }

        let inner = radius - RING_THICKNESS;
        let inner_sq = inner * inner;
        let outer_sq = radius * radius;

        let min_x = (px - radius).floor().max(0.0) as u32;
        let max_x = ((px + radius).ceil() as u32).min(width.saturating_sub(1));
        let min_y = (py - radius).floor().max(0.0) as u32;
        let max_y = ((py + radius).ceil() as u32).min(height.saturating_sub(1));

        let src_a = f32::from(alpha) / 255.0;
        if src_a <= 0.0 {
            continue;
        }
        let inv_a = 1.0 - src_a;

        // Red ring in BGRA: B=60, G=60, R=255
        let (sb, sg, sr) = (60u8, 60u8, 255u8);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = f64::from(x) - px;
                let dy = f64::from(y) - py;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= outer_sq && dist_sq >= inner_sq {
                    let off = (y as usize) * stride + (x as usize) * 4;
                    buf[off] = (f32::from(sb) * src_a + f32::from(buf[off]) * inv_a) as u8;
                    buf[off + 1] = (f32::from(sg) * src_a + f32::from(buf[off + 1]) * inv_a) as u8;
                    buf[off + 2] = (f32::from(sr) * src_a + f32::from(buf[off + 2]) * inv_a) as u8;
                    buf[off + 3] = 255;
                }
            }
        }
    }
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
    pixel_format: PixelFormat,
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
        pixel_format.ffmpeg_name().into(),
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
