//! Click-tracking use case.
//!
//! Wraps the macOS `ClickTracker` event tap and exposes a closure-based API
//! that fires on every global mouse click. Internally a forwarder thread
//! polls the tracker's queue and invokes the user-supplied callback.
//!
//! ## Threading
//!
//! The callback executes on a thread owned by this use case (the forwarder
//! thread). Callers running inside a GUI framework (Qt, AppKit, etc.) must
//! dispatch back to the main thread themselves — the callback must not call
//! into GUI APIs directly.

use snapforge_capture::clicks::ClickTracker;
#[cfg(target_os = "macos")]
use snapforge_capture::clicks::MacOSClickTapHandle;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::AppError;

/// A click event delivered to the use-case callback. Mirrors
/// [`snapforge_capture::clicks::ClickEvent`] minus the internal `Instant`
/// (consumers like the FFI only need coordinates + button).
#[derive(Debug, Clone, Copy)]
pub struct ClickEvent {
    pub x: f64,
    pub y: f64,
    pub right_click: bool,
}

/// Opaque handle. Drop to stop the event tap and join the forwarder thread.
pub struct ClickHandle {
    stop: Arc<AtomicBool>,
    forwarder: Option<thread::JoinHandle<()>>,
    // The tap handle drops the underlying CGEventTap; keep it alive for the
    // lifetime of the use case. Only present on macOS.
    #[cfg(target_os = "macos")]
    _tap: Option<MacOSClickTapHandle>,
}

impl Drop for ClickHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.forwarder.take() {
            let _ = j.join();
        }
        // _tap drops here, which disables the CGEventTap and joins its thread.
    }
}

/// Begin streaming global click events to `callback`.
///
/// On macOS this requires Accessibility permission; if the permission is
/// missing the underlying `start_macos_tap` returns `None` and this function
/// returns an error.
///
/// On non-macOS targets the tap is currently a no-op (the forwarder thread
/// still spins but never produces events) — kept compiling so the FFI layer
/// has a single code path.
pub fn start_click_tracking<F>(callback: F) -> Result<ClickHandle, AppError>
where
    F: Fn(ClickEvent) + Send + 'static,
{
    let tracker = ClickTracker::new();

    #[cfg(target_os = "macos")]
    let tap = tracker.start_macos_tap().ok_or_else(|| {
        AppError::InvalidRequest(
            "failed to start click event tap (Accessibility permission required)".into(),
        )
    })?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let tracker_thread = tracker.clone();

    let forwarder = thread::Builder::new()
        .name("snapforge-click-forwarder".into())
        .spawn(move || {
            // Watermark of the last event we've already forwarded, so we don't
            // re-emit a click on every poll. `Instant` is monotonic, which is
            // exactly what we want for a "have I seen this yet" check.
            let mut last_seen: Option<Instant> = None;
            let mut buf: Vec<snapforge_capture::clicks::ClickEvent> = Vec::with_capacity(32);
            while !stop_thread.load(Ordering::SeqCst) {
                // Look back over the last second — events older than that are
                // ones the tracker would drop anyway, and we only care about
                // brand-new ones (filtered by `last_seen`).
                tracker_thread.recent_into(1000, &mut buf);
                for ev in &buf {
                    if last_seen.is_none_or(|t| ev.timestamp > t) {
                        callback(ClickEvent {
                            x: ev.x,
                            y: ev.y,
                            right_click: ev.right_click,
                        });
                        last_seen = Some(ev.timestamp);
                    }
                }
                thread::sleep(Duration::from_millis(16));
            }
        })
        .map_err(|e| AppError::InvalidRequest(format!("failed to spawn forwarder: {}", e)))?;

    Ok(ClickHandle {
        stop,
        forwarder: Some(forwarder),
        #[cfg(target_os = "macos")]
        _tap: Some(tap),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[ignore = "real event tap needs Accessibility permission; not available in CI/sandbox"]
    #[test]
    fn smoke_start_drop() {
        let received: Arc<Mutex<Vec<ClickEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let r = Arc::clone(&received);
        let h = start_click_tracking(move |ev| {
            if let Ok(mut v) = r.lock() {
                v.push(ev);
            }
        })
        .expect("requires Accessibility permission");
        thread::sleep(Duration::from_millis(50));
        drop(h);
    }
}
