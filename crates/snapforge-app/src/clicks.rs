//! Click-tracking use case.
//!
//! Wraps the macOS `ClickTracker` event tap and exposes a closure-based API
//! that fires on every global mouse click. The tracker pushes each click into
//! a channel as it happens; a forwarder thread blocks on that channel and
//! invokes the user-supplied callback — no polling, no dedup watermark.
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

use std::sync::mpsc;
use std::thread;

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
    forwarder: Option<thread::JoinHandle<()>>,
    // The tap handle drops the underlying CGEventTap; keep it alive for the
    // lifetime of the use case, and drop it explicitly in `Drop` to close the
    // event channel. Only present on macOS.
    #[cfg(target_os = "macos")]
    tap: Option<MacOSClickTapHandle>,
}

impl Drop for ClickHandle {
    fn drop(&mut self) {
        // Drop the tap FIRST. It holds the last `ClickTracker` clone, so
        // tearing it down releases the event Sender, which disconnects the
        // channel and lets the forwarder's blocking `recv()` return. Joining
        // before this would deadlock the forwarder against a live channel.
        #[cfg(target_os = "macos")]
        {
            self.tap.take();
        }
        if let Some(j) = self.forwarder.take() {
            let _ = j.join();
        }
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
    // Each click is pushed onto this channel by `ClickTracker::add` (called
    // from the tap's run-loop thread). The Sender lives inside the tracker,
    // shared by its clones — chiefly the one the macOS tap owns — so the
    // channel stays open exactly as long as the tap does.
    let (tx, rx) = mpsc::channel::<snapforge_capture::clicks::ClickEvent>();
    tracker.set_sink(tx);

    #[cfg(target_os = "macos")]
    let tap = tracker.start_macos_tap().ok_or_else(|| {
        AppError::InvalidRequest(
            "failed to start click event tap (Accessibility permission required)".into(),
        )
    })?;

    let forwarder = thread::Builder::new()
        .name("snapforge-click-forwarder".into())
        .spawn(move || {
            // Block until each click arrives — event-driven, no poll loop and
            // no dedup watermark. The loop ends when the last Sender drops
            // (tap teardown), which disconnects the channel.
            while let Ok(ev) = rx.recv() {
                callback(ClickEvent {
                    x: ev.x,
                    y: ev.y,
                    right_click: ev.right_click,
                });
            }
        })
        .map_err(|e| AppError::InvalidRequest(format!("failed to spawn forwarder: {}", e)))?;

    Ok(ClickHandle {
        forwarder: Some(forwarder),
        #[cfg(target_os = "macos")]
        tap: Some(tap),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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
