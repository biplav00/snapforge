use crate::types::Rect;
use core_graphics::display::CGDisplay;
use image::RgbaImage;

use super::CaptureError;

/// Get IDs of all active displays.
fn active_display_ids() -> Vec<u32> {
    // CGGetActiveDisplayList: up to 16 displays
    let mut ids: Vec<u32> = vec![0; 16];
    let mut count: u32 = 0;
    unsafe {
        core_graphics::display::CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut count);
    }
    ids.truncate(count as usize);
    ids
}

pub fn display_count() -> usize {
    active_display_ids().len()
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
    let ids = active_display_ids();
    ids.get(display)
        .copied()
        .ok_or(CaptureError::NoDisplay(display))
}

fn cg_image_to_rgba(cg_image: &core_graphics::image::CGImage) -> Result<RgbaImage, CaptureError> {
    let width = cg_image.width() as u32;
    let height = cg_image.height() as u32;
    let bytes_per_row = cg_image.bytes_per_row();
    let data = cg_image.data();
    let raw_bytes: &[u8] = &data;

    let expected_pixels = (width * height) as usize;
    let expected_bytes = expected_pixels * 4;

    // Validate that the source buffer has enough data (one upfront check instead of per-pixel)
    let last_row_start = (height as usize - 1) * bytes_per_row;
    let min_required = last_row_start + (width as usize) * 4;
    if raw_bytes.len() < min_required {
        return Err(CaptureError::ImageDataFailed);
    }

    let mut rgba_buf = vec![0u8; expected_bytes];

    for y in 0..height as usize {
        let row_start = y * bytes_per_row;
        let dst_row_start = y * (width as usize) * 4;
        let src_row = &raw_bytes[row_start..row_start + (width as usize) * 4];
        let dst_row = &mut rgba_buf[dst_row_start..dst_row_start + (width as usize) * 4];

        // Swap B and R channels: BGRA → RGBA
        for (src, dst) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            dst[0] = src[2]; // R
            dst[1] = src[1]; // G
            dst[2] = src[0]; // B
            dst[3] = src[3]; // A
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
