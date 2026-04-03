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
