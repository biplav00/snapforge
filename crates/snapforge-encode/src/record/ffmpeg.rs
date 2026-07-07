use super::{RecordConfig, RecordError};
use snapforge_capture::capture;
use snapforge_capture::clicks::ClickTracker;
use snapforge_storage::config::{RecordingFormat, RecordingQuality};
use std::io::{Read as _, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const STATE_RUNNING: u8 = 0;
const STATE_PAUSED: u8 = 1;
const STATE_STOPPED: u8 = 2;

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
    state: Arc<AtomicU8>,
    thread: Option<std::thread::JoinHandle<Result<(), RecordError>>>,
}

impl RecordingHandle {
    pub fn stop(mut self) -> Result<(), RecordError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.state.store(STATE_STOPPED, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| RecordError::WriteFailed("thread panicked".into()))?
        } else {
            Ok(())
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_RUNNING
    }

    pub fn pause(&self) {
        self.state.store(STATE_PAUSED, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.state.store(STATE_RUNNING, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub fn is_paused(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_PAUSED
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

pub fn start_recording(mut config: RecordConfig) -> Result<RecordingHandle, RecordError> {
    // Guard fps before it reaches `Duration::from_secs_f64(1.0 / fps)`: fps == 0
    // yields 1.0/0.0 = inf, which panics that constructor (and would cross the
    // panic back over the FFI boundary). Clamp to a sane range for corrupted
    // configs or bad FFI callers.
    config.fps = config.fps.clamp(1, 240);

    // Find FFmpeg binary (bundled or system)
    let ffmpeg_path = super::find_ffmpeg(config.ffmpeg_path.as_ref())?;

    // Validate it actually runs and reports itself as ffmpeg.
    validate_ffmpeg(&ffmpeg_path)?;

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

/// Run `<ffmpeg_path> -version` and confirm the output identifies as ffmpeg.
fn validate_ffmpeg(ffmpeg_path: &Path) -> Result<(), RecordError> {
    let output = Command::new(ffmpeg_path)
        .arg("-version")
        .output()
        .map_err(|e| {
            RecordError::FfmpegSpawnFailed(format!(
                "ffmpeg at {} could not be executed: {}",
                ffmpeg_path.display(),
                e
            ))
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stdout.contains("ffmpeg version") || stderr.contains("ffmpeg version") {
        Ok(())
    } else {
        Err(RecordError::FfmpegSpawnFailed(format!(
            "binary at {} does not look like ffmpeg (no 'ffmpeg version' in output)",
            ffmpeg_path.display()
        )))
    }
}

/// Ensure a spawned ffmpeg child is terminated and reaped before we return
/// an error from the caller — otherwise we leave a zombie + open pipes.
fn kill_and_reap(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Maximum bytes of ffmpeg stderr retained for error diagnostics.
const STDERR_RETAIN_BYTES: usize = 4096;

/// Continuously drain ffmpeg's stderr on a background thread.
///
/// stderr must be drained *while ffmpeg runs*: if the 64KB pipe buffer fills,
/// ffmpeg blocks writing to it, stops reading stdin, and the capture thread
/// deadlocks in write_all — stop()/Drop then hang forever in join(). Only the
/// tail of the output is retained (capped) so a chatty ffmpeg can't grow
/// memory unbounded; the buffer is used for the error message on failure.
fn spawn_stderr_drain(stderr: Option<std::process::ChildStderr>) -> Arc<Mutex<String>> {
    let buf = Arc::new(Mutex::new(String::new()));
    if let Some(mut err) = stderr {
        let buf_clone = Arc::clone(&buf);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match err.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut guard) = buf_clone.lock() {
                            guard.push_str(&String::from_utf8_lossy(&chunk[..n]));
                            if guard.len() > STDERR_RETAIN_BYTES * 2 {
                                // Keep only the tail, respecting char boundaries.
                                let mut cut = guard.len() - STDERR_RETAIN_BYTES;
                                while !guard.is_char_boundary(cut) {
                                    cut -= 1;
                                }
                                guard.drain(..cut);
                            }
                        }
                    }
                }
            }
        });
    }
    buf
}

/// Build the error for a non-success ffmpeg exit using the drained stderr tail.
fn ffmpeg_exit_error(status: std::process::ExitStatus, stderr_buf: &Mutex<String>) -> RecordError {
    let buf = stderr_buf.lock().map(|g| g.clone()).unwrap_or_default();
    if buf.is_empty() {
        RecordError::WriteFailed(format!("ffmpeg exited with code: {:?}", status.code()))
    } else {
        // Truncate to last 500 chars for readability (char-boundary safe).
        let mut cut = buf.len().saturating_sub(500);
        while !buf.is_char_boundary(cut) {
            cut -= 1;
        }
        RecordError::WriteFailed(format!(
            "ffmpeg exited with code {:?}: {}",
            status.code(),
            buf[cut..].trim()
        ))
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
    let Some(mut stdin) = child.stdin.take() else {
        kill_and_reap(child);
        return Err(RecordError::FfmpegSpawnFailed("no stdin".into()));
    };
    // Drain stderr from the start so ffmpeg can never block on a full pipe.
    let stderr_buf = spawn_stderr_drain(child.stderr.take());

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let state = Arc::new(AtomicU8::new(STATE_RUNNING));
    let state_clone = state.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let region = config.region;
    let display = config.display;

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        // Run the capture loop in an inner closure so EVERY exit path —
        // success or error — falls through to the cleanup below, which marks
        // the handle state terminal and reaps ffmpeg on failure. Without
        // this, an error exit left is_running() true forever and ffmpeg as a
        // zombie.
        let result = (|| -> Result<(), RecordError> {
            // Use 256KB buffer — better for pipe throughput than full-frame buffer
            let mut writer = std::io::BufWriter::with_capacity(256 * 1024, &mut stdin);

            // Cache the last successfully captured frame so we can duplicate it
            let mut last_frame: image::RgbaImage = test_frame;

            if let Err(e) = writer.write_all(last_frame.as_raw()) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Err(RecordError::WriteFailed(
                        "ffmpeg stdin closed (broken pipe)".into(),
                    ));
                }
                return Err(RecordError::WriteFailed(e.to_string()));
            }

            let start = Instant::now();
            let mut frame_count: u64 = 1;
            // Accumulated time spent in the paused state — subtracted from real elapsed
            // so the output stream stays constant-frame-rate across pause/resume.
            let mut paused_accum = Duration::ZERO;
            let mut paused_since: Option<Instant> = None;

            loop {
                if stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Pause: freeze the frame clock, skip capture & write entirely.
                if state_clone.load(Ordering::SeqCst) == STATE_PAUSED {
                    if paused_since.is_none() {
                        paused_since = Some(Instant::now());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                } else if let Some(p0) = paused_since.take() {
                    paused_accum += p0.elapsed();
                }

                frame_count += 1;
                let target_time = start + paused_accum + frame_interval * frame_count as u32;
                let now = Instant::now();

                if now < target_time {
                    std::thread::sleep(target_time - now);
                }

                let frame_result = if let Some(r) = &region {
                    capture::capture_region(display, *r)
                } else {
                    capture::capture_fullscreen(display)
                };

                let effective_elapsed = start
                    .elapsed()
                    .checked_sub(paused_accum)
                    .unwrap_or_default();
                let elapsed_frames =
                    (effective_elapsed.as_secs_f64() / frame_interval.as_secs_f64()) as u64;
                let frames_to_write = elapsed_frames.saturating_sub(frame_count - 1).max(1);

                if let Ok(img) = frame_result {
                    if img.width() == width && img.height() == height {
                        last_frame = img;
                    } else {
                        // Display changed size mid-recording: ffmpeg reads
                        // fixed width*height*4 rawvideo frames, so copy the
                        // incoming frame into the fixed-size buffer (crop if
                        // larger, zero-pad if smaller). A short frame would
                        // permanently desync the raw stream.
                        copy_rgba_padded(&img, &mut last_frame);
                    }
                }

                for _ in 0..frames_to_write {
                    if let Err(e) = writer.write_all(last_frame.as_raw()) {
                        // Any write error is fatal, not just BrokenPipe: a
                        // partial frame permanently desyncs the raw stream, so
                        // silently continuing would truncate the video while
                        // stop() still reported Ok.
                        return Err(RecordError::WriteFailed(
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                "ffmpeg stdin closed mid-recording (broken pipe)".into()
                            } else {
                                format!("writing frames to ffmpeg failed: {}", e)
                            },
                        ));
                    }
                }

                frame_count = frame_count + frames_to_write - 1;
            }

            // Flush explicitly: BufWriter::drop swallows flush errors, which
            // would silently drop the buffered tail of the recording.
            if let Err(e) = writer.flush() {
                return Err(RecordError::WriteFailed(format!(
                    "final flush to ffmpeg failed: {}",
                    e
                )));
            }
            drop(writer);
            drop(stdin);

            let status = child
                .wait()
                .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

            if !status.success() {
                return Err(ffmpeg_exit_error(status, &stderr_buf));
            }

            Ok(())
        })();

        state_clone.store(STATE_STOPPED, Ordering::SeqCst);
        if result.is_err() {
            // No zombie ffmpeg on error exits. Harmless when the child has
            // already been waited on: kill() fails, wait() returns the
            // cached status.
            kill_and_reap(child);
        }
        result
    });

    Ok(RecordingHandle {
        stop_flag,
        state,
        thread: Some(thread),
    })
}

/// Copy `src` into the fixed-size `dst` frame: the overlapping window is
/// copied, any uncovered area is zeroed. Keeps every frame piped to ffmpeg at
/// exactly `dst_w * dst_h * 4` bytes even if the display changes size
/// mid-recording (mirrors the macOS `crop_bgra_to_region_into` behavior).
#[cfg(not(target_os = "macos"))]
fn copy_rgba_padded(src: &image::RgbaImage, dst: &mut image::RgbaImage) {
    let (dst_w, dst_h) = (dst.width() as usize, dst.height() as usize);
    let (src_w, src_h) = (src.width() as usize, src.height() as usize);
    let dst_stride = dst_w * 4;
    let src_stride = src_w * 4;
    let copy_w = dst_w.min(src_w) * 4;
    let copy_h = dst_h.min(src_h);
    let src_buf = src.as_raw();
    let dst_buf: &mut [u8] = dst;
    dst_buf.fill(0);
    for y in 0..copy_h {
        let src_off = y * src_stride;
        let dst_off = y * dst_stride;
        dst_buf[dst_off..dst_off + copy_w].copy_from_slice(&src_buf[src_off..src_off + copy_w]);
    }
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

    // Compute output dimensions up front WITHOUT touching SCK on the main
    // thread — CaptureContext::new would create SCContentFilter /
    // SCStreamConfiguration, which is exactly what we want to keep off main.
    // Reading the display's pixel size via CGDisplayCopyDisplayMode is safe.
    let (full_width, full_height) =
        capture::macos::compute_recording_output_size(config.display, max_dimension)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;

    // Scale factor from native pixel coords → downscaled output coords.
    // The region from the frontend is in native pixels; we need to scale it.
    // Use the TARGET display's pixel size — the primary display's dimensions
    // would produce wrong scale factors (and wrong region crops) whenever
    // config.display is a secondary display.
    let (native_w_px, native_h_px) = capture::macos::display_pixel_size_for_index(config.display)
        .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;
    let native_w = native_w_px as f64;
    let native_h = native_h_px as f64;
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
            Some(snapforge_domain::Rect {
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
    let Some(mut stdin) = child.stdin.take() else {
        kill_and_reap(child);
        return Err(RecordError::FfmpegSpawnFailed("no stdin".into()));
    };
    // Drain stderr from the start so ffmpeg can never block on a full pipe.
    let stderr_buf = spawn_stderr_drain(child.stderr.take());

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let state = Arc::new(AtomicU8::new(STATE_RUNNING));
    let state_clone = state.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let display = config.display;

    // Click tracking — only when the "show clicks" pref is on. When off, the
    // tap is never started and `click_snapshot` stays empty, so the hot-path
    // below writes captured frames straight through with no overlay.
    let click_tracker = config.show_clicks.then(ClickTracker::new);
    let click_tap_handle = click_tracker
        .as_ref()
        .and_then(ClickTracker::start_macos_tap);

    // Clicks come in GLOBAL desktop points (CGEventGetLocation); convert to
    // scaled-frame pixels of the TARGET display: translate by the display's
    // origin, then scale by its point→pixel factor and the downscale ratio.
    let native_scale = capture::macos::display_scale_factor_for(config.display)
        .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;
    let (display_origin_x, display_origin_y) =
        capture::macos::display_origin_points(config.display)
            .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;
    let point_to_scaled_pixel_x = native_scale * sx;
    let point_to_scaled_pixel_y = native_scale * sy;

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        // Keep the click event tap alive for the whole recording. Bound to the
        // thread's stack (not left at function scope, where it dropped — and so
        // disabled the CGEventTap — microseconds after start), so it keeps
        // feeding click_tracker until the capture loop exits.
        let _click_tap_handle = click_tap_handle;
        // Run the capture loop in an inner closure so EVERY exit path —
        // success or error — falls through to the cleanup below, which marks
        // the handle state terminal and reaps ffmpeg on failure. Without
        // this, an error exit left is_running() true forever and ffmpeg as a
        // zombie.
        let result = (|| -> Result<(), RecordError> {
            let ctx = capture::CaptureContext::new(display, max_dimension)
                .map_err(|e| RecordError::CaptureFailed(e.to_string()))?;

            let frame_bytes = (width as usize) * (height as usize) * 4;
            // Use a buffer roughly the size of one frame so each frame typically
            // flushes once (the previous 256 KiB caused 30+ flushes per 1080p
            // frame, which was strictly slower than no buffering).
            let buf_capacity = frame_bytes.clamp(1 << 20, 8 << 20); // 1–8 MiB
            let mut writer = std::io::BufWriter::with_capacity(buf_capacity, &mut stdin);

            // Two scratch buffers reused across the entire recording: one holds the
            // most recent captured (and cropped) frame, the other is the per-output
            // "composed" frame with clicks drawn over it. Pre-allocating eliminates
            // ~240 MB/s of allocator traffic at 1080p30.
            let mut last_frame_bytes: Vec<u8> = vec![0u8; frame_bytes];
            let mut composed: Vec<u8> = vec![0u8; frame_bytes];
            // Reused click snapshot — refreshed once per captured frame.
            let mut click_snapshot: Vec<snapforge_capture::clicks::ClickEvent> =
                Vec::with_capacity(32);

            // Capture the initial frame into last_frame_bytes.
            match ctx.capture_frame_raw_bgra() {
                Ok(f) => {
                    crop_bgra_to_region_into(
                        &f.bytes,
                        f.width,
                        f.height,
                        region_in_scaled.as_ref(),
                        width,
                        height,
                        &mut last_frame_bytes,
                    );
                }
                Err(e) => {
                    // The zeroed scratch buffer is written instead — the
                    // recording starts with a black frame, so leave a trace.
                    tracing::warn!(
                    "[record] initial frame capture failed ({e}); recording starts with a black frame"
                );
                }
            }

            if let Err(e) = writer.write_all(&last_frame_bytes) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    return Err(RecordError::WriteFailed(
                        "ffmpeg stdin closed (broken pipe)".into(),
                    ));
                }
                return Err(RecordError::WriteFailed(e.to_string()));
            }

            let start = Instant::now();
            let mut frame_count: u64 = 1;
            let mut paused_accum = Duration::ZERO;
            let mut paused_since: Option<Instant> = None;

            loop {
                if stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Pause: freeze the frame clock, skip capture & write entirely.
                if state_clone.load(Ordering::SeqCst) == STATE_PAUSED {
                    if paused_since.is_none() {
                        paused_since = Some(Instant::now());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                } else if let Some(p0) = paused_since.take() {
                    paused_accum += p0.elapsed();
                }

                frame_count += 1;
                let target_time = start + paused_accum + frame_interval * frame_count as u32;
                let now = Instant::now();

                if now < target_time {
                    std::thread::sleep(target_time - now);
                }

                if let Ok(raw) = ctx.capture_frame_raw_bgra() {
                    crop_bgra_to_region_into(
                        &raw.bytes,
                        raw.width,
                        raw.height,
                        region_in_scaled.as_ref(),
                        width,
                        height,
                        &mut last_frame_bytes,
                    );
                }

                // Refresh the click snapshot once per captured frame, not per
                // output frame. The lock + filter cost is amortized across all
                // duplicated frames written below. With no tracker (clicks off)
                // the snapshot stays empty and the overlay is skipped entirely.
                if let Some(tracker) = click_tracker.as_ref() {
                    tracker.recent_into(CLICK_LIFETIME_MS, &mut click_snapshot);
                }

                let effective_elapsed = start
                    .elapsed()
                    .checked_sub(paused_accum)
                    .unwrap_or_default();
                let elapsed_frames =
                    (effective_elapsed.as_secs_f64() / frame_interval.as_secs_f64()) as u64;
                let frames_to_write = elapsed_frames.saturating_sub(frame_count - 1).max(1);

                for _ in 0..frames_to_write {
                    // Hot-path optimization: when no clicks are active, skip the
                    // copy and write the captured frame directly. With clicks,
                    // copy into the scratch buffer and overlay them in place.
                    let to_write: &[u8] = if click_snapshot.is_empty() {
                        &last_frame_bytes
                    } else {
                        composed.copy_from_slice(&last_frame_bytes);
                        draw_clicks_bgra(
                            &mut composed,
                            width,
                            height,
                            &click_snapshot,
                            region_in_scaled.as_ref(),
                            display_origin_x,
                            display_origin_y,
                            point_to_scaled_pixel_x,
                            point_to_scaled_pixel_y,
                        );
                        &composed
                    };
                    if let Err(e) = writer.write_all(to_write) {
                        // Any write error is fatal, not just BrokenPipe: a
                        // partial frame permanently desyncs the raw stream, so
                        // silently continuing would truncate the video while
                        // stop() still reported Ok.
                        return Err(RecordError::WriteFailed(
                            if e.kind() == std::io::ErrorKind::BrokenPipe {
                                "ffmpeg stdin closed mid-recording (broken pipe)".into()
                            } else {
                                format!("writing frames to ffmpeg failed: {}", e)
                            },
                        ));
                    }
                }

                frame_count = frame_count + frames_to_write - 1;
            }

            // Flush explicitly: BufWriter::drop swallows flush errors, which
            // would silently drop the buffered tail of the recording.
            if let Err(e) = writer.flush() {
                return Err(RecordError::WriteFailed(format!(
                    "final flush to ffmpeg failed: {}",
                    e
                )));
            }
            drop(writer);
            drop(stdin);

            let status = child
                .wait()
                .map_err(|e| RecordError::WriteFailed(e.to_string()))?;

            if !status.success() {
                return Err(ffmpeg_exit_error(status, &stderr_buf));
            }

            Ok(())
        })();

        state_clone.store(STATE_STOPPED, Ordering::SeqCst);
        if result.is_err() {
            // No zombie ffmpeg on error exits. Harmless when the child has
            // already been waited on: kill() fails, wait() returns the
            // cached status.
            kill_and_reap(child);
        }
        result
    });

    Ok(RecordingHandle {
        stop_flag,
        state,
        thread: Some(thread),
    })
}

/// Crop a BGRA buffer into the caller's destination buffer (no allocation).
/// `dst` must already be sized to `dst_w * dst_h * 4`. The destination is
/// fully overwritten — any pixels not covered by the crop are zeroed.
#[cfg(target_os = "macos")]
fn crop_bgra_to_region_into(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    region: Option<&snapforge_domain::Rect>,
    dst_w: u32,
    dst_h: u32,
    dst: &mut [u8],
) {
    let src_stride = (src_w as usize) * 4;
    let dst_stride = (dst_w as usize) * 4;

    if let Some(r) = region {
        // Zero on entry so any partial-coverage region doesn't leak the
        // previous frame's contents into the uncovered margin.
        dst.fill(0);
        let rx = r.x.max(0) as usize;
        let ry = r.y.max(0) as usize;
        let rw = (r.width as usize).min((src_w as usize).saturating_sub(rx));
        let rh = (r.height as usize).min((src_h as usize).saturating_sub(ry));
        let rw = rw.min(dst_w as usize);
        let rh = rh.min(dst_h as usize);

        for y in 0..rh {
            let src_off = (ry + y) * src_stride + rx * 4;
            let dst_off = y * dst_stride;
            dst[dst_off..dst_off + rw * 4].copy_from_slice(&src[src_off..src_off + rw * 4]);
        }
    } else if src_w == dst_w && src_h == dst_h {
        // Common no-region case: a single contiguous copy.
        dst.copy_from_slice(&src[..dst_stride * (dst_h as usize)]);
    } else {
        dst.fill(0);
        let copy_w = (dst_w as usize).min(src_w as usize) * 4;
        let copy_h = (dst_h as usize).min(src_h as usize);
        for y in 0..copy_h {
            let src_off = y * src_stride;
            let dst_off = y * dst_stride;
            dst[dst_off..dst_off + copy_w].copy_from_slice(&src[src_off..src_off + copy_w]);
        }
    }
}

/// Lifetime in ms of a click marker on the recorded video. Promoted to a
/// crate-level constant so the recording loop and `draw_clicks_bgra` agree.
#[cfg(target_os = "macos")]
const CLICK_LIFETIME_MS: u64 = 1000;

/// Draw click markers onto a BGRA buffer (in-place).
/// Each click produces a ripple animation: a filled dot at the click point
/// plus two expanding rings that fade out over the lifetime.
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
    clicks: &[snapforge_capture::clicks::ClickEvent],
    region: Option<&snapforge_domain::Rect>,
    origin_x: f64,
    origin_y: f64,
    scale_x: f64,
    scale_y: f64,
) {
    const DOT_RADIUS: f64 = 8.0;
    const RING_START_RADIUS: f64 = 12.0;
    const RING_END_RADIUS: f64 = 52.0;
    const RING_THICKNESS: f64 = 3.5;

    if clicks.is_empty() {
        return;
    }

    let now = Instant::now();
    let (region_x, region_y) = region.map_or((0, 0), |r| (r.x, r.y));

    // BGRA: red = (60, 60, 255), white = (255, 255, 255)
    let red = (60u8, 60u8, 255u8);
    let white = (255u8, 255u8, 255u8);

    for click in clicks {
        let age_ms = now.duration_since(click.timestamp).as_millis() as f64;
        let t = (age_ms / CLICK_LIFETIME_MS as f64).clamp(0.0, 1.0);

        // Convert click GLOBAL screen-points to scaled-frame pixels: translate
        // into the target display's local point space first, then scale.
        let px = (click.x - origin_x) * scale_x - f64::from(region_x);
        let py = (click.y - origin_y) * scale_y - f64::from(region_y);

        // --- Filled center dot: visible for the first half, fades out ---
        let dot_alpha = ((1.0 - t).powf(0.6) * 230.0) as u8;
        if dot_alpha > 0 {
            fill_circle_bgra(buf, width, height, px, py, DOT_RADIUS, red, dot_alpha);
            // White inner highlight
            fill_circle_bgra(
                buf,
                width,
                height,
                px,
                py,
                DOT_RADIUS * 0.45,
                white,
                dot_alpha,
            );
        }

        // --- Primary ring: expands and fades ---
        let r1 = RING_START_RADIUS + t * (RING_END_RADIUS - RING_START_RADIUS);
        let a1 = ((1.0 - t) * 220.0) as u8;
        if a1 > 0 {
            draw_ring_bgra(buf, width, height, px, py, r1, RING_THICKNESS, red, a1);
        }

        // --- Secondary ring: delayed expand for a ripple effect ---
        let t2 = ((t - 0.2) / 0.8).clamp(0.0, 1.0);
        if t2 > 0.0 {
            let r2 = RING_START_RADIUS + t2 * (RING_END_RADIUS - RING_START_RADIUS);
            let a2 = ((1.0 - t2) * 140.0) as u8;
            if a2 > 0 {
                draw_ring_bgra(
                    buf,
                    width,
                    height,
                    px,
                    py,
                    r2,
                    RING_THICKNESS - 1.0,
                    red,
                    a2,
                );
            }
        }
    }
}

/// Integer u8-domain alpha blend: `(s*a + d*(255-a) + 127) / 255`. Approximated
/// with `(x + 128 + (x >> 8)) >> 8` so the divide is two shifts and an add,
/// staying within 1 ULP of the exact result for u16 inputs. Massively faster
/// than the previous f32 multiply path on the recording hot loop.
// Forced inline: this is the per-pixel blend on the recording hot loop; the
// call overhead is measurable, so we deliberately request always-inline.
#[allow(clippy::inline_always)]
#[inline(always)]
fn blend_u8(src: u8, dst: u8, alpha: u8) -> u8 {
    let s = u16::from(src);
    let d = u16::from(dst);
    let a = u16::from(alpha);
    let v = s * a + d * (255 - a);
    // Round-to-nearest divide by 255.
    (((v + 128) + ((v + 128) >> 8)) >> 8) as u8
}

/// Fill a solid circle in BGRA with alpha blending. Inner loop uses integer
/// squared-distance against `r_sq_q8` (radius squared in 8.8 fixed-point) so
/// the per-pixel cost is two integer mults + a compare + (when inside) a
/// short blend — no f64 in the hot loop.
#[cfg(target_os = "macos")]
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
fn fill_circle_bgra(
    buf: &mut [u8],
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    radius: f64,
    color: (u8, u8, u8),
    alpha: u8,
) {
    if alpha == 0 || radius <= 0.0 {
        return;
    }
    let stride = (width as usize) * 4;

    let min_x = (cx - radius).floor().max(0.0) as u32;
    let max_x = ((cx + radius).ceil() as u32).min(width.saturating_sub(1));
    let min_y = (cy - radius).floor().max(0.0) as u32;
    let max_y = ((cy + radius).ceil() as u32).min(height.saturating_sub(1));

    if min_x > max_x || min_y > max_y {
        return;
    }

    // Center in 8.8 fixed point so subpixel positioning is preserved without
    // floating-point math in the inner loop.
    let cx_q8 = (cx * 256.0) as i32;
    let cy_q8 = (cy * 256.0) as i32;
    let r_q8 = (radius * 256.0) as i32;
    let r_sq_q8 = (r_q8 as i64) * (r_q8 as i64);

    let (sb, sg, sr) = color;

    for y in min_y..=max_y {
        let dy_q8 = (y as i32 * 256) - cy_q8;
        let dy_sq = (dy_q8 as i64) * (dy_q8 as i64);
        if dy_sq > r_sq_q8 {
            continue;
        }
        let row_off = (y as usize) * stride;
        for x in min_x..=max_x {
            let dx_q8 = (x as i32 * 256) - cx_q8;
            let dist_sq = dy_sq + (dx_q8 as i64) * (dx_q8 as i64);
            if dist_sq <= r_sq_q8 {
                let off = row_off + (x as usize) * 4;
                buf[off] = blend_u8(sb, buf[off], alpha);
                buf[off + 1] = blend_u8(sg, buf[off + 1], alpha);
                buf[off + 2] = blend_u8(sr, buf[off + 2], alpha);
                buf[off + 3] = 255;
            }
        }
    }
}

/// Draw a hollow ring in BGRA with alpha blending. Same fixed-point treatment
/// as `fill_circle_bgra`.
#[cfg(target_os = "macos")]
#[allow(
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
fn draw_ring_bgra(
    buf: &mut [u8],
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    outer_radius: f64,
    thickness: f64,
    color: (u8, u8, u8),
    alpha: u8,
) {
    if alpha == 0 || outer_radius <= 0.0 || thickness <= 0.0 {
        return;
    }
    let stride = (width as usize) * 4;
    let inner = (outer_radius - thickness).max(0.0);

    let min_x = (cx - outer_radius).floor().max(0.0) as u32;
    let max_x = ((cx + outer_radius).ceil() as u32).min(width.saturating_sub(1));
    let min_y = (cy - outer_radius).floor().max(0.0) as u32;
    let max_y = ((cy + outer_radius).ceil() as u32).min(height.saturating_sub(1));

    if min_x > max_x || min_y > max_y {
        return;
    }

    let cx_q8 = (cx * 256.0) as i32;
    let cy_q8 = (cy * 256.0) as i32;
    let outer_q8 = (outer_radius * 256.0) as i32;
    let inner_q8 = (inner * 256.0) as i32;
    let outer_sq = (outer_q8 as i64) * (outer_q8 as i64);
    let inner_sq = (inner_q8 as i64) * (inner_q8 as i64);

    let (sb, sg, sr) = color;

    for y in min_y..=max_y {
        let dy_q8 = (y as i32 * 256) - cy_q8;
        let dy_sq = (dy_q8 as i64) * (dy_q8 as i64);
        if dy_sq > outer_sq {
            continue;
        }
        let row_off = (y as usize) * stride;
        for x in min_x..=max_x {
            let dx_q8 = (x as i32 * 256) - cx_q8;
            let dist_sq = dy_sq + (dx_q8 as i64) * (dx_q8 as i64);
            if dist_sq <= outer_sq && dist_sq >= inner_sq {
                let off = row_off + (x as usize) * 4;
                buf[off] = blend_u8(sb, buf[off], alpha);
                buf[off + 1] = blend_u8(sg, buf[off + 1], alpha);
                buf[off + 2] = blend_u8(sr, buf[off + 2], alpha);
                buf[off + 3] = 255;
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
        // Suppress periodic `frame=... fps=...` progress stats and chatty info
        // output: stderr is drained on a background thread, but keeping it
        // quiet means the retained tail holds actual errors, not progress spam.
        "-nostats".into(),
        "-loglevel".into(),
        "error".into(),
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
            // Streaming per-frame palettes: plain `palettegen` only emits its
            // palette at EOF, forcing ffmpeg to buffer every frame in memory
            // for the whole recording (unbounded growth). `stats_mode=single`
            // + `paletteuse=new=1` generate and apply a palette per frame.
            args.extend([
                "-vf".into(),
                "split[s0][s1];[s0]palettegen=stats_mode=single[p];[s1][p]paletteuse=new=1".into(),
                "-loop".into(),
                "0".into(),
            ]);
        }
    }

    // `--` ends ffmpeg's option parsing so `output_path` cannot be misread as a flag
    // (e.g. a path beginning with `-`). The output path is user-controlled via config.
    args.push("--".into());
    args.push(output_path);

    Command::new(ffmpeg_path)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped()) // Capture stderr for error diagnostics
        .spawn()
        .map_err(|e| RecordError::FfmpegSpawnFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapforge_storage::config::{RecordingFormat, RecordingQuality};
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
            show_clicks: false,
        }
    }

    #[test]
    fn test_recording_handle_stop_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::SeqCst));
        flag.store(true, Ordering::SeqCst);
        assert!(flag.load(Ordering::SeqCst));
    }

    /// Build a RecordingHandle with no backing thread so the state-machine
    /// methods (pause/resume/is_running) can be exercised without ffmpeg or a
    /// display. `thread: None` means stop()/drop() are no-ops.
    fn detached_handle() -> RecordingHandle {
        RecordingHandle {
            stop_flag: Arc::new(AtomicBool::new(false)),
            state: Arc::new(AtomicU8::new(STATE_RUNNING)),
            thread: None,
        }
    }

    #[test]
    fn handle_starts_running_not_paused() {
        let h = detached_handle();
        assert!(h.is_running());
        assert!(!h.is_paused());
    }

    #[test]
    fn handle_pause_then_resume_round_trips_state() {
        let h = detached_handle();
        h.pause();
        assert!(h.is_paused());
        assert!(!h.is_running());
        h.resume();
        assert!(h.is_running());
        assert!(!h.is_paused());
    }

    #[test]
    fn handle_stop_sets_flag_and_clears_running() {
        let h = detached_handle();
        let flag = h.stop_flag.clone();
        let state = h.state.clone();
        assert!(h.stop().is_ok());
        assert!(flag.load(Ordering::SeqCst), "stop must raise the stop flag");
        assert_eq!(
            state.load(Ordering::SeqCst),
            STATE_STOPPED,
            "stop must move state to STOPPED"
        );
    }

    #[test]
    fn handle_drop_raises_stop_flag() {
        // Dropping a handle without calling stop() must still signal the
        // capture thread to exit (prevents a runaway recording on early return).
        let h = detached_handle();
        let flag = h.stop_flag.clone();
        drop(h);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn validate_ffmpeg_rejects_non_ffmpeg_binary() {
        // Pointing at a real, runnable binary that isn't ffmpeg must produce a
        // spawn/identity error — not a false "ok" that later corrupts output.
        // `/bin/echo -version` prints "-version" and contains no "ffmpeg version".
        let echo = Path::new("/bin/echo");
        if !echo.exists() {
            return; // unusual platform; skip rather than assert on env
        }
        let result = validate_ffmpeg(echo);
        assert!(matches!(result, Err(RecordError::FfmpegSpawnFailed(_))));
    }

    #[test]
    fn validate_ffmpeg_unexecutable_path_errors() {
        // A path that can't be executed must surface FfmpegSpawnFailed.
        let result = validate_ffmpeg(Path::new("/nonexistent/definitely/not/ffmpeg"));
        assert!(matches!(result, Err(RecordError::FfmpegSpawnFailed(_))));
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
            show_clicks: false,
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
        if snapforge_capture::capture::capture_fullscreen(0).is_err() {
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let config = test_config(&tmp.path().join("recording.mp4"));

        // Capture or ffmpeg may fail in CI; only assert when recording starts.
        if let Ok(handle) = start_recording(config) {
            assert!(handle.is_running());
            std::thread::sleep(Duration::from_millis(500));
            assert!(handle.stop().is_ok());
        }
    }

    // --- blend_u8: pure integer alpha-blend correctness ---

    #[test]
    fn blend_u8_alpha_zero_keeps_destination() {
        // alpha = 0 means the source contributes nothing.
        assert_eq!(blend_u8(255, 17, 0), 17);
        assert_eq!(blend_u8(0, 200, 0), 200);
    }

    #[test]
    fn blend_u8_alpha_full_takes_source() {
        // alpha = 255 means the source fully replaces the destination.
        assert_eq!(blend_u8(200, 0, 255), 200);
        assert_eq!(blend_u8(0, 255, 255), 0);
    }

    #[test]
    fn blend_u8_half_alpha_is_midpoint_within_one_ulp() {
        // alpha = 128 (~0.5): result should sit within 1 of the exact average.
        let out = blend_u8(255, 0, 128);
        assert!(
            (127..=128).contains(&out),
            "expected ~128, got {} (drifted >1 ULP)",
            out
        );
    }

    #[test]
    fn blend_u8_never_drifts_more_than_one_from_exact() {
        // Exhaustively confirm the shift-based approximation stays within 1 ULP
        // of the exact round-to-nearest divide for every input combination.
        for src in 0u16..=255 {
            for dst in 0u16..=255 {
                for alpha in 0u16..=255 {
                    let exact = ((src * alpha + dst * (255 - alpha)) + 127) / 255;
                    let got = blend_u8(src as u8, dst as u8, alpha as u8) as u16;
                    let diff = exact.abs_diff(got);
                    assert!(
                        diff <= 1,
                        "blend_u8({src},{dst},{alpha}) = {got}, exact {exact}, diff {diff}"
                    );
                }
            }
        }
    }

    // --- crop_bgra_to_region_into: stride / resize / zero / huge handling ---

    #[cfg(target_os = "macos")]
    fn solid_bgra(w: u32, h: u32, byte: u8) -> Vec<u8> {
        vec![byte; (w as usize) * (h as usize) * 4]
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_no_region_same_size_is_exact_copy() {
        let src = solid_bgra(4, 3, 0xAB);
        let mut dst = vec![0u8; 4 * 3 * 4];
        crop_bgra_to_region_into(&src, 4, 3, None, 4, 3, &mut dst);
        assert_eq!(dst, src);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_no_region_source_larger_than_dst_zeroes_uncovered_rows() {
        // Source resized DOWN mid-session: dst smaller than src. Only the
        // top-left dst-sized window is copied; the rest stays zero.
        let src = solid_bgra(6, 6, 0xFF);
        let mut dst = vec![0x11u8; 4 * 4 * 4];
        crop_bgra_to_region_into(&src, 6, 6, None, 4, 4, &mut dst);
        // Every dst pixel should have been overwritten from the (larger) src.
        assert!(
            dst.iter().all(|&b| b == 0xFF),
            "dst not fully filled from src"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_no_region_source_smaller_than_dst_pads_with_zero() {
        // Source resized UP relative to dst: src smaller than dst. Uncovered
        // dst region must be zeroed, not left as stale bytes.
        let src = solid_bgra(2, 2, 0xFF);
        let mut dst = vec![0x55u8; 4 * 4 * 4];
        crop_bgra_to_region_into(&src, 2, 2, None, 4, 4, &mut dst);
        let dst_stride = 4 * 4;
        // Top-left 2x2 copied from src.
        for y in 0..2usize {
            for x in 0..2usize {
                let off = y * dst_stride + x * 4;
                assert_eq!(&dst[off..off + 4], &[0xFF; 4], "covered pixel wrong");
            }
        }
        // A pixel outside the covered window must be zero.
        let outside = 3 * dst_stride + 3 * 4;
        assert_eq!(
            &dst[outside..outside + 4],
            &[0u8; 4],
            "uncovered pixel not zeroed"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_region_copies_offset_window() {
        // 4x4 source where each pixel's blue byte encodes y*4+x; crop a 2x2
        // window at (1,1) and verify the right pixels land at dst origin.
        let (sw, sh) = (4u32, 4u32);
        let mut src = vec![0u8; (sw * sh * 4) as usize];
        for y in 0..sh as usize {
            for x in 0..sw as usize {
                let off = (y * sw as usize + x) * 4;
                src[off] = (y * 4 + x) as u8; // B channel = positional id
                src[off + 3] = 255;
            }
        }
        let region = snapforge_domain::Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        let mut dst = vec![0u8; 2 * 2 * 4];
        crop_bgra_to_region_into(&src, sw, sh, Some(&region), 2, 2, &mut dst);
        // dst(0,0) == src(1,1) -> id = 1*4+1 = 5
        assert_eq!(dst[0], 5);
        // dst(1,1) == src(2,2) -> id = 2*4+2 = 10
        // byte offset of dst pixel (1,1) in a 2-wide RGBA buffer:
        // (row*width + col)*4 = (1*2 + 1)*4 = 12
        let off = 12;
        assert_eq!(dst[off], 10);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_region_partially_outside_source_zeroes_margin() {
        // Region extends past the source bounds (e.g. display shrank mid
        // recording). The out-of-bounds margin must be zero, not a panic and
        // not leaked previous-frame data.
        let src = solid_bgra(4, 4, 0xFF);
        let region = snapforge_domain::Rect {
            x: 2,
            y: 2,
            width: 4,
            height: 4,
        };
        let mut dst = vec![0x33u8; 4 * 4 * 4];
        crop_bgra_to_region_into(&src, 4, 4, Some(&region), 4, 4, &mut dst);
        // Only the top-left 2x2 of dst is covered by src[2..4, 2..4].
        let dst_stride = 4 * 4;
        let covered = 0; // dst(0,0)
        assert_eq!(&dst[covered..covered + 4], &[0xFF; 4]);
        let margin = 3 * dst_stride + 3 * 4; // dst(3,3) — outside source
        assert_eq!(&dst[margin..margin + 4], &[0u8; 4], "margin not zeroed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_region_negative_origin_clamps_to_zero() {
        // A negative region origin must clamp to 0 rather than wrap or panic.
        let src = solid_bgra(4, 4, 0xFF);
        let region = snapforge_domain::Rect {
            x: -5,
            y: -5,
            width: 2,
            height: 2,
        };
        let mut dst = vec![0u8; 2 * 2 * 4];
        // Must not panic; clamped origin (0,0) copies real source bytes.
        crop_bgra_to_region_into(&src, 4, 4, Some(&region), 2, 2, &mut dst);
        assert_eq!(&dst[0..4], &[0xFF; 4]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn crop_region_fully_outside_source_yields_all_zero() {
        // Region entirely past the source: nothing to copy, all-zero output.
        let src = solid_bgra(4, 4, 0xFF);
        let region = snapforge_domain::Rect {
            x: 10,
            y: 10,
            width: 4,
            height: 4,
        };
        let mut dst = vec![0x99u8; 4 * 4 * 4];
        crop_bgra_to_region_into(&src, 4, 4, Some(&region), 4, 4, &mut dst);
        assert!(dst.iter().all(|&b| b == 0), "expected all-zero dst");
    }

    // --- fill_circle_bgra / draw_ring_bgra: bounds & no-op guards ---

    #[cfg(target_os = "macos")]
    #[test]
    fn fill_circle_zero_alpha_is_noop() {
        let mut buf = solid_bgra(8, 8, 0x10);
        let before = buf.clone();
        fill_circle_bgra(&mut buf, 8, 8, 4.0, 4.0, 3.0, (255, 255, 255), 0);
        assert_eq!(buf, before, "alpha=0 should not modify the buffer");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fill_circle_offscreen_center_does_not_panic_or_write() {
        // Center far off-screen with a small radius: clamped bounds are empty,
        // so nothing is drawn and indexing must stay in range.
        let mut buf = solid_bgra(8, 8, 0x10);
        let before = buf.clone();
        fill_circle_bgra(&mut buf, 8, 8, 1000.0, 1000.0, 3.0, (255, 0, 0), 200);
        assert_eq!(buf, before, "off-screen circle should be a no-op");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fill_circle_center_pixel_is_blended() {
        // A full-alpha white dot at the center must overwrite the center pixel.
        let mut buf = solid_bgra(8, 8, 0x00);
        fill_circle_bgra(&mut buf, 8, 8, 4.0, 4.0, 2.0, (255, 255, 255), 255);
        let stride = 8 * 4;
        let center = 4 * stride + 4 * 4;
        assert_eq!(&buf[center..center + 4], &[255, 255, 255, 255]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn draw_ring_zero_thickness_is_noop() {
        let mut buf = solid_bgra(8, 8, 0x10);
        let before = buf.clone();
        draw_ring_bgra(&mut buf, 8, 8, 4.0, 4.0, 3.0, 0.0, (255, 0, 0), 200);
        assert_eq!(buf, before, "zero-thickness ring should be a no-op");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn draw_clicks_empty_slice_is_noop() {
        let mut buf = solid_bgra(8, 8, 0x10);
        let before = buf.clone();
        draw_clicks_bgra(&mut buf, 8, 8, &[], None, 0.0, 0.0, 1.0, 1.0);
        assert_eq!(buf, before, "no clicks should leave the frame untouched");
    }
}
