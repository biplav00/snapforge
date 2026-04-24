//! Click tracking for recording overlays.
//!
//! Captures global mouse click events via CGEventTap on macOS.
//! Maintains a queue of recent clicks that can be composited into recording frames.

use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct ClickEvent {
    /// Screen x coordinate in physical pixels
    pub x: f64,
    /// Screen y coordinate in physical pixels
    pub y: f64,
    /// When the click occurred
    pub timestamp: Instant,
}

/// Thread-safe queue of recent click events.
#[derive(Clone, Default)]
pub struct ClickTracker {
    clicks: Arc<Mutex<Vec<ClickEvent>>>,
}

impl ClickTracker {
    pub fn new() -> Self {
        Self {
            clicks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a click to the queue.
    pub fn add(&self, x: f64, y: f64) {
        if let Ok(mut clicks) = self.clicks.lock() {
            clicks.push(ClickEvent {
                x,
                y,
                timestamp: Instant::now(),
            });
            // Keep only the last 64 clicks to bound memory
            if clicks.len() > 64 {
                let drop = clicks.len() - 64;
                clicks.drain(0..drop);
            }
        }
    }

    /// Get clicks from within the last `max_age_ms` milliseconds.
    pub fn recent(&self, max_age_ms: u64) -> Vec<ClickEvent> {
        let Ok(clicks) = self.clicks.lock() else {
            return Vec::new();
        };
        let now = Instant::now();
        clicks
            .iter()
            .filter(|c| now.duration_since(c.timestamp).as_millis() <= max_age_ms as u128)
            .copied()
            .collect()
    }

    /// Start the macOS event tap that feeds this tracker.
    /// Returns a handle that stops the tap when dropped.
    /// Requires Accessibility permission.
    #[cfg(target_os = "macos")]
    pub fn start_macos_tap(&self) -> Option<MacOSClickTapHandle> {
        macos_tap::start(self.clone())
    }
}

#[cfg(target_os = "macos")]
pub use macos_tap::{has_accessibility_permission, MacOSClickTapHandle};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_new_and_recent_empty() {
        let t = ClickTracker::new();
        let r = t.recent(1000);
        assert!(r.is_empty());
    }

    #[test]
    fn test_tracker_add_and_recent() {
        let t = ClickTracker::new();
        t.add(10.0, 20.0);
        let r = t.recent(10_000);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].x, 10.0);
    }

    // Tap creation requires Accessibility permission and a live CGSession;
    // skip in CI / sandbox by default.
    #[cfg(target_os = "macos")]
    #[ignore]
    #[test]
    fn test_macos_tap_start_drop() {
        let t = ClickTracker::new();
        let h = t.start_macos_tap();
        drop(h);
    }
}

/// Check if click tracking is available (Accessibility permission granted on macOS).
pub fn has_click_tracking_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        has_accessibility_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[cfg(target_os = "macos")]
mod macos_tap {
    use super::ClickTracker;
    use std::ffi::c_void;
    use std::mem::ManuallyDrop;
    use std::ptr;
    use std::sync::mpsc;
    use std::sync::Arc;

    // CGEventTap types
    type CFMachPortRef = *mut c_void;
    type CFRunLoopSourceRef = *mut c_void;
    type CFRunLoopRef = *mut c_void;
    type CFStringRef = *const c_void;
    type CGEventRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type CGEventMask = u64;
    type CGEventType = u32;

    const KCG_HID_EVENT_TAP: u32 = 0;
    const KCG_TAIL_APPEND_EVENT_TAP: u32 = 1;
    const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
    const KCG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const KCG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    /// Emitted when CG disables our tap because our callback took too long
    /// OR because Accessibility permission was revoked. Value: 0xFFFFFFFE.
    const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    /// Emitted when the tap was disabled via `CGEventTapEnable(false)`.
    const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
    /// If we fail to re-enable this many times in a row, give up — otherwise
    /// we could spin forever on a permanently revoked permission.
    const MAX_REENABLE_ATTEMPTS: u32 = 5;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    type CGEventTapCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    type CFDictionaryRef = *const c_void;

    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: CGEventMask,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        fn CFRunLoopRun();
        fn CFRunLoopStop(rl: CFRunLoopRef);
        fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
        fn CFRelease(cf: *const c_void);
        static kCFRunLoopCommonModes: CFStringRef;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }

    /// Check if Accessibility permission is granted.
    pub fn has_accessibility_permission() -> bool {
        unsafe { AXIsProcessTrustedWithOptions(ptr::null()) }
    }

    /// Wrapper to make CF pointers Send/Sync — we only use them to signal stop.
    #[derive(Copy, Clone)]
    struct SendableRunLoop(CFRunLoopRef);
    unsafe impl Send for SendableRunLoop {}
    unsafe impl Sync for SendableRunLoop {}

    #[derive(Copy, Clone)]
    struct SendableTap(CFMachPortRef);
    unsafe impl Send for SendableTap {}
    unsafe impl Sync for SendableTap {}

    pub struct MacOSClickTapHandle {
        run_loop: SendableRunLoop,
        tap: SendableTap,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for MacOSClickTapHandle {
        fn drop(&mut self) {
            unsafe {
                // Disable the tap first so no more events fire.
                CGEventTapEnable(self.tap.0, false);
                // Stop the thread's CFRunLoopRun.
                CFRunLoopStop(self.run_loop.0);
            }
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    struct CallbackData {
        tracker: ClickTracker,
        tap: std::sync::atomic::AtomicPtr<c_void>,
        reenable_attempts: std::sync::atomic::AtomicU32,
    }

    unsafe extern "C" fn event_callback(
        _proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef {
        if user_info.is_null() {
            return event;
        }
        // user_info is a *mut Arc<CallbackData>. We only want a borrow — never
        // drop it here; the owning thread will drop it on teardown.
        let arc_ptr = user_info as *const Arc<CallbackData>;
        let data = ManuallyDrop::new(unsafe { std::ptr::read(arc_ptr) });

        // Handle tap-disabled notifications. macOS disables a tap when our
        // callback exceeds the time budget or when Accessibility permission
        // is revoked mid-session; we have to explicitly re-enable it.
        if event_type == KCG_EVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == KCG_EVENT_TAP_DISABLED_BY_USER_INPUT
        {
            let tap = data.tap.load(std::sync::atomic::Ordering::SeqCst);
            if !tap.is_null() {
                let attempts = data
                    .reenable_attempts
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                if attempts <= MAX_REENABLE_ATTEMPTS {
                    CGEventTapEnable(tap, true);
                    eprintln!(
                        "[clicks] event tap disabled (type 0x{:x}); re-enabled (attempt {})",
                        event_type, attempts
                    );
                } else {
                    eprintln!(
                        "[clicks] event tap disabled (type 0x{:x}); giving up after {} attempts — \
                         Accessibility permission may have been revoked",
                        event_type, attempts
                    );
                }
            }
            return event;
        }

        // NOTE: the re-enable counter is intentionally monotonic. Resetting it
        // on a healthy click lets a permanently-denied permission loop forever
        // (revoke → re-enable fails → click from another app → reset → revoke
        // again). After MAX_REENABLE_ATTEMPTS permanent failures we log once
        // and stop retrying; the tap stays disabled and the tracker goes quiet.
        let loc = CGEventGetLocation(event);
        data.tracker.add(loc.x, loc.y);
        event
    }

    /// Result of the tap-creation step, sent from the worker thread back to `start`.
    enum SetupResult {
        Ok {
            run_loop: SendableRunLoop,
            tap: SendableTap,
        },
        Err,
    }

    pub fn start(tracker: ClickTracker) -> Option<MacOSClickTapHandle> {
        let (tx, rx) = mpsc::sync_channel::<SetupResult>(1);

        let thread = std::thread::spawn(move || {
            // Additionally listen for tap-disabled events so we can re-enable
            // on timeout / revoked permission. CGEventMask is a bit field of
            // CGEventType values; both disable codes are > 63 so we can't OR
            // them into the mask, but CG delivers them unconditionally when
            // they fire on our tap.
            let mask: CGEventMask =
                (1u64 << KCG_EVENT_LEFT_MOUSE_DOWN) | (1u64 << KCG_EVENT_RIGHT_MOUSE_DOWN);

            let data = Arc::new(CallbackData {
                tracker,
                tap: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()),
                reenable_attempts: std::sync::atomic::AtomicU32::new(0),
            });
            // Heap-allocate a copy of the Arc that the C callback will borrow.
            // We own this raw pointer on this thread and free it after CFRunLoopRun returns.
            let data_raw: *mut Arc<CallbackData> = Box::into_raw(Box::new(Arc::clone(&data)));

            let tap = unsafe {
                CGEventTapCreate(
                    KCG_HID_EVENT_TAP,
                    KCG_TAIL_APPEND_EVENT_TAP,
                    KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
                    mask,
                    event_callback,
                    data_raw.cast::<c_void>(),
                )
            };

            if tap.is_null() {
                eprintln!("[clicks] CGEventTapCreate failed — accessibility permission required");
                // Free the raw Arc pointer we created.
                unsafe {
                    let _ = Box::from_raw(data_raw);
                }
                let _ = tx.send(SetupResult::Err);
                return;
            }

            // Publish the tap pointer BEFORE wiring the run loop source and
            // enabling the tap. If CG dispatches a disable-notification the
            // instant the source is added, the callback must be able to find
            // the tap to re-enable it.
            data.tap.store(tap, std::sync::atomic::Ordering::SeqCst);

            let source =
                unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
            let current_rl = unsafe { CFRunLoopGetCurrent() };
            unsafe {
                CFRunLoopAddSource(current_rl, source, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);
            }

            // Signal to start() that the tap is live.
            let _ = tx.send(SetupResult::Ok {
                run_loop: SendableRunLoop(current_rl),
                tap: SendableTap(tap),
            });

            unsafe {
                CFRunLoopRun();

                // Cleanup — CFRunLoopRun returned because CFRunLoopStop was called on teardown.
                CFRelease(source);
                CFRelease(tap);
                // Free the raw Arc last, so any callback in-flight has already returned.
                let _ = Box::from_raw(data_raw);
            }

            drop(data);
        });

        // Block until the worker thread either reports the tap is live, or fails.
        match rx.recv() {
            Ok(SetupResult::Ok { run_loop, tap }) => Some(MacOSClickTapHandle {
                run_loop,
                tap,
                thread: Some(thread),
            }),
            _ => {
                // Worker thread failed or channel dropped — join it so we don't leak.
                let _ = thread.join();
                None
            }
        }
    }
}
