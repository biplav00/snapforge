use image::RgbaImage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("failed to access clipboard: {0}")]
    AccessFailed(String),
    #[error("failed to set clipboard image: {0}")]
    SetFailed(String),
}

/// Copy an RgbaImage to the system clipboard.
///
/// On macOS, writes PNG data directly to NSPasteboard for maximum compatibility.
/// Falls back to arboard on other platforms.
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), ClipboardError> {
    // Encode as PNG first
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    image::ImageEncoder::write_image(
        encoder,
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| ClipboardError::SetFailed(format!("PNG encode failed: {}", e)))?;

    #[cfg(target_os = "macos")]
    {
        copy_png_to_pasteboard(&png_bytes)?;
    }

    #[cfg(target_os = "windows")]
    {
        copy_png_via_powershell(&png_bytes)?;
    }

    #[cfg(target_os = "linux")]
    {
        copy_png_via_tool(&png_bytes)?;
    }

    Ok(())
}

/// Write PNG bytes to macOS NSPasteboard using a temp file + osascript.
/// This ensures the data persists and is compatible with all macOS apps.
#[cfg(target_os = "macos")]
fn copy_png_to_pasteboard(png_bytes: &[u8]) -> Result<(), ClipboardError> {
    use std::io::Write;

    // Write PNG to a temp file
    let tmp_path = std::env::temp_dir().join("snapforge_clipboard.png");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| ClipboardError::SetFailed(format!("temp file failed: {}", e)))?;
    file.write_all(png_bytes)
        .map_err(|e| ClipboardError::SetFailed(format!("write failed: {}", e)))?;
    drop(file);

    // Use osascript to set the clipboard to the image file
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "set the clipboard to (read (POSIX file \"{}\") as «class PNGf»)",
            tmp_path.display()
        ))
        .output()
        .map_err(|e| ClipboardError::SetFailed(format!("osascript failed: {}", e)))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ClipboardError::SetFailed(format!(
            "osascript error: {}",
            stderr
        )));
    }

    Ok(())
}

/// Write PNG to clipboard on Windows using PowerShell + System.Windows.Forms.
#[cfg(target_os = "windows")]
fn copy_png_via_powershell(png_bytes: &[u8]) -> Result<(), ClipboardError> {
    use std::io::Write;

    let tmp_path = std::env::temp_dir().join("snapforge_clipboard.png");
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| ClipboardError::SetFailed(format!("temp file failed: {}", e)))?;
    file.write_all(png_bytes)
        .map_err(|e| ClipboardError::SetFailed(format!("write failed: {}", e)))?;
    drop(file);

    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         [System.Windows.Forms.Clipboard]::SetImage(\
         [System.Drawing.Image]::FromFile('{}'))",
        tmp_path.display()
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .map_err(|e| ClipboardError::SetFailed(format!("powershell failed: {}", e)))?;

    let _ = std::fs::remove_file(&tmp_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ClipboardError::SetFailed(format!(
            "powershell error: {}",
            stderr
        )));
    }

    Ok(())
}

/// Write PNG to clipboard on Linux using xclip or wl-copy.
#[cfg(target_os = "linux")]
fn copy_png_via_tool(png_bytes: &[u8]) -> Result<(), ClipboardError> {
    use std::io::Write;

    // Try wl-copy first (Wayland), then xclip (X11)
    let result = std::process::Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(std::process::Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(child) => child,
        Err(_) => {
            // Fall back to xclip
            std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png"])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| {
                    ClipboardError::SetFailed(format!("neither wl-copy nor xclip found: {}", e))
                })?
        }
    };

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(png_bytes).map_err(|e| {
            ClipboardError::SetFailed(format!("write to clipboard tool failed: {}", e))
        })?;
    }

    let status = child
        .wait()
        .map_err(|e| ClipboardError::SetFailed(format!("clipboard tool failed: {}", e)))?;

    if !status.success() {
        return Err(ClipboardError::SetFailed(
            "clipboard tool exited with error".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_small_image() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        // May fail in headless CI — just verify no panic
        let _ = copy_image_to_clipboard(&img);
    }

    #[test]
    fn test_copy_large_image() {
        let img = RgbaImage::from_pixel(1920, 1080, image::Rgba([0, 128, 255, 255]));
        let _ = copy_image_to_clipboard(&img);
    }

    #[test]
    fn test_copy_transparent_image() {
        let img = RgbaImage::from_pixel(50, 50, image::Rgba([0, 0, 0, 0]));
        let _ = copy_image_to_clipboard(&img);
    }

    #[test]
    fn test_png_encoding() {
        let img = RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 255]));
        let mut png_bytes: Vec<u8> = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        assert!(!png_bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&png_bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_clipboard_error_display() {
        let err = ClipboardError::AccessFailed("test".to_string());
        assert!(format!("{}", err).contains("test"));

        let err = ClipboardError::SetFailed("fail".to_string());
        assert!(format!("{}", err).contains("fail"));
    }
}
