#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// Serializes as the variant name ("Png"/"Jpg"/"WebP"), which is what the
// on-disk config and the Qt config bridge persist. The `alias` attributes let
// deserialization also accept the lowercase spellings the screenshot/record
// request JSON uses, so a single serde definition covers both wire paths and
// the two spellings can no longer drift apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CaptureFormat {
    #[default]
    #[serde(alias = "png")]
    Png,
    #[serde(alias = "jpg", alias = "jpeg")]
    Jpg,
    #[serde(alias = "webp")]
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
