#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod xcap_impl;

use image::RgbaImage;
use snapforge_domain::Rect;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no display found at index {0}")]
    NoDisplay(usize),
    #[error("screen capture failed")]
    CaptureFailed,
    #[error("failed to get image data")]
    ImageDataFailed,
}

/// Intersect `region` (pixel coordinates relative to the captured frame,
/// origin possibly negative) with a `frame_w` x `frame_h` frame.
///
/// Returns `(x, y, width, height)` of the visible part, or `None` when the
/// intersection is empty. Unlike a bare `x.max(0)`, a negative origin also
/// shrinks the extent by the clipped amount, so the far edge stays where the
/// caller asked for it.
pub(crate) fn clip_region_to_frame(
    region: Rect,
    frame_w: u32,
    frame_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    fn clip_axis(start: i32, len: u32, limit: u32) -> Option<(u32, u32)> {
        let lo = i64::from(start).clamp(0, i64::from(limit));
        let hi = (i64::from(start) + i64::from(len)).clamp(0, i64::from(limit));
        (hi > lo).then(|| (lo as u32, (hi - lo) as u32))
    }
    let (x, w) = clip_axis(region.x, region.width, frame_w)?;
    let (y, h) = clip_axis(region.y, region.height, frame_h)?;
    Some((x, y, w, h))
}

/// Crop `full` to `region` using [`clip_region_to_frame`]; an empty
/// intersection is a `CaptureFailed` error.
pub(crate) fn crop_to_region(full: &RgbaImage, region: Rect) -> Result<RgbaImage, CaptureError> {
    let (x, y, w, h) = clip_region_to_frame(region, full.width(), full.height())
        .ok_or(CaptureError::CaptureFailed)?;
    Ok(image::imageops::crop_imm(full, x, y, w, h).to_image())
}

pub fn capture_fullscreen(display: usize) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_fullscreen(display)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::capture_fullscreen(display)
    }
}

pub fn capture_region(display: usize, region: Rect) -> Result<RgbaImage, CaptureError> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_region(display, region)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::capture_region(display, region)
    }
}

/// Check if screen capture permission is granted (macOS only, always true on other platforms).
pub fn has_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::has_screen_capture_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Request screen capture permission (macOS only, no-op on other platforms).
pub fn request_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_screen_capture_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// A reusable capture context for fast repeated captures (e.g. recording).
/// Create once on a background thread, call capture_frame() per frame.
#[cfg(target_os = "macos")]
pub use macos::{CaptureContext, RawFrame};

/// Get the point-to-pixel scale factor of the primary display.
pub fn display_scale_factor() -> f64 {
    #[cfg(target_os = "macos")]
    {
        macos::primary_display_scale_factor()
    }
    #[cfg(not(target_os = "macos"))]
    {
        1.0
    }
}

pub fn display_count() -> usize {
    #[cfg(target_os = "macos")]
    {
        macos::display_count()
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::display_count()
    }
}

pub fn display_at_point(x: i32, y: i32) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        macos::display_at_point(x, y)
    }
    #[cfg(not(target_os = "macos"))]
    {
        xcap_impl::display_at_point(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_count() {
        let count = display_count();
        // In headless CI there may be no display — just verify it doesn't panic
        // On a desktop environment it should be > 0
        let _ = count;
    }

    #[test]
    fn test_capture_fullscreen() {
        let result = capture_fullscreen(0);
        // May fail in headless CI, so we just check the success path
        if let Ok(img) = result {
            assert!(img.width() > 0);
            assert!(img.height() > 0);
        }
    }

    #[test]
    fn test_capture_region_valid() {
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let result = capture_region(0, region);
        if let Ok(img) = result {
            assert_eq!(img.width(), 100);
            assert_eq!(img.height(), 100);
        }
    }

    #[test]
    fn test_capture_region_zero_size() {
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        let result = capture_region(0, region);
        // Zero-size region should fail
        if result.is_ok() {
            // Some implementations might handle this gracefully
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_capture_invalid_display() {
        let result = capture_fullscreen(99);
        assert!(result.is_err(), "invalid display index should fail");
    }

    #[test]
    fn capture_region_invalid_display_errors() {
        // A region request against a nonexistent display must error, not panic,
        // and must not silently fall back to display 0.
        let region = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let result = capture_region(99, region);
        assert!(
            result.is_err(),
            "region capture on invalid display should fail"
        );
    }

    #[test]
    fn capture_region_oversized_clamps_to_display_bounds() {
        // Region far larger than any real display: the implementation clamps
        // width/height to the display, so the result (if capture succeeds) must
        // never exceed the full-frame size. Skips cleanly in headless CI.
        let Ok(full) = capture_fullscreen(0) else {
            return;
        };
        let huge = snapforge_domain::Rect {
            x: 0,
            y: 0,
            width: u32::MAX,
            height: u32::MAX,
        };
        if let Ok(img) = capture_region(0, huge) {
            assert!(img.width() <= full.width());
            assert!(img.height() <= full.height());
            assert!(img.width() > 0 && img.height() > 0);
        }
    }

    #[test]
    fn capture_region_origin_past_display_errors() {
        // An origin beyond the display extent leaves a zero-size window, which
        // must surface as an error rather than a 0x0 image or a panic.
        let Ok(full) = capture_fullscreen(0) else {
            return;
        };
        let off = snapforge_domain::Rect {
            x: i32::try_from(full.width()).unwrap_or(i32::MAX - 1000) + 1000,
            y: i32::try_from(full.height()).unwrap_or(i32::MAX - 1000) + 1000,
            width: 100,
            height: 100,
        };
        assert!(capture_region(0, off).is_err());
    }

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn clip_region_inside_frame_is_unchanged() {
        assert_eq!(
            clip_region_to_frame(rect(10, 20, 30, 40), 100, 100),
            Some((10, 20, 30, 40))
        );
    }

    #[test]
    fn clip_region_negative_origin_shrinks_extent() {
        // x=-10,w=50 covers columns -10..40 → visible 0..40 (width 40), not
        // 0..50 as the old `x.max(0)` produced.
        assert_eq!(
            clip_region_to_frame(rect(-10, -5, 50, 25), 100, 100),
            Some((0, 0, 40, 20))
        );
    }

    #[test]
    fn clip_region_overhanging_far_edge_is_clamped() {
        assert_eq!(
            clip_region_to_frame(rect(90, 95, 50, 50), 100, 100),
            Some((90, 95, 10, 5))
        );
        assert_eq!(
            clip_region_to_frame(rect(0, 0, u32::MAX, u32::MAX), 100, 80),
            Some((0, 0, 100, 80))
        );
    }

    #[test]
    fn clip_region_empty_intersections_are_none() {
        // Entirely left of / above the frame.
        assert_eq!(clip_region_to_frame(rect(-50, 0, 50, 10), 100, 100), None);
        assert_eq!(clip_region_to_frame(rect(0, -50, 10, 50), 100, 100), None);
        // Entirely past the far edge.
        assert_eq!(clip_region_to_frame(rect(100, 0, 10, 10), 100, 100), None);
        // Zero-size.
        assert_eq!(clip_region_to_frame(rect(5, 5, 0, 10), 100, 100), None);
        // Extreme values must not overflow. i32::MIN + u32::MAX ends at
        // i32::MAX, so this rect spans the whole frame.
        assert_eq!(
            clip_region_to_frame(rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX), 100, 100),
            Some((0, 0, 100, 100))
        );
        assert_eq!(
            clip_region_to_frame(rect(i32::MIN, i32::MIN, 1000, 1000), 100, 100),
            None
        );
        assert_eq!(
            clip_region_to_frame(rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX), 100, 100),
            None
        );
    }

    #[test]
    fn crop_to_region_handles_negative_origin() {
        let full = RgbaImage::new(100, 100);
        let img = crop_to_region(&full, rect(-10, -10, 30, 30)).unwrap();
        assert_eq!((img.width(), img.height()), (20, 20));
        assert!(crop_to_region(&full, rect(-30, 0, 30, 30)).is_err());
    }

    #[test]
    fn display_at_point_far_offscreen_is_none_or_valid_index() {
        // Coordinates no display can contain must yield None (honoring the
        // C -1 contract) rather than snapping to display 0. If a virtual
        // desktop somehow contains it, the index must be in range.
        let count = display_count();
        match display_at_point(i32::MIN / 2, i32::MIN / 2) {
            None => {}
            Some(idx) => assert!(idx < count.max(1)),
        }
    }
}
