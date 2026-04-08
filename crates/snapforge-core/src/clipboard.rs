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
/// On macOS, writes PNG data directly to NSPasteboard via objc2.
/// On Windows, writes a DIB bitmap via Win32 clipboard API.
/// On Linux, pipes PNG data to wl-copy or xclip.
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
        copy_png_to_clipboard_win(image)?;
    }

    #[cfg(target_os = "linux")]
    {
        copy_png_via_tool(&png_bytes)?;
    }

    Ok(())
}

/// Write PNG bytes directly to macOS NSPasteboard using objc2.
/// No temp file or process spawn — uses the native Cocoa pasteboard API.
#[cfg(target_os = "macos")]
fn copy_png_to_pasteboard(png_bytes: &[u8]) -> Result<(), ClipboardError> {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSData;

    unsafe {
        // Find NSPasteboard class — may not exist in headless/test environments
        let cls = AnyClass::get(c"NSPasteboard").ok_or_else(|| {
            ClipboardError::AccessFailed("NSPasteboard class not available".to_string())
        })?;

        let pasteboard: Retained<AnyObject> = msg_send![cls, generalPasteboard];
        let _: () = msg_send![&pasteboard, clearContents];

        let data = NSData::with_bytes(png_bytes);
        let png_type = objc2_foundation::NSString::from_str("public.png");
        let success: bool = msg_send![&pasteboard, setData: &*data, forType: &*png_type];

        if !success {
            return Err(ClipboardError::SetFailed(
                "NSPasteboard setData:forType: returned NO".to_string(),
            ));
        }
    }

    Ok(())
}

/// Write PNG to clipboard on Windows using clipboard-win (direct Win32 API).
/// Decodes PNG to a DIB bitmap and sets it via the Win32 clipboard API.
#[cfg(target_os = "windows")]
fn copy_png_to_clipboard_win(image: &RgbaImage) -> Result<(), ClipboardError> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    let img_data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: std::borrow::Cow::Borrowed(image.as_raw()),
    };

    clipboard
        .set_image(img_data)
        .map_err(|e| ClipboardError::SetFailed(e.to_string()))
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
