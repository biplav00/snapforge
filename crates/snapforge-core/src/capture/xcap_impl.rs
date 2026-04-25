use crate::types::Rect;
use image::RgbaImage;

use super::CaptureError;

pub fn display_count() -> usize {
    xcap::Monitor::all()
        .map(|monitors| monitors.len())
        .unwrap_or(0)
}

/// Map a point to a monitor index using each monitor's bounding rect.
/// Falls back to 0 if no monitor contains the point.
pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    let monitors = xcap::Monitor::all().ok()?;
    for (i, m) in monitors.iter().enumerate() {
        let mx = m.x() as i32;
        let my = m.y() as i32;
        let mw = m.width() as i32;
        let mh = m.height() as i32;
        if x >= mx && x < mx + mw && y >= my && y < my + mh {
            return Some(i);
        }
    }
    Some(0)
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let monitors = xcap::Monitor::all().map_err(|_| CaptureError::CaptureFailed)?;
    let monitor = monitors
        .get(display)
        .ok_or(CaptureError::NoDisplay(display))?;
    let img = monitor
        .capture_image()
        .map_err(|_| CaptureError::CaptureFailed)?;
    Ok(img)
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
