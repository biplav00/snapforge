use screen_core::types::{CaptureFormat, Rect};

#[test]
fn test_fullscreen_screenshot_png() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("fullscreen.png");

    let result = screen_core::screenshot_fullscreen(
        0,
        &path,
        CaptureFormat::Png,
        90,
        false,
    );

    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let metadata = std::fs::metadata(&saved_path).unwrap();
        assert!(metadata.len() > 100, "PNG should be more than 100 bytes");

        let img = image::open(&saved_path).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }
}

#[test]
fn test_region_screenshot_jpg() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.jpg");
    let region = Rect { x: 0, y: 0, width: 200, height: 150 };

    let result = screen_core::screenshot_region(
        0,
        region,
        &path,
        CaptureFormat::Jpg,
        85,
        false,
    );

    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let img = image::open(&saved_path).unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 150);
    }
}

#[test]
fn test_region_screenshot_webp() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("region.webp");
    let region = Rect { x: 50, y: 50, width: 100, height: 100 };

    let result = screen_core::screenshot_region(
        0,
        region,
        &path,
        CaptureFormat::WebP,
        90,
        false,
    );

    if let Ok(saved_path) = result {
        assert!(saved_path.exists());
        let metadata = std::fs::metadata(&saved_path).unwrap();
        assert!(metadata.len() > 0);
    }
}
