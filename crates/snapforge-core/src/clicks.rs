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
pub use macos_tap::MacOSClickTapHandle;

#[cfg(target_os = "macos")]
mod macos_tap {
    use super::ClickTracker;
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
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
    }

    /// Wrapper to make CFRunLoopRef Send/Sync — we only use it to signal stop.
    #[derive(Copy, Clone)]
    struct SendableRunLoop(CFRunLoopRef);
    unsafe impl Send for SendableRunLoop {}
    unsafe impl Sync for SendableRunLoop {}

    pub struct MacOSClickTapHandle {
        stop_flag: Arc<AtomicBool>,
        run_loop: Arc<Mutex<Option<SendableRunLoop>>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    use std::sync::Mutex;

    impl Drop for MacOSClickTapHandle {
        fn drop(&mut self) {
            self.stop_flag.store(true, Ordering::SeqCst);
            if let Ok(guard) = self.run_loop.lock() {
                if let Some(rl) = *guard {
                    unsafe { CFRunLoopStop(rl.0) };
                }
            }
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    struct CallbackData {
        tracker: ClickTracker,
    }

    unsafe extern "C" fn event_callback(
        _proxy: CGEventTapProxy,
        _event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef {
        if !user_info.is_null() {
            let data = &*(user_info as *const CallbackData);
            let loc = CGEventGetLocation(event);
            data.tracker.add(loc.x, loc.y);
        }
        event
    }

    pub fn start(tracker: ClickTracker) -> Option<MacOSClickTapHandle> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let run_loop: Arc<Mutex<Option<SendableRunLoop>>> = Arc::new(Mutex::new(None));
        let run_loop_clone = run_loop.clone();

        let thread = std::thread::spawn(move || {
            let mask: CGEventMask =
                (1u64 << KCG_EVENT_LEFT_MOUSE_DOWN) | (1u64 << KCG_EVENT_RIGHT_MOUSE_DOWN);

            let data = Box::new(CallbackData { tracker });
            let data_ptr = Box::into_raw(data).cast::<c_void>();

            let tap = unsafe {
                CGEventTapCreate(
                    KCG_HID_EVENT_TAP,
                    KCG_TAIL_APPEND_EVENT_TAP,
                    KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
                    mask,
                    event_callback,
                    data_ptr,
                )
            };

            if tap.is_null() {
                eprintln!("[clicks] CGEventTapCreate failed — accessibility permission required");
                unsafe {
                    let _ = Box::from_raw(data_ptr.cast::<CallbackData>());
                }
                return;
            }

            unsafe {
                let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
                let current_rl = CFRunLoopGetCurrent();
                CFRunLoopAddSource(current_rl, source, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);

                if let Ok(mut guard) = run_loop_clone.lock() {
                    *guard = Some(SendableRunLoop(current_rl));
                }

                CFRunLoopRun();

                // Cleanup
                CFRelease(source);
                CFRelease(tap);
                let _ = Box::from_raw(data_ptr.cast::<CallbackData>());
            }
        });

        // Give the thread a moment to set up
        std::thread::sleep(std::time::Duration::from_millis(50));

        Some(MacOSClickTapHandle {
            stop_flag,
            run_loop,
            thread: Some(thread),
        })
    }
}
