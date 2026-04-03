use crate::types::CaptureFormat;
use image::RgbaImage;
use std::io::Cursor;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("failed to encode image: {0}")]
    EncodeFailed(String),
    #[error("failed to write file: {0}")]
    WriteFailed(#[from] std::io::Error),
    #[error("unsupported format: {0}")]
    Unsupported(String),
}

/// Encode an RgbaImage to bytes in the given format.
pub fn encode_image(
    image: &RgbaImage,
    format: CaptureFormat,
    quality: u8,
) -> Result<Vec<u8>, FormatError> {
    let mut buf = Cursor::new(Vec::new());

    match format {
        CaptureFormat::Png => {
            image
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
        CaptureFormat::Jpg => {
            let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
        CaptureFormat::WebP => {
            image
                .write_to(&mut buf, image::ImageFormat::WebP)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
        }
    }

    Ok(buf.into_inner())
}

/// Save an RgbaImage to a file path. Creates parent directories if needed.
pub fn save_image(
    image: &RgbaImage,
    path: &Path,
    format: CaptureFormat,
    quality: u8,
) -> Result<(), FormatError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = encode_image(image, format, quality)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image() -> RgbaImage {
        RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn test_encode_png() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::Png, 90).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn test_encode_jpg() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::Jpg, 90).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_encode_webp() {
        let img = test_image();
        let bytes = encode_image(&img, CaptureFormat::WebP, 90).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[0..4], b"RIFF");
    }

    #[test]
    fn test_save_image_creates_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/dir/test.png");
        let img = test_image();
        save_image(&img, &path, CaptureFormat::Png, 90).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_save_image_jpg_quality() {
        let tmp = tempfile::tempdir().unwrap();
        let img = test_image();

        let path_high = tmp.path().join("high.jpg");
        save_image(&img, &path_high, CaptureFormat::Jpg, 95).unwrap();

        let path_low = tmp.path().join("low.jpg");
        save_image(&img, &path_low, CaptureFormat::Jpg, 10).unwrap();

        let size_high = std::fs::metadata(&path_high).unwrap().len();
        let size_low = std::fs::metadata(&path_low).unwrap().len();
        assert!(
            size_high > size_low,
            "higher quality should produce larger file"
        );
    }
}
