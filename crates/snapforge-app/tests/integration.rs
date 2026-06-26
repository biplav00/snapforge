//! Real-display screenshot round-trips through the `take_screenshot` use case.
//!
//! Ported from the deleted `snapforge-core` facade (which had its own
//! `screenshot_fullscreen` / `screenshot_region` orchestrators) onto the
//! single use-case entry point. Display-gated: skipped on headless runners
//! unless `SNAPFORGE_REQUIRE_DISPLAY=1`.

use snapforge_app::screenshot::{take_screenshot, ScreenshotRequest};
use snapforge_app::{CaptureFormat, Rect};
use std::path::PathBuf;

/// True when the developer / CI runner has asserted "I have a real display
/// attached and capture is expected to succeed". Set on workstations and on
/// any CI matrix entry with a graphics session; unset on headless runners.
fn require_display() -> bool {
    std::env::var("SNAPFORGE_REQUIRE_DISPLAY")
        .is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Either unwrap (display required) or return None (headless). A regression on
/// a runner that DOES have a display fails CI rather than silently skipping.
fn assert_or_skip<T>(name: &str, result: Result<T, impl std::fmt::Debug>) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            assert!(
                !require_display(),
                "{}: capture failed but SNAPFORGE_REQUIRE_DISPLAY=1: {:?}",
                name,
                e
            );
            eprintln!(
                "[integration] skipping {}: capture unavailable on this runner: {:?}",
                name, e
            );
            None
        }
    }
}

fn request(
    region: Option<Rect>,
    path: PathBuf,
    format: CaptureFormat,
    quality: u8,
) -> ScreenshotRequest {
    ScreenshotRequest {
        display: 0,
        region,
        output_path: path,
        format,
        quality,
        copy_to_clipboard: false,
        add_to_history: false,
    }
}

#[test]
fn test_fullscreen_screenshot_png() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("fullscreen.png");

    let result = take_screenshot(request(None, path, CaptureFormat::Png, 90));

    let Some(res) = assert_or_skip("test_fullscreen_screenshot_png", result) else {
        return;
    };
    assert!(res.saved_path.exists());
    let metadata = std::fs::metadata(&res.saved_path).unwrap();
    assert!(metadata.len() > 100, "PNG should be more than 100 bytes");

    let img = image::open(&res.saved_path).unwrap();
    assert!(img.width() > 0);
    assert!(img.height() > 0);
}

#[test]
fn test_region_screenshot_jpg() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.jpg");
    let region = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 150,
    };

    let result = take_screenshot(request(Some(region), path, CaptureFormat::Jpg, 85));

    let Some(res) = assert_or_skip("test_region_screenshot_jpg", result) else {
        return;
    };
    assert!(res.saved_path.exists());
    let img = image::open(&res.saved_path).unwrap();
    assert_eq!(img.width(), 200);
    assert_eq!(img.height(), 150);
}

#[test]
fn test_region_screenshot_webp() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.webp");
    let region = Rect {
        x: 50,
        y: 50,
        width: 100,
        height: 100,
    };

    let result = take_screenshot(request(Some(region), path, CaptureFormat::WebP, 90));

    let Some(res) = assert_or_skip("test_region_screenshot_webp", result) else {
        return;
    };
    assert!(res.saved_path.exists());
    let metadata = std::fs::metadata(&res.saved_path).unwrap();
    assert!(metadata.len() > 0);
}
