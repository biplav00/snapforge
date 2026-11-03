use super::{RecordConfig, RecordError};
use snapforge_capture::capture;
#[cfg(target_os = "macos")]
use snapforge_capture::clicks::ClickTracker;
use snapforge_storage::config::{RecordingFormat, RecordingQuality};
use std::io::{BufWriter, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const STATE_RUNNING: u8 = 0;
const STATE_PAUSED: u8 = 1;
const STATE_STOPPED: u8 = 2;

/// How long the worker waits for ffmpeg to exit after stdin is closed on a
/// normal stop (it still has to flush the encoder and finish the container).
const FFMPEG_EXIT_DEADLINE: Duration = Duration::from_secs(20);
/// How long the worker waits for ffmpeg to exit after a frame write failed
/// (ffmpeg has usually already died — this just collects its exit status).
const FFMPEG_WRITE_ERROR_EXIT_DEADLINE: Duration = Duration::from_secs(5);
/// `stop()` waits this long for the worker before killing ffmpeg. Longer than
/// `FFMPEG_EXIT_DEADLINE` so the worker's own deadline normally fires first;
/// this one covers a worker stuck in `write_all` on a hung ffmpeg.
const STOP_DEADLINE: Duration = Duration::from_secs(25);
/// After killing ffmpeg, how long `stop()` waits for the worker to notice
/// before giving up on it (the thread is then detached, not joined).
const STOP_KILL_GRACE: Duration = Duration::from_secs(5);
/// A continuous run of failed frame captures lasting this long ends the
/// recording with `CaptureFailed` instead of silently freezing the video.
const CAPTURE_FAILURE_LIMIT: Duration = Duration::from_secs(3);
/// How long the macOS worker waits for the SCStream's first frame before
/// starting the file with a black frame.
#[cfg(target_os = "macos")]
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(2);

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

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

pub struct RecordingHandle {
    stop_flag: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    thread: Option<JoinHandle<Result<(), RecordError>>>,
    /// Shared with the worker so `stop()` can kill a hung ffmpeg. `Child::kill`
    /// is a no-op once the worker has reaped the process, so this can never
    /// signal a recycled PID.
    child: Option<Arc<Mutex<Child>>>,
}

impl RecordingHandle {
    /// Stop the recording and wait for ffmpeg to finalize the file. Returns
    /// the worker's terminal error if it had already failed (capture failure,
    /// ffmpeg exit, write error) — the error is preserved until this call.
    ///
    /// Bounded: if the worker hasn't finished after `STOP_DEADLINE`, ffmpeg is
    /// killed and an error is returned.
    pub fn stop(mut self) -> Result<(), RecordError> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<(), RecordError> {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.state.store(STATE_STOPPED, Ordering::SeqCst);
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };

        let started = Instant::now();
        let mut killed_at: Option<Instant> = None;
        while !thread.is_finished() {
            match killed_at {
                None if started.elapsed() >= STOP_DEADLINE => {
                    tracing::error!(
                        "[record] worker did not finish within {}s of stop; killing ffmpeg",
                        STOP_DEADLINE.as_secs()
                    );
                    if let Some(child) = &self.child {
                        let _ = lock(child).kill();
                    }
                    killed_at = Some(Instant::now());
                }
                Some(t) if t.elapsed() >= STOP_KILL_GRACE => {
                    // Worker is stuck somewhere killing ffmpeg can't unblock
                    // (e.g. inside the capture API). Detach it rather than
                    // hang the caller forever.
                    tracing::error!("[record] worker still stuck after killing ffmpeg; detaching");
                    return Err(RecordError::WriteFailed(
                        "recording did not stop in time; ffmpeg was killed".into(),
                    ));
                }
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let result = thread
            .join()
            .map_err(|_| RecordError::WriteFailed("thread panicked".into()))?;
        if killed_at.is_some() {
            return Err(RecordError::WriteFailed(
                "ffmpeg stopped responding; the recording was force-stopped and may be truncated"
                    .into(),
            ));
        }
        result
    }

    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_RUNNING
    }

    /// True while the worker is RUNNING or PAUSED; false once it has exited
    /// (normally or after an error). When this turns false on its own the
    /// recording has failed — call `stop()` to collect the error.
    pub fn is_alive(&self) -> bool {
        self.state.load(Ordering::SeqCst) != STATE_STOPPED
            && self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// RUNNING -> PAUSED. Returns true if the recording is now paused
    /// (including when it already was), false if it has stopped — a stopped
    /// state is never overwritten.
    pub fn pause(&self) -> bool {
        transition(&self.state, STATE_RUNNING, STATE_PAUSED)
    }

    /// PAUSED -> RUNNING. Returns true if the recording is now running
    /// (including when it already was), false if it has stopped.
    pub fn resume(&self) -> bool {
        transition(&self.state, STATE_PAUSED, STATE_RUNNING)
    }

    #[cfg(test)]
    pub fn is_paused(&self) -> bool {
        self.state.load(Ordering::SeqCst) == STATE_PAUSED
    }
}

/// Atomically move `state` from `from` to `to`. Succeeds (true) if the state
/// is `to` afterwards; never touches a STOPPED state.
fn transition(state: &AtomicU8, from: u8, to: u8) -> bool {
    match state.compare_exchange(from, to, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) => true,
        Err(current) => current == to,
    }
}

impl Drop for RecordingHandle {
    fn drop(&mut self) {
        // Ensure recording stops and ffmpeg process is reaped even if stop() wasn't called
        let _ = self.shutdown();
    }
}

pub fn start_recording(mut config: RecordConfig) -> Result<RecordingHandle, RecordError> {
    // Guard fps before it reaches `Duration::from_secs_f64(1.0 / fps)`: fps == 0
    // yields 1.0/0.0 = inf, which panics that constructor (and would cross the
    // panic back over the FFI boundary). Clamp to a sane range for corrupted
    // configs or bad FFI callers.
    config.fps = config.fps.clamp(1, 240);

    // Find + validate the FFmpeg binary (bundled or system). Cached after the
    // first success so the `which`/`-version` subprocesses don't run on the
    // caller's (Qt main) thread for every recording.
    let ffmpeg_path = resolve_ffmpeg(config.ffmpeg_path.as_ref())?;

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

/// `find_ffmpeg` + `validate_ffmpeg`, memoized per provided path. Only
/// successes are cached; a cached path that has since disappeared is
/// re-resolved.
fn resolve_ffmpeg(provided: Option<&PathBuf>) -> Result<PathBuf, RecordError> {
    type Cache = Option<(Option<PathBuf>, PathBuf)>;
    static CACHE: Mutex<Cache> = Mutex::new(None);

    if let Some((key, path)) = lock(&CACHE).as_ref() {
        if key.as_ref() == provided && path.exists() {
            return Ok(path.clone());
        }
    }
    let path = super::find_ffmpeg(provided)?;
    validate_ffmpeg(&path)?;
    *lock(&CACHE) = Some((provided.cloned(), path.clone()));
    Ok(path)
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

/// Make writes to ffmpeg's stdin fail with `EPIPE` instead of raising
/// SIGPIPE once ffmpeg has exited. This code ships as a staticlib inside the
/// Qt app, where (unlike a Rust binary) SIGPIPE is NOT ignored — without this
/// a write after ffmpeg dies (disk full, encoder error) kills the whole app.
#[cfg(target_os = "macos")]
const F_SETNOSIGPIPE: libc::c_int = 73; // <sys/fcntl.h>; not exported by libc 0.2
#[cfg(all(target_os = "macos", test))]
const F_GETNOSIGPIPE: libc::c_int = 74;

#[cfg(target_os = "macos")]
fn disable_sigpipe(stdin: &ChildStdin) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: plain fcntl on a valid, owned fd; no memory is passed.
    let rc = unsafe { libc::fcntl(stdin.as_raw_fd(), F_SETNOSIGPIPE, 1) };
    if rc == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// No per-fd SIGPIPE opt-out off macOS; the non-macOS path is only used by
/// Rust binaries/tests, where std already ignores SIGPIPE.
#[cfg(not(target_os = "macos"))]
fn disable_sigpipe(_stdin: &ChildStdin) -> std::io::Result<()> {
    Ok(())
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
/// The returned thread must be joined after ffmpeg exits (before reading the
/// buffer) so its final error lines aren't lost.
fn spawn_stderr_drain(
    stderr: Option<std::process::ChildStderr>,
) -> (Arc<Mutex<String>>, Option<JoinHandle<()>>) {
    let buf = Arc::new(Mutex::new(String::new()));
    let Some(mut err) = stderr else {
        return (buf, None);
    };
    let buf_clone = Arc::clone(&buf);
    let thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match err.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut guard = lock(&buf_clone);
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
    });
    (buf, Some(thread))
}

/// Build the error for a non-success ffmpeg exit using the drained stderr tail.
fn ffmpeg_exit_error(status: ExitStatus, stderr_buf: &Mutex<String>) -> RecordError {
    let buf = lock(stderr_buf).clone();
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

/// Poll the child for exit for up to `deadline`, then kill it. The lock is
/// only held for each non-blocking `try_wait`, so `stop()` can still kill
/// the child concurrently.
fn wait_with_deadline(child: &Mutex<Child>, deadline: Duration) -> Result<ExitStatus, RecordError> {
    let started = Instant::now();
    loop {
        {
            let mut c = lock(child);
            match c.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) => {}
                Err(e) => return Err(RecordError::WriteFailed(e.to_string())),
            }
            if started.elapsed() >= deadline {
                let _ = c.kill();
                let _ = c.wait();
                return Err(RecordError::WriteFailed(format!(
                    "ffmpeg did not finish within {}s; it was killed and the recording may be truncated",
                    deadline.as_secs()
                )));
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A running ffmpeg child plus its stderr drain. Dropping it kills and reaps
/// ffmpeg if it's still running, so no exit path can leave a zombie.
struct FfmpegProcess {
    child: Arc<Mutex<Child>>,
    stderr_buf: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
}

impl FfmpegProcess {
    /// Spawn ffmpeg and return it together with a buffered writer over its
    /// stdin (SIGPIPE disabled). Any failure after the spawn reaps the child.
    fn launch(
        ffmpeg_path: &Path,
        config: &RecordConfig,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        buf_capacity: usize,
    ) -> Result<(Self, BufWriter<ChildStdin>), RecordError> {
        let mut child = spawn_ffmpeg(ffmpeg_path, config, width, height, pixel_format)?;
        let Some(stdin) = child.stdin.take() else {
            kill_and_reap(child);
            return Err(RecordError::FfmpegSpawnFailed("no stdin".into()));
        };
        if let Err(e) = disable_sigpipe(&stdin) {
            drop(stdin);
            kill_and_reap(child);
            return Err(RecordError::FfmpegSpawnFailed(format!(
                "could not configure ffmpeg stdin pipe: {e}"
            )));
        }
        // Drain stderr from the start so ffmpeg can never block on a full pipe.
        let (stderr_buf, stderr_thread) = spawn_stderr_drain(child.stderr.take());
        Ok((
            Self {
                child: Arc::new(Mutex::new(child)),
                stderr_buf,
                stderr_thread,
            },
            BufWriter::with_capacity(buf_capacity, stdin),
        ))
    }

    fn join_stderr(&mut self) {
        if let Some(t) = self.stderr_thread.take() {
            let _ = t.join();
        }
    }

    /// stdin already closed: wait for ffmpeg to finish the file.
    fn finish(mut self) -> Result<(), RecordError> {
        let status = wait_with_deadline(&self.child, FFMPEG_EXIT_DEADLINE);
        self.join_stderr();
        let status = status?;
        if status.success() {
            Ok(())
        } else {
            Err(ffmpeg_exit_error(status, &self.stderr_buf))
        }
    }

    /// stdin already closed after a failed write: collect ffmpeg's exit
    /// status + stderr so the user sees *why* (e.g. "No space left on
    /// device") rather than just "broken pipe".
    fn write_error(mut self, e: &std::io::Error) -> RecordError {
        let status = wait_with_deadline(&self.child, FFMPEG_WRITE_ERROR_EXIT_DEADLINE);
        self.join_stderr();
        match status {
            Ok(s) if !s.success() => ffmpeg_exit_error(s, &self.stderr_buf),
            _ if e.kind() == std::io::ErrorKind::BrokenPipe => RecordError::WriteFailed(
                "ffmpeg stopped accepting frames mid-recording (broken pipe)".into(),
            ),
            _ => RecordError::WriteFailed(format!("writing frames to ffmpeg failed: {e}")),
        }
    }
}

impl Drop for FfmpegProcess {
    fn drop(&mut self) {
        {
            let mut c = lock(&self.child);
            if !matches!(c.try_wait(), Ok(Some(_))) {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        self.join_stderr();
    }
}

/// Why the capture loop ended early.
enum LoopError {
    /// Writing to ffmpeg's stdin failed (ffmpeg died, disk full, ...).
    Write(std::io::Error),
    /// Screen capture kept failing. ffmpeg is still finalized gracefully so
    /// the frames recorded so far are kept, but the error is returned.
    Capture(RecordError),
    /// Setup failure inside the worker; ffmpeg is killed.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    // constructed by the macOS loop only
    Fatal(RecordError),
}

/// Close stdin and turn the loop outcome + ffmpeg's exit into the worker's
/// result. Every exit path goes through here, so ffmpeg is always reaped.
fn finalize(
    writer: BufWriter<ChildStdin>,
    process: FfmpegProcess,
    outcome: Result<(), LoopError>,
) -> Result<(), RecordError> {
    let capture_error = match outcome {
        Ok(()) => None,
        Err(LoopError::Capture(e)) => Some(e),
        Err(LoopError::Write(e)) => {
            // Discard the buffer without flushing (the pipe is dead) and
            // close stdin before waiting on ffmpeg.
            drop(writer.into_parts());
            return Err(process.write_error(&e));
        }
        Err(LoopError::Fatal(e)) => {
            drop(writer.into_parts());
            drop(process);
            return Err(e);
        }
    };
    // Flush explicitly: BufWriter::drop swallows flush errors, which would
    // silently drop the buffered tail of the recording.
    let encoded = match writer.into_inner() {
        Ok(stdin) => {
            drop(stdin);
            process.finish()
        }
        Err(e) => {
            let (err, w) = e.into_parts();
            drop(w.into_parts());
            Err(process.write_error(&err))
        }
    };
    match capture_error {
        Some(e) => Err(e),
        None => encoded,
    }
}

/// Write `frame` `count` times.
fn write_repeated<W: Write>(w: &mut W, frame: &[u8], count: u64) -> std::io::Result<()> {
    for _ in 0..count {
        w.write_all(frame)?;
    }
    Ok(())
}

/// Number of output slots due at `effective_elapsed` (paused time excluded)
/// given `frame_count` frames written so far *including* the one about to be
/// written. Always at least 1; more than 1 means the loop stalled and the
/// gap must be filled to keep a constant frame rate.
fn frames_due(effective_elapsed: Duration, frame_interval: Duration, frame_count: u64) -> u64 {
    let elapsed_frames = (effective_elapsed.as_secs_f64() / frame_interval.as_secs_f64()) as u64;
    elapsed_frames.saturating_sub(frame_count - 1).max(1)
}

/// Constant-frame-rate clock that freezes while paused, so pausing leaves no
/// gap (and no duplicated frames) in the output.
struct FrameClock {
    start: Instant,
    interval: Duration,
    /// Frames written so far.
    frame_count: u64,
    paused_accum: Duration,
    paused_since: Option<Instant>,
}

impl FrameClock {
    /// Starts after the initial frame has been written.
    fn new(interval: Duration) -> Self {
        Self {
            start: Instant::now(),
            interval,
            frame_count: 1,
            paused_accum: Duration::ZERO,
            paused_since: None,
        }
    }

    fn pause_tick(&mut self) {
        if self.paused_since.is_none() {
            self.paused_since = Some(Instant::now());
        }
    }

    /// Returns true if the clock was paused and has just been resumed.
    fn resume(&mut self) -> bool {
        match self.paused_since.take() {
            Some(p0) => {
                self.paused_accum += p0.elapsed();
                true
            }
            None => false,
        }
    }

    /// Sleep until the next slot, then return how many frames are due (the
    /// first `n - 1` are catch-up copies of the previous frame).
    fn wait_next(&mut self) -> u64 {
        let target = self.start + self.paused_accum + self.interval * (self.frame_count + 1) as u32;
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target - now);
        }
        let effective = self
            .start
            .elapsed()
            .checked_sub(self.paused_accum)
            .unwrap_or_default();
        frames_due(effective, self.interval, self.frame_count + 1)
    }

    fn advance(&mut self, written: u64) {
        self.frame_count += written;
    }
}

/// Tracks a run of consecutive capture failures: logs (rate-limited) and
/// reports a terminal error once the run exceeds `CAPTURE_FAILURE_LIMIT`.
#[derive(Default)]
struct CaptureFailures {
    first: Option<Instant>,
    count: u64,
}

impl CaptureFailures {
    fn record_failure(&mut self, now: Instant, err: &capture::CaptureError) -> Option<RecordError> {
        // The display is gone (disconnected): no point waiting it out.
        if matches!(err, capture::CaptureError::NoDisplay(_)) {
            tracing::error!("[record] recording display disconnected: {err}");
            return Some(RecordError::CaptureFailed(format!(
                "the recorded display was disconnected ({err})"
            )));
        }
        let first = *self.first.get_or_insert(now);
        self.count += 1;
        // Log the 1st, 2nd, 4th, 8th, ... failure of a run.
        if self.count.is_power_of_two() {
            tracing::warn!(
                "[record] frame capture failed ({} in a row): {err}",
                self.count
            );
        }
        let streak = now.saturating_duration_since(first);
        (streak >= CAPTURE_FAILURE_LIMIT).then(|| {
            RecordError::CaptureFailed(format!(
                "screen capture failed {} times in a row over {:.1}s: {err}",
                self.count,
                streak.as_secs_f64()
            ))
        })
    }

    fn record_success(&mut self) {
        if self.count > 0 {
            tracing::info!(
                "[record] frame capture recovered after {} failures",
                self.count
            );
        }
        self.reset();
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// `width * height * 4` without overflow.
fn frame_byte_len(width: u32, height: u32) -> Result<usize, RecordError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4))
        .filter(|&n| n > 0)
        .ok_or_else(|| {
            RecordError::CaptureFailed(format!("invalid recording size {width}x{height}"))
        })
}

/// Clip `region` (native display pixels) to the display, then scale it into
/// the downscaled output frame and round to even (libx264/yuv420p needs even
/// dimensions). Errors if nothing usable remains.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // used by the macOS capture loop only
#[allow(clippy::cast_possible_wrap)]
fn scale_region_to_output(
    region: &snapforge_domain::Rect,
    native: (u32, u32),
    output: (u32, u32),
) -> Result<snapforge_domain::Rect, RecordError> {
    let (native_w, native_h) = (i64::from(native.0), i64::from(native.1));
    let (out_w, out_h) = output;
    if native_w == 0 || native_h == 0 || out_w == 0 || out_h == 0 {
        return Err(RecordError::CaptureFailed("display has zero size".into()));
    }
    let x0 = i64::from(region.x).clamp(0, native_w);
    let y0 = i64::from(region.y).clamp(0, native_h);
    let x1 = (i64::from(region.x) + i64::from(region.width)).clamp(0, native_w);
    let y1 = (i64::from(region.y) + i64::from(region.height)).clamp(0, native_h);
    if x1 <= x0 || y1 <= y0 {
        return Err(RecordError::CaptureFailed(
            "recording region lies outside the display".into(),
        ));
    }

    let sx = f64::from(out_w) / native_w as f64;
    let sy = f64::from(out_h) / native_h as f64;
    let rx = ((x0 as f64 * sx) as u32).min(out_w) & !1;
    let ry = ((y0 as f64 * sy) as u32).min(out_h) & !1;
    let rw = (((x1 - x0) as f64 * sx) as u32).min(out_w - rx) & !1;
    let rh = (((y1 - y0) as f64 * sy) as u32).min(out_h - ry) & !1;
    if rw == 0 || rh == 0 {
        return Err(RecordError::CaptureFailed(
            "recording region is too small".into(),
        ));
    }
    Ok(snapforge_domain::Rect {
        x: rx as i32,
        y: ry as i32,
        width: rw,
        height: rh,
    })
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
    frame_byte_len(width, height)?;
    let test_frame = if width != raw_test_frame.width() || height != raw_test_frame.height() {
        image::imageops::crop_imm(&raw_test_frame, 0, 0, width, height).to_image()
    } else {
        raw_test_frame
    };

    // 256KB buffer — better for pipe throughput than full-frame buffer
    let (process, mut writer) = FfmpegProcess::launch(
        &ffmpeg_path,
        &config,
        width,
        height,
        PixelFormat::Rgba,
        256 * 1024,
    )?;
    let child = Arc::clone(&process.child);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let state = Arc::new(AtomicU8::new(STATE_RUNNING));
    let state_clone = state.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let region = config.region;
    let display = config.display;

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        let outcome = (|| -> Result<(), LoopError> {
            // Cache the last successfully captured frame so we can duplicate it
            let mut last_frame: image::RgbaImage = test_frame;
            writer
                .write_all(last_frame.as_raw())
                .map_err(LoopError::Write)?;

            let mut clock = FrameClock::new(frame_interval);
            let mut failures = CaptureFailures::default();

            loop {
                if stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Pause: freeze the frame clock, skip capture & write entirely.
                if state_clone.load(Ordering::SeqCst) == STATE_PAUSED {
                    clock.pause_tick();
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                if clock.resume() {
                    failures.reset();
                }

                let due = clock.wait_next();
                // Catch-up after a stall: repeat the PREVIOUS frame for the
                // missed slots, then write the fresh capture in its own slot.
                write_repeated(&mut writer, last_frame.as_raw(), due - 1)
                    .map_err(LoopError::Write)?;

                let frame_result = if let Some(r) = &region {
                    capture::capture_region(display, *r)
                } else {
                    capture::capture_fullscreen(display)
                };
                match frame_result {
                    Ok(img) => {
                        failures.record_success();
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
                    Err(e) => {
                        if let Some(err) = failures.record_failure(Instant::now(), &e) {
                            return Err(LoopError::Capture(err));
                        }
                    }
                }

                // Any write error is fatal: a partial frame permanently
                // desyncs the raw stream.
                writer
                    .write_all(last_frame.as_raw())
                    .map_err(LoopError::Write)?;
                clock.advance(due);
            }
            Ok(())
        })();

        let result = finalize(writer, process, outcome);
        state_clone.store(STATE_STOPPED, Ordering::SeqCst);
        result
    });

    Ok(RecordingHandle {
        stop_flag,
        state,
        thread: Some(thread),
        child: Some(child),
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
    let capture_err =
        |e: snapforge_capture::capture::CaptureError| RecordError::CaptureFailed(e.to_string());

    // Cap output at ~1080p for recording to keep per-frame cost reasonable.
    // For screenshots we always use native resolution; this cap is recording-only.
    let max_dimension = Some(1920u32);

    // Compute output dimensions up front WITHOUT touching SCK on the main
    // thread — RecordingSource::start creates SCContentFilter / SCStream,
    // which is exactly what we want to keep off main.
    // Reading the display's pixel size via CGDisplayCopyDisplayMode is safe.
    let (full_width, full_height) =
        capture::macos::compute_recording_output_size(config.display, max_dimension)
            .map_err(capture_err)?;

    // Scale factor from native pixel coords → downscaled output coords.
    // The region from the frontend is in native pixels; we need to scale it.
    // Use the TARGET display's pixel size — the primary display's dimensions
    // would produce wrong scale factors (and wrong region crops) whenever
    // config.display is a secondary display.
    let (native_w_px, native_h_px) =
        capture::macos::display_pixel_size_for_index(config.display).map_err(capture_err)?;
    let native = (
        u32::try_from(native_w_px).unwrap_or(u32::MAX),
        u32::try_from(native_h_px).unwrap_or(u32::MAX),
    );
    let sx = if native.0 > 0 {
        f64::from(full_width) / f64::from(native.0)
    } else {
        1.0
    };
    let sy = if native.1 > 0 {
        f64::from(full_height) / f64::from(native.1)
    } else {
        1.0
    };

    let (width, height, region_in_scaled) = if let Some(r) = &config.region {
        let scaled = scale_region_to_output(r, native, (full_width, full_height))?;
        (scaled.width, scaled.height, Some(scaled))
    } else {
        (full_width, full_height, None)
    };
    let frame_bytes = frame_byte_len(width, height)?;

    // Clicks come in GLOBAL desktop points (CGEventGetLocation); convert to
    // scaled-frame pixels of the TARGET display: translate by the display's
    // origin, then scale by its point→pixel factor and the downscale ratio.
    // Queried before ffmpeg is spawned so a failure here can't leak a child.
    let native_scale =
        capture::macos::display_scale_factor_for(config.display).map_err(capture_err)?;
    let (display_origin_x, display_origin_y) =
        capture::macos::display_origin_points(config.display).map_err(capture_err)?;
    let point_to_scaled_pixel_x = native_scale * sx;
    let point_to_scaled_pixel_y = native_scale * sy;

    // Use a buffer roughly the size of one frame so each frame typically
    // flushes once (the previous 256 KiB caused 30+ flushes per 1080p
    // frame, which was strictly slower than no buffering).
    let buf_capacity = frame_bytes.clamp(1 << 20, 8 << 20); // 1–8 MiB
    let (process, mut writer) = FfmpegProcess::launch(
        &ffmpeg_path,
        &config,
        width,
        height,
        PixelFormat::Bgra,
        buf_capacity,
    )?;
    let child = Arc::clone(&process.child);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let state = Arc::new(AtomicU8::new(STATE_RUNNING));
    let state_clone = state.clone();
    let frame_interval = Duration::from_secs_f64(1.0 / config.fps as f64);
    let display = config.display;
    let fps = config.fps;

    // Click tracking — only when the "show clicks" pref is on. When off, the
    // tap is never started and `click_snapshot` stays empty, so the hot-path
    // below writes captured frames straight through with no overlay.
    let click_tracker = config.show_clicks.then(ClickTracker::new);
    let click_tap_handle = click_tracker
        .as_ref()
        .and_then(ClickTracker::start_macos_tap);

    let thread = std::thread::spawn(move || -> Result<(), RecordError> {
        // Keep the click event tap alive for the whole recording. Bound to the
        // thread's stack (not left at function scope, where it dropped — and so
        // disabled the CGEventTap — microseconds after start), so it keeps
        // feeding click_tracker until the capture loop exits.
        let _click_tap_handle = click_tap_handle;
        let outcome = (|| -> Result<(), LoopError> {
            // SCStream (frames pushed by SCK, newest kept in a slot) with an
            // automatic fallback to one SCScreenshotManager shot per frame.
            let mut source = capture::RecordingSource::start(display, max_dimension, fps)
                .map_err(|e| LoopError::Fatal(RecordError::CaptureFailed(e.to_string())))?;
            tracing::info!(
                "[record] capturing via {}",
                if source.is_stream() {
                    "SCStream"
                } else {
                    "per-frame screenshots (fallback)"
                }
            );

            // Two scratch buffers reused across the entire recording: one holds the
            // most recent captured (and cropped) frame, the other is the per-output
            // "composed" frame with clicks drawn over it. Pre-allocating eliminates
            // ~240 MB/s of allocator traffic at 1080p30.
            let mut captured: Vec<u8> = vec![0u8; frame_bytes];
            let mut composed: Vec<u8> = vec![0u8; frame_bytes];
            // Which buffer holds the last frame written to ffmpeg (repeated
            // for catch-up slots without re-composing the click overlay).
            let mut prev_is_composed = false;
            // Reused click snapshot — refreshed once per captured frame.
            let mut click_snapshot: Vec<snapforge_capture::clicks::ClickEvent> =
                Vec::with_capacity(32);

            // Capture the initial frame (the stream waits briefly for SCK's
            // first delivery).
            let first = source.first_frame(FIRST_FRAME_TIMEOUT, |f| {
                crop_bgra_to_region_into(
                    &f.bytes,
                    f.width,
                    f.height,
                    region_in_scaled.as_ref(),
                    width,
                    height,
                    &mut captured,
                );
            });
            match first {
                Ok(true) => {}
                Ok(false) => {
                    tracing::warn!(
                        "[record] no frame from the stream within {FIRST_FRAME_TIMEOUT:?}; recording starts with a black frame"
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
            writer.write_all(&captured).map_err(LoopError::Write)?;

            let mut clock = FrameClock::new(frame_interval);
            let mut failures = CaptureFailures::default();

            loop {
                if stop_clone.load(Ordering::SeqCst) {
                    break;
                }

                // Pause: freeze the frame clock, skip capture & write entirely.
                if state_clone.load(Ordering::SeqCst) == STATE_PAUSED {
                    clock.pause_tick();
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }
                if clock.resume() {
                    failures.reset();
                }

                let due = clock.wait_next();
                // Catch-up after a stall: repeat the PREVIOUS output frame
                // for the missed slots so new content isn't shown early.
                let prev: &[u8] = if prev_is_composed {
                    &composed
                } else {
                    &captured
                };
                write_repeated(&mut writer, prev, due - 1).map_err(LoopError::Write)?;

                // A stream with no new frame (screen idle) is a success: the
                // previous capture is still the current screen content.
                let frame = source.next_frame(|raw| {
                    crop_bgra_to_region_into(
                        &raw.bytes,
                        raw.width,
                        raw.height,
                        region_in_scaled.as_ref(),
                        width,
                        height,
                        &mut captured,
                    );
                });
                match frame {
                    Ok(_) => failures.record_success(),
                    Err(e) => {
                        // Keep the previous capture (the clicks overlay still
                        // animates over it); give up after a sustained run.
                        if let Some(err) = failures.record_failure(Instant::now(), &e) {
                            return Err(LoopError::Capture(err));
                        }
                    }
                }

                // Refresh the click snapshot and compose the overlay once per
                // captured frame. With no tracker (clicks off) the snapshot
                // stays empty and the overlay is skipped entirely.
                if let Some(tracker) = click_tracker.as_ref() {
                    tracker.recent_into(CLICK_LIFETIME_MS, &mut click_snapshot);
                }
                // Hot-path optimization: when no clicks are active, write the
                // captured frame directly. With clicks, copy into the scratch
                // buffer and overlay them in place.
                prev_is_composed = !click_snapshot.is_empty();
                let to_write: &[u8] = if prev_is_composed {
                    composed.copy_from_slice(&captured);
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
                } else {
                    &captured
                };
                // Any write error is fatal: a partial frame permanently
                // desyncs the raw stream.
                writer.write_all(to_write).map_err(LoopError::Write)?;
                clock.advance(due);
            }
            Ok(())
        })();

        let result = finalize(writer, process, outcome);
        state_clone.store(STATE_STOPPED, Ordering::SeqCst);
        result
    });

    Ok(RecordingHandle {
        stop_flag,
        state,
        thread: Some(thread),
        child: Some(child),
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
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // used by the macOS capture loop only
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

/// Build ffmpeg's argument list (pure; split out so it can be unit-tested).
fn ffmpeg_args(
    config: &RecordConfig,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
) -> Vec<String> {
    let crf = match config.quality {
        RecordingQuality::Low => "28",
        RecordingQuality::Medium => "23",
        RecordingQuality::High => "18",
    };

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
        format!("{}x{}", width, height),
        "-r".into(),
        config.fps.to_string(),
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
                // Convert RGB -> YUV with the BT.709 matrix (ffmpeg's default
                // is BT.601, which shifts colours) and tag the stream so
                // players decode it with the same matrix. `setparams` is
                // needed for primaries/trc to actually land in the stream;
                // the output flags below alone leave them "unknown".
                "-vf".into(),
                "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p,\
                 setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv"
                    .into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-colorspace".into(),
                "bt709".into(),
                "-color_primaries".into(),
                "bt709".into(),
                "-color_trc".into(),
                "bt709".into(),
                "-color_range".into(),
                "tv".into(),
                // Keyframe every 2s so fragments (below) stay short.
                "-g".into(),
                (config.fps * 2).to_string(),
                // Fragmented MP4: the file is playable up to the last
                // completed fragment even if ffmpeg or the app is killed
                // mid-recording (a regular MP4 has no `moov` until the end).
                // The file still starts with `ftyp` followed by an (empty)
                // `moov`.
                "-movflags".into(),
                "+frag_keyframe+empty_moov+default_base_moof".into(),
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
    args.push(config.output_path.to_string_lossy().to_string());
    args
}

fn spawn_ffmpeg(
    ffmpeg_path: &Path,
    config: &RecordConfig,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
) -> Result<Child, RecordError> {
    if let Some(parent) = config.output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| RecordError::WriteFailed(format!("failed to create output dir: {}", e)))?;
    }

    Command::new(ffmpeg_path)
        .args(ffmpeg_args(config, width, height, pixel_format))
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
            child: None,
        }
    }

    #[test]
    fn pause_resume_are_idempotent() {
        let h = detached_handle();
        assert!(h.pause());
        assert!(h.pause(), "pausing a paused recording is still paused");
        assert!(h.is_paused());
        assert!(h.resume());
        assert!(h.resume(), "resuming a running recording is still running");
        assert!(h.is_running());
    }

    #[test]
    fn pause_resume_never_overwrite_stopped() {
        // The worker marks STOPPED when it exits (e.g. after an error); a
        // late pause/resume from the UI must not resurrect the state.
        let h = detached_handle();
        h.state.store(STATE_STOPPED, Ordering::SeqCst);
        assert!(!h.pause());
        assert_eq!(h.state.load(Ordering::SeqCst), STATE_STOPPED);
        assert!(!h.resume());
        assert_eq!(h.state.load(Ordering::SeqCst), STATE_STOPPED);
        assert!(!h.is_alive());
    }

    #[test]
    fn is_alive_tracks_worker_thread() {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let state = Arc::new(AtomicU8::new(STATE_RUNNING));
        let (flag, st) = (stop_flag.clone(), state.clone());
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            st.store(STATE_STOPPED, Ordering::SeqCst);
            Err(RecordError::CaptureFailed("boom".into()))
        });
        let h = RecordingHandle {
            stop_flag: stop_flag.clone(),
            state,
            thread: Some(thread),
            child: None,
        };
        assert!(h.is_alive());
        assert!(h.pause());
        assert!(h.is_alive(), "paused is still alive");
        // Worker exits on its own with an error.
        stop_flag.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(5);
        while h.is_alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!h.is_alive());
        // ... and stop() still returns the worker's terminal error.
        assert!(matches!(h.stop(), Err(RecordError::CaptureFailed(_))));
    }

    #[test]
    fn frames_due_on_time_is_one() {
        let iv = Duration::from_millis(100);
        // 2nd frame due at 200ms (clock starts after frame 1 is written).
        assert_eq!(frames_due(Duration::from_millis(200), iv, 2), 1);
        // Early wakeup still writes exactly one frame.
        assert_eq!(frames_due(Duration::from_millis(50), iv, 2), 1);
    }

    #[test]
    fn frames_due_after_stall_counts_missed_slots() {
        let iv = Duration::from_millis(100);
        // 500ms elapsed but only frame 2 is being written: slots 2..=5 due.
        assert_eq!(frames_due(Duration::from_millis(500), iv, 2), 4);
    }

    #[test]
    fn catch_up_repeats_previous_frame_before_new_one() {
        // Mirrors the loop: (due - 1) copies of the previous frame, then the
        // freshly captured frame — new content must not appear early.
        let prev = [1u8, 1];
        let new = [2u8, 2];
        let mut out = Vec::new();
        let due = 3;
        write_repeated(&mut out, &prev, due - 1).unwrap();
        out.write_all(&new).unwrap();
        assert_eq!(out, vec![1, 1, 1, 1, 2, 2]);

        let mut none = Vec::new();
        write_repeated(&mut none, &prev, 0).unwrap();
        assert_eq!(none, Vec::<u8>::new());
    }

    #[test]
    fn capture_failures_end_recording_only_after_limit() {
        let mut f = CaptureFailures::default();
        let t0 = Instant::now();
        let e = capture::CaptureError::CaptureFailed;
        assert!(f.record_failure(t0, &e).is_none());
        assert!(f
            .record_failure(t0 + Duration::from_millis(2900), &e)
            .is_none());
        let err = f.record_failure(t0 + CAPTURE_FAILURE_LIMIT, &e);
        assert!(
            matches!(err, Some(RecordError::CaptureFailed(ref m)) if m.contains("screen capture failed"))
        );
    }

    #[test]
    fn capture_no_display_ends_recording_immediately() {
        let mut f = CaptureFailures::default();
        let err = f.record_failure(Instant::now(), &capture::CaptureError::NoDisplay(1));
        assert!(
            matches!(err, Some(RecordError::CaptureFailed(ref m)) if m.contains("disconnected"))
        );
    }

    #[test]
    fn capture_success_resets_failure_streak() {
        let mut f = CaptureFailures::default();
        let t0 = Instant::now();
        let e = capture::CaptureError::CaptureFailed;
        assert!(f.record_failure(t0, &e).is_none());
        f.record_success();
        // A new streak starts from this failure, not t0.
        assert!(f.record_failure(t0 + Duration::from_secs(10), &e).is_none());
    }

    #[test]
    fn frame_byte_len_rejects_overflow_and_empty() {
        assert_eq!(frame_byte_len(2, 3).unwrap(), 24);
        assert!(matches!(
            frame_byte_len(0, 10),
            Err(RecordError::CaptureFailed(_))
        ));
        #[cfg(target_pointer_width = "32")]
        assert!(frame_byte_len(u32::MAX, u32::MAX).is_err());
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            frame_byte_len(u32::MAX, u32::MAX).ok(),
            (u32::MAX as usize)
                .checked_mul(u32::MAX as usize)
                .and_then(|n| n.checked_mul(4))
        );
    }

    fn rect(x: i32, y: i32, width: u32, height: u32) -> snapforge_domain::Rect {
        snapforge_domain::Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn region_inside_display_scales_and_rounds_even() {
        // 4K native -> 1080p output: factor 0.5.
        let r =
            scale_region_to_output(&rect(101, 201, 801, 601), (3840, 2160), (1920, 1080)).unwrap();
        assert_eq!(r, rect(50, 100, 400, 300));
    }

    #[test]
    fn region_is_clipped_to_display() {
        // Extends past the right/bottom edge and starts at a negative origin.
        let r = scale_region_to_output(&rect(-100, -50, 5000, 5000), (1920, 1080), (1920, 1080))
            .unwrap();
        assert_eq!(r, rect(0, 0, 1920, 1080));

        let r = scale_region_to_output(&rect(1800, 1000, 400, 400), (1920, 1080), (1920, 1080))
            .unwrap();
        assert_eq!(r, rect(1800, 1000, 120, 80));
        assert!(r.x as u32 + r.width <= 1920 && r.y as u32 + r.height <= 1080);
    }

    #[test]
    fn region_outside_or_degenerate_errors() {
        let out = |r| scale_region_to_output(&r, (1920, 1080), (1920, 1080));
        assert!(matches!(
            out(rect(5000, 0, 10, 10)),
            Err(RecordError::CaptureFailed(_))
        ));
        assert!(matches!(
            out(rect(-500, 0, 100, 100)),
            Err(RecordError::CaptureFailed(_))
        ));
        assert!(matches!(
            out(rect(10, 10, 1, 1)),
            Err(RecordError::CaptureFailed(_))
        ));
        assert!(matches!(
            out(rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX)),
            Err(RecordError::CaptureFailed(_))
        ));
        assert!(scale_region_to_output(&rect(0, 0, 10, 10), (0, 0), (1920, 1080)).is_err());
    }

    #[test]
    fn mp4_args_tag_bt709_and_fragment() {
        let cfg = test_config(Path::new("/tmp/x.mp4"));
        #[cfg(target_os = "macos")]
        let pf = PixelFormat::Bgra;
        #[cfg(not(target_os = "macos"))]
        let pf = PixelFormat::Rgba;
        let args = ffmpeg_args(&cfg, 64, 48, pf);
        let joined = args.join(" ");
        assert!(joined.contains("out_color_matrix=bt709"));
        assert!(joined.contains("-colorspace bt709"));
        assert!(joined.contains("-color_primaries bt709"));
        assert!(joined.contains("-color_trc bt709"));
        assert!(joined.contains("+frag_keyframe+empty_moov+default_base_moof"));
        assert_eq!(args.iter().filter(|a| *a == "-vf").count(), 1);
        assert_eq!(args.last().map(String::as_str), Some("/tmp/x.mp4"));

        let mut gif = cfg.clone();
        gif.format = RecordingFormat::Gif;
        let joined = ffmpeg_args(&gif, 64, 48, pf).join(" ");
        assert!(!joined.contains("movflags"));
        assert!(!joined.contains("bt709"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ffmpeg_stdin_has_sigpipe_disabled() {
        // `cat` stands in for ffmpeg: we only need a child with a stdin pipe.
        use std::os::fd::AsRawFd;
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        disable_sigpipe(&stdin).unwrap();
        let flag = unsafe { libc::fcntl(stdin.as_raw_fd(), F_GETNOSIGPIPE) };
        assert_eq!(flag, 1);
        drop(stdin);
        let _ = child.wait();
    }

    #[test]
    fn write_after_child_exit_reports_ffmpeg_error() {
        // A child that exits non-zero with a message on stderr: the write
        // error path must surface that message, not just "broken pipe".
        let mut child = Command::new("/bin/sh")
            .args(["-c", "echo 'No space left on device' >&2; exit 1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        disable_sigpipe(&stdin).unwrap();
        let (stderr_buf, stderr_thread) = spawn_stderr_drain(child.stderr.take());
        let process = FfmpegProcess {
            child: Arc::new(Mutex::new(child)),
            stderr_buf,
            stderr_thread,
        };
        // Write until the pipe breaks (the child never reads).
        let chunk = vec![0u8; 64 * 1024];
        let err = loop {
            if let Err(e) = stdin.write_all(&chunk) {
                break e;
            }
        };
        drop(stdin);
        let msg = process.write_error(&err).to_string();
        assert!(msg.contains("No space left on device"), "got: {msg}");
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
        assert!(h.pause());
        assert!(h.is_paused());
        assert!(!h.is_running());
        assert!(h.resume());
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
            match handle.stop() {
                Ok(()) => {}
                // SCK context creation needs Screen Recording permission for
                // the test binary, which CI / plain `cargo test` may lack.
                Err(RecordError::CaptureFailed(e)) => {
                    eprintln!("skipping: capture unavailable in this environment: {e}");
                }
                Err(e) => panic!("stop failed: {e:?}"),
            }
        }
    }

    /// End-to-end: record 2 s of display 0 through the real capture path
    /// (SCStream, or its screenshot fallback) and check the MP4 with
    /// `ffprobe`. Needs Screen Recording permission for the test binary and
    /// ffmpeg/ffprobe on PATH, so it's opt-in:
    /// `cargo test -p snapforge-encode -- --ignored e2e_record_two_seconds --nocapture`
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "needs Screen Recording permission, a display, ffmpeg and ffprobe"]
    fn e2e_record_two_seconds() {
        assert!(
            snapforge_capture::capture::has_permission(),
            "the test binary lacks Screen Recording permission (System Settings > Privacy & Security > Screen Recording)"
        );
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("e2e.mp4");
        let mut config = test_config(&out);
        config.fps = 30;
        let handle = start_recording(config).expect("recording should start");
        std::thread::sleep(Duration::from_secs(2));
        handle.stop().expect("recording should stop cleanly");

        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-count_frames",
                "-show_entries",
                "stream=width,height,pix_fmt,nb_read_frames,color_space,color_primaries,color_transfer,color_range",
                "-of",
                "default=noprint_wrappers=1",
            ])
            .arg(&out)
            .output()
            .expect("ffprobe should run");
        let text = String::from_utf8_lossy(&probe.stdout).to_string();
        eprintln!("ffprobe:\n{text}");
        assert!(
            probe.status.success(),
            "ffprobe failed: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        let field = |k: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .unwrap_or_default()
                .to_string()
        };
        let frames: u64 = field("nb_read_frames").parse().unwrap_or(0);
        assert!(
            (45..=75).contains(&frames),
            "expected ~60 frames at 30 fps, got {frames}"
        );
        assert_eq!(field("pix_fmt"), "yuv420p");
        assert_eq!(field("color_primaries"), "bt709");
        assert_eq!(field("color_transfer"), "bt709");
        assert_eq!(field("color_space"), "bt709");
        assert_eq!(field("color_range"), "tv");
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
