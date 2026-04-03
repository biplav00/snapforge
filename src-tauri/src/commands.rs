use base64::Engine;
use base64::engine::general_purpose::STANDARD;

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
