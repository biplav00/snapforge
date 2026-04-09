use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Show a temporary toast window with a message.
#[tauri::command]
pub fn show_toast(app: tauri::AppHandle, message: String) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    // Close existing toast if any
    if let Some(w) = app.get_webview_window("toast") {
        let _ = w.close();
    }

    let encoded = urlencoding::encode(&message);
    let url = format!("toast.html?msg={encoded}");

    // Position at bottom-right of primary monitor
    let (pos_x, pos_y) = {
        let monitor = app.primary_monitor().ok().flatten();
        if let Some(m) = monitor {
            let size = m.size();
            let scale = m.scale_factor();
            let w = size.width as f64 / scale;
            let h = size.height as f64 / scale;
            (w - 320.0, h - 70.0)
        } else {
            (1600.0, 1000.0)
        }
    };

    let _ = WebviewWindowBuilder::new(&app, "toast", WebviewUrl::App(url.into()))
        .title("")
        .inner_size(300.0, 50.0)
        .position(pos_x, pos_y)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .transparent(true)
        .skip_taskbar(true)
        .focused(false)
        .build();

    // Auto-close after 3 seconds
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if let Some(w) = app_clone.get_webview_window("toast") {
            let _ = w.close();
        }
    });

    Ok(())
}

/// Get the number of available displays.
#[tauri::command]
pub fn get_display_count() -> usize {
    snapforge_core::capture::display_count()
}

/// Return the pre-captured screenshot for a specific display.
#[tauri::command]
pub fn get_pre_captured_screen(
    state: tauri::State<'_, crate::PreCapturedScreens>,
    display: usize,
) -> Result<String, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    guard
        .get(&display)
        .cloned()
        .ok_or_else(|| format!("No pre-captured screen for display {display}"))
}

/// Capture fullscreen and return base64-encoded PNG for the overlay background.
#[tauri::command]
pub fn capture_screen(display: usize) -> Result<String, String> {
    let image = snapforge_core::capture::capture_fullscreen(display).map_err(|e| e.to_string())?;

    let bytes = snapforge_core::format::encode_image_fast(&image).map_err(|e| e.to_string())?;

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
    let region = snapforge_core::types::Rect {
        x,
        y,
        width,
        height,
    };
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = snapforge_core::screenshot_region(
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
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    let format = config.screenshot_format;
    let quality = config.jpg_quality;

    let path = snapforge_core::screenshot_fullscreen(
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

    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;

    let save_path = config.save_file_path();
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let rgba = img.to_rgba8();
    snapforge_core::format::save_image(
        &rgba,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
    )
    .map_err(|e| e.to_string())?;

    // Record in history
    let _ = add_to_history(save_path.display().to_string());

    Ok(save_path.display().to_string())
}

/// Copy a composited image (screenshot + annotations) to clipboard from base64 PNG.
/// Also saves a copy to disk and records in history.
#[tauri::command]
pub fn copy_composited_image(image_base64: String) -> Result<(), String> {
    let bytes = STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 decode failed: {}", e))?;

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| format!("image decode failed: {}", e))?;

    let rgba = img.to_rgba8();
    snapforge_core::clipboard::copy_image_to_clipboard(&rgba).map_err(|e| e.to_string())?;

    // Save a copy to disk for history
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;
    let save_path = config.save_file_path();
    if let Some(parent) = save_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    snapforge_core::format::save_image(
        &rgba,
        &save_path,
        config.screenshot_format,
        config.jpg_quality,
    )
    .map_err(|e| e.to_string())?;

    let _ = add_to_history(save_path.display().to_string());

    Ok(())
}

/// Copy an image file to the system clipboard.
#[tauri::command]
pub fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    let img = image::open(&path).map_err(|e| format!("failed to open image: {e}"))?;
    let rgba = img.to_rgba8();
    snapforge_core::clipboard::copy_image_to_clipboard(&rgba).map_err(|e| e.to_string())
}

/// Get the current app config as JSON.
#[tauri::command]
pub fn get_config() -> Result<String, String> {
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;
    serde_json::to_string(&config).map_err(|e| e.to_string())
}

/// Save the app config from JSON.
#[tauri::command]
pub fn save_config(config_json: String) -> Result<(), String> {
    let config: snapforge_core::config::AppConfig =
        serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.save().map_err(|e| e.to_string())
}

/// Open the save directory in the system file manager.
#[tauri::command]
pub fn open_save_folder() -> Result<(), String> {
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;
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
    let region = snapforge_core::types::Rect {
        x,
        y,
        width,
        height,
    };
    let image =
        snapforge_core::capture::capture_region(display, region).map_err(|e| e.to_string())?;
    snapforge_core::clipboard::copy_image_to_clipboard(&image).map_err(|e| e.to_string())?;
    Ok(())
}

/// Check if FFmpeg is installed.
#[tauri::command]
pub fn check_ffmpeg() -> Result<(), String> {
    snapforge_core::record::check_ffmpeg().map_err(|e| e.to_string())
}

/// Start recording. Returns the output file path.
/// Runs the actual recording start on a background thread since SCK capture
/// (used for the test frame) needs the main RunLoop to be free.
#[tauri::command]
pub fn start_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
    display: usize,
    region_x: Option<i32>,
    region_y: Option<i32>,
    region_w: Option<u32>,
    region_h: Option<u32>,
) -> Result<String, String> {
    let config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;

    let region = match (region_x, region_y, region_w, region_h) {
        (Some(x), Some(y), Some(w), Some(h)) => Some(snapforge_core::types::Rect {
            x,
            y,
            width: w,
            height: h,
        }),
        _ => None,
    };

    let output_path = config.recording_file_path();
    let record_config = snapforge_core::record::RecordConfig {
        display,
        region,
        output_path: output_path.clone(),
        format: config.recording.format,
        fps: config.recording.fps,
        quality: config.recording.quality,
        ffmpeg_path: None, // will search bundled sidecar then system PATH
    };

    // Run on background thread — SCK capture (test frame) needs the main RunLoop free
    let handle =
        std::thread::spawn(move || snapforge_core::record::ffmpeg::start_recording(record_config))
            .join()
            .map_err(|_| "recording thread panicked".to_string())?
            .map_err(|e| e.to_string())?;

    let path_str = output_path.display().to_string();

    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    *guard = Some(handle);

    if let Ok(mut path_guard) = state.output_path.lock() {
        *path_guard = Some(path_str.clone());
    }

    Ok(path_str)
}

/// Stop recording. Returns the output file path.
/// Adds the recording to history automatically.
#[tauri::command]
pub fn stop_recording(
    state: tauri::State<'_, crate::recording::RecordingState>,
) -> Result<String, String> {
    let mut guard = state.handle.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = guard.take() {
        handle.stop().map_err(|e| e.to_string())?;
    }

    let path = state
        .output_path
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .unwrap_or_default();

    // Add to history
    if !path.is_empty() {
        let _ = add_to_history(path.clone());
    }

    Ok(path)
}

/// Check if currently recording.
#[tauri::command]
pub fn is_recording(state: tauri::State<'_, crate::recording::RecordingState>) -> bool {
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

/// Get screenshot history as JSON, with base64-encoded thumbnail data.
/// Reads thumbnails in parallel for faster loading.
#[tauri::command]
pub fn get_history() -> Result<String, String> {
    let history = snapforge_core::history::ScreenshotHistory::load().map_err(|e| e.to_string())?;

    #[derive(serde::Serialize)]
    struct EntryWithData {
        path: String,
        timestamp: String,
        thumbnail_data: String,
    }

    // Read all thumbnail files in parallel using thread::scope
    let entries: Vec<EntryWithData> = std::thread::scope(|s| {
        let handles: Vec<_> = history
            .entries
            .iter()
            .map(|e| {
                let path = e.path.clone();
                let timestamp = e.timestamp.clone();
                let thumb_path = e.thumbnail_path.clone();
                s.spawn(move || {
                    let thumb_data = std::fs::read(&thumb_path)
                        .ok()
                        .map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
                        .unwrap_or_default();
                    EntryWithData {
                        path,
                        timestamp,
                        thumbnail_data: thumb_data,
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    serde_json::to_string(&entries).map_err(|e| e.to_string())
}

/// Add a screenshot to history and generate a thumbnail.
#[tauri::command]
pub fn add_to_history(path: String) -> Result<(), String> {
    let mut history =
        snapforge_core::history::ScreenshotHistory::load().map_err(|e| e.to_string())?;
    history.add_entry(&path).map_err(|e| e.to_string())
}

/// Open the folder containing the file, with the file selected.
#[tauri::command]
pub fn open_file_in_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.parent().is_some() {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(&path)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", &path))
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        {
            open::that(p.parent().unwrap()).map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[allow(unreachable_code)]
        {
            open::that(p.parent().unwrap()).map_err(|e| e.to_string())
        }
    } else {
        Err("Invalid file path".to_string())
    }
}

/// Clear all screenshot history.
#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    let mut history =
        snapforge_core::history::ScreenshotHistory::load().map_err(|e| e.to_string())?;
    history.clear().map_err(|e| e.to_string())
}

/// Remember the selected region for "capture last region" hotkey.
#[tauri::command]
pub fn save_last_region_to_config(
    display: usize,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let mut config = snapforge_core::config::AppConfig::load().map_err(|e| e.to_string())?;
    config.last_region = Some(snapforge_core::types::LastRegion {
        display,
        rect: snapforge_core::types::Rect {
            x,
            y,
            width,
            height,
        },
    });
    config.save().map_err(|e| e.to_string())
}
