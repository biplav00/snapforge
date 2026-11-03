//! Continuous display capture via `SCStream` (recording hot path).
//!
//! ScreenCaptureKit pushes `CMSampleBuffer`s to [`StreamOutput`] on a private
//! serial dispatch queue. Each *complete* frame's `CVPixelBuffer` is copied
//! (honouring its row stride) into a [`FrameSlot`] that the encode worker
//! polls at its own constant frame rate. Idle frames (screen unchanged) carry
//! no image and are skipped, so the worker simply repeats the last frame.
//!
//! The stream is configured for 32BGRA in the sRGB colour space, so the bytes
//! are exactly what the ffmpeg pipe expects (`-pix_fmt bgra`, tagged BT.709,
//! whose primaries match sRGB).

use std::ffi::c_void;
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    kCVPixelFormatType_32BGRA, CVPixelBuffer, CVPixelBufferGetBaseAddress,
    CVPixelBufferGetBytesPerRow, CVPixelBufferGetDataSize, CVPixelBufferGetHeight,
    CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth, CVPixelBufferIsPlanar,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSError, NSObjectProtocol};
use objc2_screen_capture_kit::{
    SCFrameStatus, SCStream, SCStreamConfiguration, SCStreamDelegate, SCStreamFrameInfoStatus,
    SCStreamOutput, SCStreamOutputType,
};

use super::{
    display_filter, display_id_for_index, ensure_sck_screenshot_available, sc_display_for_id,
    srgb_color_space_name, CGDisplayIsOnline, RawFrame, SCK_CONTENT_TIMEOUT,
};
use crate::capture::CaptureError;

/// Bounded wait for `startCaptureWithCompletionHandler`.
const STREAM_START_TIMEOUT: Duration = Duration::from_secs(5);
/// Bounded wait for `stopCaptureWithCompletionHandler` (runs on every exit
/// path, including `Drop`, so it must never hang the worker).
const STREAM_STOP_TIMEOUT: Duration = Duration::from_secs(2);
/// Frames SCK may have in flight. Small: we only ever want the newest one.
const STREAM_QUEUE_DEPTH: isize = 4;

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Copy `height` rows of `width` BGRA pixels out of a buffer whose rows are
/// `bytes_per_row` apart (CVPixelBuffers pad rows for alignment) into `dst`,
/// tightly packed. `dst` is reused: cleared, then refilled.
pub(crate) fn copy_bgra_rows(
    src: &[u8],
    bytes_per_row: usize,
    width: usize,
    height: usize,
    dst: &mut Vec<u8>,
) -> Result<(), CaptureError> {
    let row = width.checked_mul(4).ok_or(CaptureError::ImageDataFailed)?;
    if width == 0 || height == 0 || bytes_per_row < row {
        return Err(CaptureError::ImageDataFailed);
    }
    let needed = (height - 1)
        .checked_mul(bytes_per_row)
        .and_then(|v| v.checked_add(row))
        .ok_or(CaptureError::ImageDataFailed)?;
    if src.len() < needed {
        return Err(CaptureError::ImageDataFailed);
    }
    dst.clear();
    if bytes_per_row == row {
        dst.extend_from_slice(&src[..row * height]);
    } else {
        dst.reserve(row * height);
        for y in 0..height {
            let start = y * bytes_per_row;
            dst.extend_from_slice(&src[start..start + row]);
        }
    }
    Ok(())
}

/// Only `Complete` frames carry new pixels. `Idle` means "nothing changed",
/// and `Blank`/`Suspended`/`Started`/`Stopped` carry no usable image.
pub(crate) fn is_complete_frame(status: Option<isize>) -> bool {
    status == Some(SCFrameStatus::Complete.0)
}

/// Newest-frame mailbox shared between the SCK callback (producer) and the
/// encode worker (consumer). Holds at most one frame plus one recycled
/// buffer, so steady-state recording allocates nothing.
#[derive(Default)]
pub(crate) struct FrameSlot {
    latest: Option<RawFrame>,
    /// Buffer of the previously published frame, handed back to the
    /// producer to fill next.
    spare: Vec<u8>,
    /// Number of frames published so far (0 = none yet).
    seq: u64,
}

impl FrameSlot {
    /// Take the recycled buffer to fill outside the lock.
    pub(crate) fn take_spare(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.spare)
    }

    /// Give back a buffer that was taken but not published (bad frame).
    pub(crate) fn return_spare(&mut self, buf: Vec<u8>) {
        if buf.capacity() > self.spare.capacity() {
            self.spare = buf;
        }
    }

    /// Replace the latest frame; the displaced frame's buffer becomes the spare.
    pub(crate) fn publish(&mut self, frame: RawFrame) {
        if let Some(old) = self.latest.replace(frame) {
            self.spare = old.bytes;
        }
        self.seq += 1;
    }

    /// The latest frame and its sequence number, if newer than `seen`.
    pub(crate) fn newer_than(&self, seen: u64) -> Option<(&RawFrame, u64)> {
        if self.seq > seen {
            self.latest.as_ref().map(|f| (f, self.seq))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Shared state + SCK delegate
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    slot: Mutex<FrameSlot>,
    /// Signalled whenever a frame is published or the stream fails.
    changed: Condvar,
    /// Set by `stream:didStopWithError:` — the stream is dead.
    failure: Mutex<Option<String>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn fail(&self, msg: String) {
        *lock(&self.failure) = Some(msg);
        self.changed.notify_all();
    }

    fn failure(&self) -> Option<String> {
        lock(&self.failure).clone()
    }

    /// Copy one sample buffer into the slot. Called on the SCK queue; must
    /// never panic (it runs inside an Objective-C callback).
    fn ingest(&self, sample: &CMSampleBuffer) {
        if !is_complete_frame(frame_status(sample)) {
            return;
        }
        // SAFETY: `sample` is a valid sample buffer for the callback's duration.
        let Some(image) = (unsafe { sample.image_buffer() }) else {
            return;
        };
        let mut buf = lock(&self.slot).take_spare();
        match copy_pixel_buffer(&image, &mut buf) {
            Ok((width, height)) => {
                lock(&self.slot).publish(RawFrame {
                    bytes: buf,
                    width,
                    height,
                });
                self.changed.notify_all();
            }
            Err(()) => lock(&self.slot).return_spare(buf),
        }
    }
}

/// `SCStreamFrameInfoStatus` from the sample's first attachment dictionary.
fn frame_status(sample: &CMSampleBuffer) -> Option<isize> {
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: isize, out: *mut c_void) -> bool;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFNumberGetTypeID() -> usize;
    }
    const K_CF_NUMBER_SINT64_TYPE: isize = 4;
    // SAFETY: the attachments array (Get rule, retained by the binding) and
    // its dictionaries live as long as `sample`; every CF object is
    // type-checked before use. NSString keys are toll-free bridged CFStrings.
    unsafe {
        let attachments = sample.sample_attachments_array(false)?;
        let array: *const c_void = (&raw const *attachments).cast();
        if CFArrayGetCount(array) < 1 {
            return None;
        }
        let dict = CFArrayGetValueAtIndex(array, 0);
        if dict.is_null() || CFGetTypeID(dict) != CFDictionaryGetTypeID() {
            return None;
        }
        let key: *const c_void = (&raw const *SCStreamFrameInfoStatus).cast();
        let number = CFDictionaryGetValue(dict, key);
        if number.is_null() || CFGetTypeID(number) != CFNumberGetTypeID() {
            return None;
        }
        let mut value: i64 = 0;
        CFNumberGetValue(
            number,
            K_CF_NUMBER_SINT64_TYPE,
            (&raw mut value).cast::<c_void>(),
        )
        .then(|| isize::try_from(value).ok())
        .flatten()
    }
}

/// Lock a BGRA `CVPixelBuffer`, copy it tightly packed into `dst`, unlock.
fn copy_pixel_buffer(pb: &CVPixelBuffer, dst: &mut Vec<u8>) -> Result<(u32, u32), ()> {
    if CVPixelBufferGetPixelFormatType(pb) != kCVPixelFormatType_32BGRA || CVPixelBufferIsPlanar(pb)
    {
        return Err(());
    }
    // SAFETY: lock/unlock are paired with the same (read-only) flags; the
    // base address is only dereferenced between them, bounded by the
    // buffer's reported data size.
    unsafe {
        if CVPixelBufferLockBaseAddress(pb, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return Err(());
        }
        let width = CVPixelBufferGetWidth(pb);
        let height = CVPixelBufferGetHeight(pb);
        let bytes_per_row = CVPixelBufferGetBytesPerRow(pb);
        let base = CVPixelBufferGetBaseAddress(pb).cast::<u8>().cast_const();
        let size = CVPixelBufferGetDataSize(pb);
        let result = if base.is_null() {
            Err(())
        } else {
            let src = std::slice::from_raw_parts(base, size);
            copy_bgra_rows(src, bytes_per_row, width, height, dst).map_err(|_| ())
        };
        CVPixelBufferUnlockBaseAddress(pb, CVPixelBufferLockFlags::ReadOnly);
        result?;
        Ok((
            u32::try_from(width).map_err(|_| ())?,
            u32::try_from(height).map_err(|_| ())?,
        ))
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and this class has
    // no Drop impl; the ivar is a plain `Arc` (Send + Sync).
    #[unsafe(super(NSObject))]
    #[name = "SnapforgeStreamOutput"]
    #[ivars = Arc<Shared>]
    struct StreamOutput;

    unsafe impl NSObjectProtocol for StreamOutput {}

    unsafe impl SCStreamOutput for StreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output(
            &self,
            _stream: &SCStream,
            sample: &CMSampleBuffer,
            kind: SCStreamOutputType,
        ) {
            if kind == SCStreamOutputType::Screen {
                self.ivars().ingest(sample);
            }
        }
    }

    unsafe impl SCStreamDelegate for StreamOutput {
        #[unsafe(method(stream:didStopWithError:))]
        fn stream_did_stop(&self, _stream: &SCStream, error: &NSError) {
            let msg = error.localizedDescription().to_string();
            tracing::error!("[capture] SCStream stopped with error: {msg}");
            self.ivars().fail(msg);
        }
    }
);

impl StreamOutput {
    fn new(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(shared);
        // SAFETY: NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }
}

/// Run an SCK call that reports completion through an `NSError*` block and
/// wait (bounded) for it. `Err` carries a description for logging.
fn wait_completion(
    timeout: Duration,
    call: impl FnOnce(&block2::DynBlock<dyn Fn(*mut NSError)>),
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<Option<String>>();
    let block = block2::RcBlock::new(move |err: *mut NSError| {
        // SAFETY: SCK passes either null or a valid NSError.
        let msg = unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string());
        let _ = tx.send(msg);
    });
    call(&block);
    match rx.recv_timeout(timeout) {
        Ok(None) => Ok(()),
        Ok(Some(msg)) => Err(msg),
        Err(_) => Err(format!("no completion within {timeout:?}")),
    }
}

fn stream_config(
    out_w: u32,
    out_h: u32,
    fps: u32,
    shows_cursor: bool,
) -> Retained<SCStreamConfiguration> {
    // SAFETY: plain property setters on a fresh configuration object.
    unsafe {
        let config = SCStreamConfiguration::new();
        config.setWidth(out_w as usize);
        config.setHeight(out_h as usize);
        config.setPixelFormat(kCVPixelFormatType_32BGRA);
        config.setColorSpaceName(srgb_color_space_name());
        config.setMinimumFrameInterval(CMTime::new(1, i32::try_from(fps.max(1)).unwrap_or(60)));
        config.setQueueDepth(STREAM_QUEUE_DEPTH);
        config.setShowsCursor(shows_cursor);
        config
    }
}

// ---------------------------------------------------------------------------
// StreamCapture
// ---------------------------------------------------------------------------

/// A running `SCStream` on one display. Not `Send`: create, poll and drop it
/// on the same (worker) thread. Dropping it stops the stream.
pub struct StreamCapture {
    stream: Retained<SCStream>,
    shared: Arc<Shared>,
    /// Kept alive for the stream's lifetime (SCK may hold it weakly).
    _output: Retained<StreamOutput>,
    _queue: DispatchRetained<DispatchQueue>,
    display: usize,
    display_id: u32,
    /// Sequence number of the last frame handed to the caller.
    last_seen: u64,
    /// `startCapture` was issued, so `stopCapture` is owed.
    running: bool,
    failure_logged: bool,
    pub output_width: u32,
    pub output_height: u32,
}

impl StreamCapture {
    /// Start streaming display `display` (CoreGraphics index) scaled to
    /// `out_w`×`out_h`, at most `fps` frames per second, in sRGB BGRA.
    pub fn start(
        display: usize,
        out_w: u32,
        out_h: u32,
        fps: u32,
        shows_cursor: bool,
    ) -> Result<Self, CaptureError> {
        ensure_sck_screenshot_available()?;
        if out_w == 0 || out_h == 0 {
            return Err(CaptureError::CaptureFailed);
        }
        let display_id = display_id_for_index(display)?;
        let sc_display = sc_display_for_id(display, display_id, SCK_CONTENT_TIMEOUT)?;
        let filter = display_filter(&sc_display);
        let config = stream_config(out_w, out_h, fps, shows_cursor);

        let shared = Arc::new(Shared::default());
        let output = StreamOutput::new(Arc::clone(&shared));
        let queue = DispatchQueue::new("com.snapforge.capture.stream", None);
        // SAFETY: filter/config/delegate are valid objects; the delegate and
        // output object outlive the stream (owned by the returned value).
        let stream = unsafe {
            SCStream::initWithFilter_configuration_delegate(
                SCStream::alloc(),
                &filter,
                &config,
                Some(ProtocolObject::from_ref(&*output)),
            )
        };
        // SAFETY: `queue` is a serial queue we own for the stream's lifetime.
        unsafe {
            stream.addStreamOutput_type_sampleHandlerQueue_error(
                ProtocolObject::from_ref(&*output),
                SCStreamOutputType::Screen,
                Some(&queue),
            )
        }
        .map_err(|e| {
            tracing::warn!(
                "[capture] SCStream addStreamOutput failed: {}",
                e.localizedDescription()
            );
            CaptureError::CaptureFailed
        })?;

        let mut this = Self {
            stream,
            shared,
            _output: output,
            _queue: queue,
            display,
            display_id,
            last_seen: 0,
            // Set before starting: even a timed-out start may complete later,
            // so Drop must always issue a stop.
            running: true,
            failure_logged: false,
            output_width: out_w,
            output_height: out_h,
        };
        // SAFETY: `this.stream` is a configured, not-yet-started stream.
        let started = wait_completion(STREAM_START_TIMEOUT, |block| unsafe {
            this.stream.startCaptureWithCompletionHandler(Some(block));
        });
        if let Err(msg) = started {
            tracing::warn!("[capture] SCStream failed to start: {msg}");
            this.stop_inner();
            return Err(CaptureError::CaptureFailed);
        }
        Ok(this)
    }

    /// Why the stream died, if it did (`stream:didStopWithError:`).
    pub fn failure(&self) -> Option<String> {
        self.shared.failure()
    }

    fn check_alive(&mut self) -> Result<(), CaptureError> {
        // CGDisplayIsOnline (not IsActive) so a merely sleeping display
        // doesn't abort the recording.
        if unsafe { CGDisplayIsOnline(self.display_id) } == 0 {
            return Err(CaptureError::NoDisplay(self.display));
        }
        if let Some(msg) = self.failure() {
            if !self.failure_logged {
                self.failure_logged = true;
                tracing::error!("[capture] recording stream is dead: {msg}");
            }
            return Err(CaptureError::CaptureFailed);
        }
        Ok(())
    }

    /// Hand the newest frame to `f` if one arrived since the last call.
    /// `Ok(false)` means the screen hasn't changed (repeat the last frame);
    /// `Err` means the display is gone (`NoDisplay`) or the stream failed.
    pub fn latest_frame(&mut self, f: impl FnOnce(&RawFrame)) -> Result<bool, CaptureError> {
        self.check_alive()?;
        let slot = lock(&self.shared.slot);
        match slot.newer_than(self.last_seen) {
            Some((frame, seq)) => {
                f(frame);
                self.last_seen = seq;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Like [`Self::latest_frame`], but waits up to `timeout` for a frame
    /// (used for the recording's first frame).
    pub fn wait_frame(
        &mut self,
        timeout: Duration,
        f: impl FnOnce(&RawFrame),
    ) -> Result<bool, CaptureError> {
        let deadline = Instant::now() + timeout;
        {
            let mut slot = lock(&self.shared.slot);
            while slot.newer_than(self.last_seen).is_none() && self.shared.failure().is_none() {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                slot = self
                    .shared
                    .changed
                    .wait_timeout(slot, deadline - now)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
        }
        self.latest_frame(f)
    }

    /// Stop the stream (bounded wait). Also done automatically on drop.
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        if !std::mem::take(&mut self.running) {
            return;
        }
        // SAFETY: stopping a started (or failed-to-start) stream is allowed;
        // the latter just reports an error through the completion handler.
        let stopped = wait_completion(STREAM_STOP_TIMEOUT, |block| unsafe {
            self.stream.stopCaptureWithCompletionHandler(Some(block));
        });
        if let Err(msg) = stopped {
            tracing::debug!("[capture] SCStream stop: {msg}");
        }
    }
}

impl Drop for StreamCapture {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(byte: u8) -> RawFrame {
        RawFrame {
            bytes: vec![byte; 8],
            width: 2,
            height: 1,
        }
    }

    #[test]
    fn copy_rows_tight_buffer_is_verbatim() {
        let src: Vec<u8> = (0..24).collect(); // 3x2 BGRA
        let mut dst = vec![9; 3];
        copy_bgra_rows(&src, 12, 3, 2, &mut dst).unwrap();
        assert_eq!(dst, src);
    }

    #[test]
    fn copy_rows_skips_stride_padding() {
        // 2x3 pixels, rows padded to 16 bytes (8 data + 8 padding).
        let mut src = Vec::new();
        for y in 0..3u8 {
            src.extend_from_slice(&[y; 8]);
            src.extend_from_slice(&[0xEE; 8]);
        }
        let mut dst = Vec::new();
        copy_bgra_rows(&src, 16, 2, 3, &mut dst).unwrap();
        assert_eq!(dst.len(), 2 * 3 * 4);
        for (y, row) in dst.chunks(8).enumerate() {
            assert!(row.iter().all(|&b| b == y as u8), "row {y}: {row:?}");
        }
    }

    #[test]
    fn copy_rows_last_row_may_omit_padding() {
        // CVPixelBuffer data size may end right after the last row's pixels.
        let src = vec![1u8; 16 + 8];
        let mut dst = Vec::new();
        copy_bgra_rows(&src, 16, 2, 2, &mut dst).unwrap();
        assert_eq!(dst.len(), 16);
    }

    #[test]
    fn copy_rows_rejects_bad_geometry() {
        let mut dst = Vec::new();
        // Stride smaller than a row.
        assert!(copy_bgra_rows(&[0; 64], 4, 2, 2, &mut dst).is_err());
        // Buffer too short.
        assert!(copy_bgra_rows(&[0; 15], 8, 2, 2, &mut dst).is_err());
        // Empty.
        assert!(copy_bgra_rows(&[0; 64], 8, 0, 2, &mut dst).is_err());
        assert!(copy_bgra_rows(&[0; 64], 8, 2, 0, &mut dst).is_err());
        // Overflow.
        assert!(copy_bgra_rows(&[0; 64], usize::MAX, 2, 3, &mut dst).is_err());
    }

    #[test]
    fn only_complete_frames_are_used() {
        assert!(is_complete_frame(Some(SCFrameStatus::Complete.0)));
        for s in [
            SCFrameStatus::Idle,
            SCFrameStatus::Blank,
            SCFrameStatus::Suspended,
            SCFrameStatus::Started,
            SCFrameStatus::Stopped,
        ] {
            assert!(!is_complete_frame(Some(s.0)));
        }
        assert!(!is_complete_frame(None));
    }

    #[test]
    fn frame_slot_reports_each_frame_once() {
        let mut slot = FrameSlot::default();
        assert!(slot.newer_than(0).is_none(), "nothing published yet");
        slot.publish(frame(1));
        let (f, seq) = slot.newer_than(0).unwrap();
        assert_eq!((f.bytes[0], seq), (1, 1));
        assert!(slot.newer_than(seq).is_none(), "no new frame: repeat last");
        slot.publish(frame(2));
        slot.publish(frame(3));
        // Consumer that fell behind only sees the newest frame.
        let (f, seq) = slot.newer_than(1).unwrap();
        assert_eq!((f.bytes[0], seq), (3, 3));
    }

    #[test]
    fn frame_slot_recycles_buffers() {
        let mut slot = FrameSlot::default();
        slot.publish(frame(1));
        assert!(
            slot.take_spare().is_empty(),
            "first publish displaces nothing"
        );
        let ptr = slot.latest.as_ref().unwrap().bytes.as_ptr();
        slot.publish(frame(2));
        let spare = slot.take_spare();
        assert_eq!(spare.as_ptr(), ptr, "displaced frame's buffer is reused");
        assert!(slot.take_spare().is_empty(), "spare is handed out once");
        slot.return_spare(spare);
        let back = slot.take_spare();
        assert_eq!(back.as_ptr(), ptr);
    }

    /// Runs the real ObjC setters/getters (objc2 checks selector type
    /// encodings in debug builds), so a wrong binding fails here even
    /// without Screen Recording permission.
    #[test]
    fn stream_config_is_srgb_bgra_at_requested_size_and_rate() {
        let c = stream_config(1280, 720, 24, true);
        unsafe {
            assert_eq!((c.width(), c.height()), (1280, 720));
            assert_eq!(c.pixelFormat(), kCVPixelFormatType_32BGRA);
            assert_eq!(c.colorSpaceName().to_string(), "kCGColorSpaceSRGB");
            assert_eq!(c.queueDepth(), STREAM_QUEUE_DEPTH);
            assert!(c.showsCursor());
            let t = c.minimumFrameInterval();
            assert_eq!((t.value, t.timescale), (1, 24));
        }
    }

    #[test]
    fn stream_output_class_registers_and_conforms() {
        use objc2::runtime::ProtocolObject;
        let shared = Arc::new(Shared::default());
        let out = StreamOutput::new(Arc::clone(&shared));
        // Both protocol casts must type-check and the object must be live.
        let _o: &ProtocolObject<dyn SCStreamOutput> = ProtocolObject::from_ref(&*out);
        let _d: &ProtocolObject<dyn SCStreamDelegate> = ProtocolObject::from_ref(&*out);
        assert!(out.respondsToSelector(objc2::sel!(stream:didOutputSampleBuffer:ofType:)));
        assert!(out.respondsToSelector(objc2::sel!(stream:didStopWithError:)));
        assert_eq!(Arc::strong_count(&shared), 2, "ivar holds the shared state");
        drop(out);
        assert_eq!(
            Arc::strong_count(&shared),
            1,
            "ivar released with the object"
        );
    }

    /// Deliver `stream:didStopWithError:` through the ObjC runtime, as SCK
    /// would, and check the stream is then reported dead.
    #[test]
    fn did_stop_with_error_marks_stream_failed() {
        use objc2_foundation::{ns_string, NSError};
        let shared = Arc::new(Shared::default());
        let out = StreamOutput::new(Arc::clone(&shared));
        let err =
            unsafe { NSError::errorWithDomain_code_userInfo(ns_string!("test"), -3815, None) };
        // Any object stands in for the SCStream (the method ignores it).
        let fake_stream = NSObject::new();
        let _: () = unsafe { msg_send![&*out, stream: &*fake_stream, didStopWithError: &*err] };
        assert!(shared.failure().is_some(), "failure recorded");
    }

    #[test]
    fn stream_start_invalid_display_errors() {
        assert!(StreamCapture::start(99, 640, 360, 30, true).is_err());
    }

    /// Real SCStream smoke test. Skips (passes) without Screen Recording
    /// permission, since SCK then can't start a stream.
    #[test]
    fn stream_delivers_srgb_bgra_frames_when_permitted() {
        if !super::super::has_screen_capture_permission() {
            eprintln!("skipping: no Screen Recording permission");
            return;
        }
        let Ok(mut s) = StreamCapture::start(0, 640, 360, 30, false) else {
            eprintln!("skipping: SCStream could not start");
            return;
        };
        let mut got = None;
        let fresh = s
            .wait_frame(Duration::from_secs(3), |f| {
                got = Some((f.width, f.height, f.bytes.len()));
            })
            .unwrap();
        if fresh {
            let (w, h, len) = got.unwrap();
            assert!(w > 0 && h > 0);
            assert_eq!(len, w as usize * h as usize * 4);
        }
        s.stop();
    }
}
