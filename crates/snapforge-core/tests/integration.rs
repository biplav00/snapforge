use snapforge_core::types::{CaptureFormat, Rect};

/// True when the developer / CI runner has asserted "I have a real display
/// attached and capture is expected to succeed". Set on workstations and on
/// any CI matrix entry with a graphics session; unset on headless runners.
fn require_display() -> bool {
    std::env::var("SNAPFORGE_REQUIRE_DISPLAY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Either unwrap (display required) or return None (headless). Replaces the
/// old `if let Ok(...)` silent skip so a regression on a runner that DOES
/// have a display can fail CI.
fn assert_or_skip<T>(name: &str, result: Result<T, impl std::fmt::Debug>) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            if require_display() {
                panic!("{}: capture failed but SNAPFORGE_REQUIRE_DISPLAY=1: {:?}", name, e);
            }
            eprintln!(
                "[integration] skipping {}: capture unavailable on this runner: {:?}",
                name, e
            );
            None
        }
    }
}

#[test]
fn test_fullscreen_screenshot_png() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("fullscreen.png");

    let result = snapforge_core::screenshot_fullscreen(0, &path, CaptureFormat::Png, 90, false);

    let Some(saved_path) = assert_or_skip("test_fullscreen_screenshot_png", result) else {
        return;
    };
    assert!(saved_path.exists());
    let metadata = std::fs::metadata(&saved_path).unwrap();
    assert!(metadata.len() > 100, "PNG should be more than 100 bytes");

    let img = image::open(&saved_path).unwrap();
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

    let result = snapforge_core::screenshot_region(0, region, &path, CaptureFormat::Jpg, 85, false);

    let Some(saved_path) = assert_or_skip("test_region_screenshot_jpg", result) else {
        return;
    };
    assert!(saved_path.exists());
    let img = image::open(&saved_path).unwrap();
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

    let result =
        snapforge_core::screenshot_region(0, region, &path, CaptureFormat::WebP, 90, false);

    let Some(saved_path) = assert_or_skip("test_region_screenshot_webp", result) else {
        return;
    };
    assert!(saved_path.exists());
    let metadata = std::fs::metadata(&saved_path).unwrap();
    assert!(metadata.len() > 0);
}
