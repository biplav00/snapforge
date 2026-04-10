//! C FFI wrappers over snapforge-core for the Qt frontend.
//!
//! Convention:
//! - Return 0 on success, -1 on error.
//! - Caller-owned buffers are allocated here and freed via snapforge_free_buffer.
//! - String out-params are allocated here and freed via snapforge_free_string.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;

use snapforge_core::capture;
use snapforge_core::clipboard;
use snapforge_core::config::{AppConfig, RecordingFormat, RecordingQuality};
use snapforge_core::format;
use snapforge_core::history::ScreenshotHistory;
use snapforge_core::record::ffmpeg::RecordingHandle;
use snapforge_core::record::RecordConfig;
use snapforge_core::types::{CaptureFormat, Rect};

/// Captured image data returned to C callers.
/// The caller must free `data` via `snapforge_free_buffer`.
#[repr(C)]
pub struct CapturedImage {
    pub data: *mut u8,
    pub len: usize,
    pub width: u32,
    pub height: u32,
}

/// Capture the full screen for a given display index.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
/// Returns: width > 0 on success, width == 0 on error.
#[no_mangle]
pub extern "C" fn snapforge_capture_fullscreen(display: u32) -> CapturedImage {
    let result = capture::capture_fullscreen(display as usize);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let mut raw = image.into_raw();
            let data = raw.as_mut_ptr();
            let len = raw.len();
            std::mem::forget(raw);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
    }
}

/// Capture a region of the screen.
/// Returns a CapturedImage with RGBA pixel data.
/// The caller must free `result.data` via `snapforge_free_buffer(result.data, result.len)`.
#[no_mangle]
pub extern "C" fn snapforge_capture_region(
    display: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> CapturedImage {
    let region = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    let result = capture::capture_region(display as usize, region);
    match result {
        Ok(image) => {
            let width = image.width();
            let height = image.height();
            let mut raw = image.into_raw();
            let data = raw.as_mut_ptr();
            let len = raw.len();
            std::mem::forget(raw);
            CapturedImage {
                data,
                len,
                width,
                height,
            }
        }
        Err(_) => CapturedImage {
            data: ptr::null_mut(),
            len: 0,
            width: 0,
            height: 0,
        },
    }
}

/// Free a pixel buffer returned by snapforge_capture_*.
///
/// # Safety
///
/// `data` must have been returned by a `snapforge_capture_*` function
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_free_buffer(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len));
    }
}

/// Save RGBA pixel data to a file.
/// `path` is a null-terminated UTF-8 string.
/// `fmt`: 0 = PNG, 1 = JPG, 2 = WebP.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// - `data` must point to a valid buffer of at least `width * height * 4` bytes.
/// - `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_save_image(
    data: *const u8,
    width: u32,
    height: u32,
    path: *const c_char,
    fmt: u32,
    quality: u8,
) -> c_int {
    if data.is_null() || path.is_null() {
        return -1;
    }

    let path_str = CStr::from_ptr(path);
    let Ok(path_s) = path_str.to_str() else {
        return -1;
    };
    let path = Path::new(path_s);

    let format = match fmt {
        0 => CaptureFormat::Png,
        1 => CaptureFormat::Jpg,
        2 => CaptureFormat::WebP,
        _ => return -1,
    };

    let len = (width as usize) * (height as usize) * 4;
    let slice = std::slice::from_raw_parts(data, len);

    let Some(image) = image::RgbaImage::from_raw(width, height, slice.to_vec()) else {
        return -1;
    };

    match format::save_image(&image, path, format, quality) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Copy RGBA pixel data to the system clipboard.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `data` must point to a valid buffer of at least `width * height * 4` bytes.
#[no_mangle]
pub unsafe extern "C" fn snapforge_copy_to_clipboard(
    data: *const u8,
    width: u32,
    height: u32,
) -> c_int {
    if data.is_null() {
        return -1;
    }

    let len = (width as usize) * (height as usize) * 4;
    let slice = std::slice::from_raw_parts(data, len);

    let Some(image) = image::RgbaImage::from_raw(width, height, slice.to_vec()) else {
        return -1;
    };

    match clipboard::copy_image_to_clipboard(&image) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Check if screen capture permission is granted.
/// Returns 1 if granted, 0 if not.
#[no_mangle]
pub extern "C" fn snapforge_has_permission() -> c_int {
    c_int::from(capture::has_permission())
}

/// Request screen capture permission.
/// Returns 1 if granted, 0 if not.
#[no_mangle]
pub extern "C" fn snapforge_request_permission() -> c_int {
    c_int::from(capture::request_permission())
}

/// Get the number of available displays.
#[no_mangle]
pub extern "C" fn snapforge_display_count() -> u32 {
    capture::display_count() as u32
}

/// Get the DPI scale factor of the primary display.
#[no_mangle]
pub extern "C" fn snapforge_display_scale_factor() -> f64 {
    capture::display_scale_factor()
}

/// Get the default save directory path (e.g. ~/Pictures/Snapforge/).
/// Returns a heap-allocated null-terminated string. Caller must free via snapforge_free_string.
/// Returns NULL on error.
#[no_mangle]
pub extern "C" fn snapforge_default_save_path() -> *mut c_char {
    let config = snapforge_core::config::AppConfig::load().unwrap_or_default();
    let dir = &config.save_directory;
    match CString::new(dir.to_string_lossy().as_ref()) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string returned by snapforge_* functions.
///
/// # Safety
///
/// `s` must have been returned by a `snapforge_*` function and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

// ---------------------------------------------------------------------------
// Recording FFI
// ---------------------------------------------------------------------------

/// Wrapper around `RecordingHandle` that allows stop-by-pointer (the inner
/// handle's `stop` method consumes `self`, so we keep it in an `Option`).
struct FfiRecordingHandle {
    inner: Option<RecordingHandle>,
}

/// Start a screen recording.
///
/// `config_json` is a null-terminated UTF-8 JSON string with fields:
///   display (u32), region (optional {x,y,width,height}), output_path (string),
///   format ("mp4"/"gif"), fps (u32), quality ("low"/"medium"/"high"),
///   ffmpeg_path (optional string).
///
/// Returns an opaque handle on success, NULL on error.
/// The caller must eventually call `snapforge_stop_recording` and then
/// `snapforge_free_recording_handle`.
///
/// # Safety
///
/// `config_json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_start_recording(config_json: *const c_char) -> *mut c_void {
    if config_json.is_null() {
        return ptr::null_mut();
    }

    let c_str = CStr::from_ptr(config_json);
    let Ok(json_str) = c_str.to_str() else {
        return ptr::null_mut();
    };

    let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return ptr::null_mut();
    };

    let display = v["display"].as_u64().unwrap_or(0) as usize;

    let region = if v["region"].is_object() {
        let r = &v["region"];
        Some(Rect {
            x: r["x"].as_i64().unwrap_or(0) as i32,
            y: r["y"].as_i64().unwrap_or(0) as i32,
            width: r["width"].as_u64().unwrap_or(0) as u32,
            height: r["height"].as_u64().unwrap_or(0) as u32,
        })
    } else {
        None
    };

    let output_path = match v["output_path"].as_str() {
        Some(s) => PathBuf::from(s),
        None => return ptr::null_mut(),
    };

    let format = match v["format"].as_str() {
        Some("gif") => RecordingFormat::Gif,
        _ => RecordingFormat::Mp4,
    };

    let fps = v["fps"].as_u64().unwrap_or(30) as u32;

    let quality = match v["quality"].as_str() {
        Some("low") => RecordingQuality::Low,
        Some("high") => RecordingQuality::High,
        _ => RecordingQuality::Medium,
    };

    let ffmpeg_path = v["ffmpeg_path"].as_str().map(PathBuf::from);

    let config = RecordConfig {
        display,
        region,
        output_path,
        format,
        fps,
        quality,
        ffmpeg_path,
    };

    match snapforge_core::record::ffmpeg::start_recording(config) {
        Ok(handle) => {
            let wrapper = Box::new(FfiRecordingHandle {
                inner: Some(handle),
            });
            Box::into_raw(wrapper).cast::<c_void>()
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Stop a recording and wait for it to finish.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_start_recording`
/// and must not have been freed.
#[no_mangle]
pub unsafe extern "C" fn snapforge_stop_recording(handle: *mut c_void) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let wrapper = &mut *handle.cast::<FfiRecordingHandle>();
    match wrapper.inner.take() {
        Some(h) => match h.stop() {
            Ok(()) => 0,
            Err(_) => -1,
        },
        None => -1, // already stopped
    }
}

/// Check if a recording is still active.
/// Returns 1 if recording, 0 if not.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_start_recording`
/// and must not have been freed.
#[no_mangle]
pub unsafe extern "C" fn snapforge_is_recording(handle: *mut c_void) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let wrapper = &*handle.cast::<FfiRecordingHandle>();
    match &wrapper.inner {
        Some(h) => c_int::from(h.is_running()),
        None => 0,
    }
}

/// Free a recording handle. The recording must have been stopped first
/// (or will be stopped by the drop implementation).
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `snapforge_start_recording`
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn snapforge_free_recording_handle(handle: *mut c_void) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle.cast::<FfiRecordingHandle>());
    }
}

// ---------------------------------------------------------------------------
// History FFI
// ---------------------------------------------------------------------------

/// List all history entries as a JSON string.
/// Returns NULL on error. Caller must free via `snapforge_free_string`.
#[no_mangle]
pub extern "C" fn snapforge_history_list() -> *mut c_char {
    let Ok(history) = ScreenshotHistory::load() else {
        return ptr::null_mut();
    };
    let Ok(json) = serde_json::to_string(&history.entries) else {
        return ptr::null_mut();
    };
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Add a file path to the screenshot history.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_history_add(path: *const c_char) -> c_int {
    if path.is_null() {
        return -1;
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        return -1;
    };
    let Ok(mut history) = ScreenshotHistory::load() else {
        return -1;
    };
    match history.add_entry(path_str) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Delete a history entry by file path.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `path` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_history_delete(path: *const c_char) -> c_int {
    if path.is_null() {
        return -1;
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        return -1;
    };
    let Ok(mut history) = ScreenshotHistory::load() else {
        return -1;
    };
    match history.remove_entry(path_str) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Clear all history entries.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn snapforge_history_clear() -> c_int {
    let Ok(mut history) = ScreenshotHistory::load() else {
        return -1;
    };
    match history.clear() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

// ---------------------------------------------------------------------------
// Config FFI
// ---------------------------------------------------------------------------

/// Load the application config and return it as a JSON string.
/// Returns NULL on error. Caller must free via `snapforge_free_string`.
#[no_mangle]
pub extern "C" fn snapforge_config_load() -> *mut c_char {
    let Ok(config) = AppConfig::load() else {
        return ptr::null_mut();
    };
    let Ok(json) = serde_json::to_string(&config) else {
        return ptr::null_mut();
    };
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Save application config from a JSON string.
/// Returns 0 on success, -1 on error.
///
/// # Safety
///
/// `json` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn snapforge_config_save(json: *const c_char) -> c_int {
    if json.is_null() {
        return -1;
    }
    let Ok(json_str) = CStr::from_ptr(json).to_str() else {
        return -1;
    };
    let Ok(config) = serde_json::from_str::<AppConfig>(json_str) else {
        return -1;
    };
    match config.save() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
