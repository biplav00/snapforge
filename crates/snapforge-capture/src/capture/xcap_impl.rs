//! Non-macOS capture backend built on `xcap` 0.9.
//!
//! NOTE (units): `xcap::Monitor` reports `x/y/width/height` in the platform's
//! native desktop units (physical pixels on X11/Windows), while
//! `capture_image` returns physical pixels. `display_at_point` therefore
//! expects a point in the same desktop units; a HiDPI Qt caller passing
//! logical coordinates must scale by the monitor's `scale_factor` first.

use image::RgbaImage;
use snapforge_domain::Rect;

use super::CaptureError;

pub fn display_count() -> usize {
    xcap::Monitor::all()
        .map(|monitors| monitors.len())
        .unwrap_or(0)
}

/// Map a point to a monitor index using each monitor's bounding rect.
/// Returns `None` if no monitor contains the point (C contract: -1).
pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    let monitors = xcap::Monitor::all().ok()?;
    let (px, py) = (i64::from(x), i64::from(y));
    monitors.iter().position(|m| {
        let (Ok(mx), Ok(my), Ok(mw), Ok(mh)) = (m.x(), m.y(), m.width(), m.height()) else {
            return false;
        };
        let (mx, my) = (i64::from(mx), i64::from(my));
        px >= mx && px < mx + i64::from(mw) && py >= my && py < my + i64::from(mh)
    })
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    let monitors = xcap::Monitor::all().map_err(|_| CaptureError::CaptureFailed)?;
    let monitor = monitors
        .get(display)
        .ok_or(CaptureError::NoDisplay(display))?;
    monitor
        .capture_image()
        .map_err(|_| CaptureError::CaptureFailed)
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    let full = capture_fullscreen(display)?;
    super::crop_to_region(&full, region)
}
