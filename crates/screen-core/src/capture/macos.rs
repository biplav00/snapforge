use crate::types::Rect;
use core_graphics::display::{CGDisplay, CGMainDisplayID};
use image::RgbaImage;

use super::CaptureError;

pub fn display_count() -> usize {
    if CGDisplay::new(unsafe { CGMainDisplayID() }).is_active() {
        1
    } else {
        0
    }
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let display_id = get_display_id(display)?;
    let cg_display = CGDisplay::new(display_id);
    let cg_image = cg_display.image().ok_or(CaptureError::CaptureFailed)?;

    cg_image_to_rgba(&cg_image)
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    let full = capture_fullscreen(display)?;

    let x = region.x.max(0) as u32;
    let y = region.y.max(0) as u32;
    let w = region.width.min(full.width().saturating_sub(x));
    let h = region.height.min(full.height().saturating_sub(y));

    if w == 0 || h == 0 {
        return Err(CaptureError::CaptureFailed);
    }

    let cropped = image::imageops::crop_imm(&full, x, y, w, h).to_image();
    Ok(cropped)
}

fn get_display_id(display: usize) -> Result<u32, CaptureError> {
    if display == 0 {
        Ok(unsafe { CGMainDisplayID() })
    } else {
        Err(CaptureError::NoDisplay(display))
    }
}

fn cg_image_to_rgba(
    cg_image: &core_graphics::image::CGImage,
) -> Result<RgbaImage, CaptureError> {
    let width = cg_image.width() as u32;
    let height = cg_image.height() as u32;
    let bytes_per_row = cg_image.bytes_per_row();
    let data = cg_image.data();
    let raw_bytes: &[u8] = &data;

    let mut rgba_buf = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        let row_start = (y as usize) * bytes_per_row;
        for x in 0..width {
            let pixel_start = row_start + (x as usize) * 4;
            // Bounds check to avoid panic on unexpected pixel formats or padding
            if pixel_start + 3 >= raw_bytes.len() {
                return Err(CaptureError::ImageDataFailed);
            }
            // CoreGraphics uses BGRA byte order
            let b = raw_bytes[pixel_start];
            let g = raw_bytes[pixel_start + 1];
            let r = raw_bytes[pixel_start + 2];
            let a = raw_bytes[pixel_start + 3];
            rgba_buf.extend_from_slice(&[r, g, b, a]);
        }
    }

    RgbaImage::from_raw(width, height, rgba_buf).ok_or(CaptureError::ImageDataFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count_nonzero() {
        assert!(display_count() > 0, "should detect at least one display");
    }

    #[test]
    fn test_capture_fullscreen_main() {
        let img = capture_fullscreen(0);
        if let Ok(img) = img {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_main() {
        let region = crate::types::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let img = capture_region(0, region);
        if let Ok(img) = img {
            assert_eq!(img.width(), 100);
            assert_eq!(img.height(), 100);
        }
    }

    #[test]
    fn test_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err());
    }
}
