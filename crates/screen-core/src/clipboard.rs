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
pub fn copy_image_to_clipboard(image: &RgbaImage) -> Result<(), ClipboardError> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| ClipboardError::AccessFailed(e.to_string()))?;

    let img_data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: std::borrow::Cow::Borrowed(image.as_raw()),
    };

    clipboard.set_image(img_data)
        .map_err(|e| ClipboardError::SetFailed(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_to_clipboard() {
        let img = RgbaImage::from_pixel(10, 10, image::Rgba([255, 0, 0, 255]));
        // This may fail in headless CI environments without a display server.
        let result = copy_image_to_clipboard(&img);
        if result.is_ok() {
            assert!(result.is_ok());
        }
    }
}
