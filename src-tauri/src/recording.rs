// src-tauri/src/recording.rs
use screen_core::record::ffmpeg::RecordingHandle;
use std::sync::Mutex;

pub struct RecordingState {
    pub handle: Mutex<Option<RecordingHandle>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.handle
            .lock()
            .map(|h| h.as_ref().map_or(false, |handle| handle.is_running()))
            .unwrap_or(false)
    }
}
