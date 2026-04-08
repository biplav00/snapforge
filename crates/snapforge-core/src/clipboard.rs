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
        copy_png_to_clipboard_win(&png_bytes)?;
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
fn copy_png_to_clipboard_win(png_bytes: &[u8]) -> Result<(), ClipboardError> {
    use clipboard_win::{formats, set_clipboard};

    // Decode PNG to raw RGBA pixels, then convert to a BMP/DIB for the clipboard.
    // clipboard-win's Bitmap format expects a top-down DIB without file header.
    let img = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .map_err(|e| ClipboardError::SetFailed(format!("PNG decode failed: {}", e)))?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    // Build a BITMAPV5HEADER (124 bytes) for a top-down 32-bit BGRA DIB
    let header_size: u32 = 124;
    let row_size = (width * 4) as usize;
    let pixel_size = row_size * height as usize;
    let mut dib = Vec::with_capacity(header_size as usize + pixel_size);

    // BITMAPV5HEADER
    dib.extend_from_slice(&header_size.to_le_bytes()); // biSize
    dib.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(-(height as i32)).to_le_bytes()); // biHeight (negative = top-down)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&3u32.to_le_bytes()); // biCompression = BI_BITFIELDS
    dib.extend_from_slice(&(pixel_size as u32).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    dib.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
                                                // Color masks: R, G, B, A
    dib.extend_from_slice(&0x00FF0000u32.to_le_bytes()); // bV5RedMask
    dib.extend_from_slice(&0x0000FF00u32.to_le_bytes()); // bV5GreenMask
    dib.extend_from_slice(&0x000000FFu32.to_le_bytes()); // bV5BlueMask
    dib.extend_from_slice(&0xFF000000u32.to_le_bytes()); // bV5AlphaMask
    dib.extend_from_slice(&0x73524742u32.to_le_bytes()); // bV5CSType = LCS_sRGB
                                                         // CIEXYZTRIPLE (36 bytes of zeros)
    dib.extend_from_slice(&[0u8; 36]);
    // Gamma values (3 × 4 bytes of zeros)
    dib.extend_from_slice(&[0u8; 12]);
    dib.extend_from_slice(&4u32.to_le_bytes()); // bV5Intent = LCS_GM_IMAGES
    dib.extend_from_slice(&0u32.to_le_bytes()); // bV5ProfileData
    dib.extend_from_slice(&0u32.to_le_bytes()); // bV5ProfileSize
    dib.extend_from_slice(&0u32.to_le_bytes()); // bV5Reserved

    // Write pixel data as BGRA
    for pixel in rgba.pixels() {
        dib.push(pixel[2]); // B
        dib.push(pixel[1]); // G
        dib.push(pixel[0]); // R
        dib.push(pixel[3]); // A
    }

    set_clipboard(formats::CF_DIB, &dib)
        .map_err(|e| ClipboardError::SetFailed(format!("Win32 clipboard failed: {}", e)))?;

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
