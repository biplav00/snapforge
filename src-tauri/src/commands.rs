use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Return the pre-captured screenshot (taken before the overlay window appeared).
#[tauri::command]
pub fn get_pre_captured_screen(
    state: tauri::State<'_, crate::PreCapturedScreen>,
) -> Result<String, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    guard
        .clone()
        .ok_or_else(|| "No pre-captured screen available".to_string())
}

/// Capture fullscreen and return base64-encoded PNG for the overlay background.
#[tauri::command]
pub fn capture_screen(display: usize) -> Result<String, String> {
    let image = screen_core::capture::capture_fullscreen(display)
        .map_err(|e| e.to_string())?;

    let bytes = screen_core::format::encode_image(
        &image,
        screen_core::types::CaptureFormat::Png,
        90,
    )
    .map_err(|e| e.to_string())?;

    Ok(STANDARD.encode(&bytes))
}

/// Save a region of the captured screen to disk.
#[tauri::command]
pub fn save_region(
    display: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let region = screen_core::types::Rect { x, y, width, height };
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = screen_core::screenshot_region(
        display,
        region,
        &save_path,
        format,
        quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

/// Save a fullscreen capture to disk (no region selection).
#[tauri::command]
pub fn save_fullscreen(display: usize) -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = screen_core::screenshot_fullscreen(
        display,
        &save_path,
        format,
        quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

/// Save a composited image (screenshot + annotations) from base64 PNG.
#[tauri::command]
pub fn save_composited_image(image_base64: String) -> Result<String, String> {
    let bytes = STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 decode failed: {}", e))?;

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("image decode failed: {}", e))?;

    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let rgba = img.to_rgba8();
    screen_core::format::save_image(
        &rgba,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
    )
    .map_err(|e| e.to_string())?;

    Ok(save_path.display().to_string())
}

/// Copy a composited image (screenshot + annotations) to clipboard from base64 PNG.
#[tauri::command]
pub fn copy_composited_image(image_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 decode failed: {}", e))?;

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("image decode failed: {}", e))?;

    let rgba = img.to_rgba8();
    screen_core::clipboard::copy_image_to_clipboard(&rgba)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get the current app config as JSON.
#[tauri::command]
pub fn get_config() -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&config).map_err(|e| e.to_string())
}

/// Save the app config from JSON.
#[tauri::command]
pub fn save_config(config_json: String) -> Result<(), String> {
    let config: screen_core::config::AppConfig =
        serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.save().map_err(|e| e.to_string())
}

/// Open the save directory in the system file manager.
#[tauri::command]
pub fn open_save_folder() -> Result<(), String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;
    let dir = &config.save_directory;
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    open::that(dir).map_err(|e| e.to_string())
}

/// Reload global hotkeys from config (call after saving preferences).
#[tauri::command]
pub fn reload_hotkeys(app: tauri::AppHandle) -> Result<(), String> {
    crate::hotkeys::reload_hotkeys(&app).map_err(|e| e.to_string())
}

/// Capture a region and copy it to clipboard (no file save).
#[tauri::command]
pub fn capture_and_copy_region(
    display: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let region = screen_core::types::Rect { x, y, width, height };
    let image = screen_core::capture::capture_region(display, region)
        .map_err(|e| e.to_string())?;
    screen_core::clipboard::copy_image_to_clipboard(&image)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if FFmpeg is installed.
#[tauri::command]
pub fn check_ffmpeg() -> Result<(), String> {
    screen_core::record::check_ffmpeg().map_err(|e| e.to_string())
}

/// Start recording. Returns the output file path.
#[tauri::command]
pub fn start_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
    display: usize,
    region_x: Option<i32>,
    region_y: Option<i32>,
    region_w: Option<u32>,
    region_h: Option<u32>,
) -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let region = match (region_x, region_y, region_w, region_h) {
        (Some(x), Some(y), Some(w), Some(h)) => {
            Some(screen_core::types::Rect { x, y, width: w, height: h })
        }
        _ => None,
    };

    let output_path = config.recording_file_path();
    let record_config = screen_core::record::RecordConfig {
        display,
        region,
        output_path: output_path.clone(),
        format: config.recording.format,
        fps: config.recording.fps,
        quality: config.recording.quality,
        ffmpeg_path: None, // will search bundled sidecar then system PATH
    };

    let handle = screen_core::record::ffmpeg::start_recording(record_config)
        .map_err(|e| e.to_string())?;

    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    *guard = Some(handle);

    Ok(output_path.display().to_string())
}

/// Stop recording.
#[tauri::command]
pub fn stop_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
) -> Result<(), String> {
    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = guard.take() {
        handle.stop().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Check if currently recording.
#[tauri::command]
pub fn is_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
) -> bool {
    state.is_recording()
}

/// Start recording and open the indicator window.
#[tauri::command]
pub fn start_recording_and_show_indicator(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::recording::RecordingState>,
    display: usize,
    region_x: Option<i32>,
    region_y: Option<i32>,
    region_w: Option<u32>,
    region_h: Option<u32>,
) -> Result<String, String> {
    let path = start_recording(state, display, region_x, region_y, region_w, region_h)?;
    crate::open_recording_indicator(&app);
    Ok(path)
}

/// Save a screenshot of the last remembered region.
pub fn save_last_region() -> Result<String, String> {
    let config = screen_core::config::AppConfig::load()
        .map_err(|e| e.to_string())?;

    let last = config.last_region
        .ok_or_else(|| "No last region saved".to_string())?;

    let save_path = config.save_file_path();
    let path = screen_core::screenshot_region(
        last.display,
        last.rect,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
        config.auto_copy_clipboard,
    )
    .map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}
