// src-tauri/src/recording.rs
use snapforge_core::record::ffmpeg::RecordingHandle;
use std::sync::Mutex;

pub struct RecordingState {
    pub handle: Mutex<Option<RecordingHandle>>,
    pub output_path: Mutex<Option<String>>,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            handle: Mutex::new(None),
            output_path: Mutex::new(None),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.handle
            .lock()
            .map(|h| {
                h.as_ref()
                    .is_some_and(snapforge_core::record::ffmpeg::RecordingHandle::is_running)
            })
            .unwrap_or(false)
    }
}
