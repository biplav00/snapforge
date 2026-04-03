#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CaptureFormat {
    #[default]
    Png,
    Jpg,
    WebP,
}

impl CaptureFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            CaptureFormat::Png => "png",
            CaptureFormat::Jpg => "jpg",
            CaptureFormat::WebP => "webp",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(CaptureFormat::Png),
            "jpg" | "jpeg" => Some(CaptureFormat::Jpg),
            "webp" => Some(CaptureFormat::WebP),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LastRegion {
    pub display: usize,
    pub rect: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_format_extension() {
        assert_eq!(CaptureFormat::Png.extension(), "png");
        assert_eq!(CaptureFormat::Jpg.extension(), "jpg");
        assert_eq!(CaptureFormat::WebP.extension(), "webp");
    }

    #[test]
    fn test_capture_format_from_extension() {
        assert_eq!(
            CaptureFormat::from_extension("png"),
            Some(CaptureFormat::Png)
        );
        assert_eq!(
            CaptureFormat::from_extension("PNG"),
            Some(CaptureFormat::Png)
        );
        assert_eq!(
            CaptureFormat::from_extension("jpg"),
            Some(CaptureFormat::Jpg)
        );
        assert_eq!(
            CaptureFormat::from_extension("jpeg"),
            Some(CaptureFormat::Jpg)
        );
        assert_eq!(
            CaptureFormat::from_extension("webp"),
            Some(CaptureFormat::WebP)
        );
        assert_eq!(CaptureFormat::from_extension("bmp"), None);
    }

    #[test]
    fn test_capture_format_default() {
        assert_eq!(CaptureFormat::default(), CaptureFormat::Png);
    }

    #[test]
    fn test_rect_serde_roundtrip() {
        let rect = Rect {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };
        let json = serde_json::to_string(&rect).unwrap();
        let deserialized: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(rect, deserialized);
    }
}
