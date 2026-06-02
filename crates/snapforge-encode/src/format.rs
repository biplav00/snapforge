use image::RgbaImage;
use snapforge_domain::CaptureFormat;
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
    let quality = quality.clamp(1, 100);
    let (w, h) = (image.width() as usize, image.height() as usize);

    match format {
        CaptureFormat::Png => {
            // NOTE: PNG deflate (via the `png` crate) is single-threaded and
            // cannot be parallelized without swapping the encoder, which would
            // change output bytes and is out of scope here. The `image` rayon
            // feature accelerates other codecs (e.g. JPEG) but not PNG deflate.
            let mut buf = Cursor::new(Vec::with_capacity(w * h * 4));
            image
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
            Ok(buf.into_inner())
        }
        CaptureFormat::Jpg => {
            // JPEG has no alpha channel; warn once if the image has any
            // non-opaque pixel so the user knows the alpha was dropped.
            // Sample the corners + center first as a fast path; full scan
            // only if those look opaque but we want to be sure for arbitrary
            // sizes — the cost is dominated by the encode anyway.
            // Scan the alpha bytes (every 4th byte of the packed RGBA slice).
            // Parallelized for large frames so the pre-encode pass doesn't add
            // a serial walk over the full 4K buffer.
            let alpha_src = image.as_raw();
            let has_transparency = if w * h >= 256 * 256 {
                use rayon::prelude::*;
                alpha_src.par_chunks(4).any(|px| px[3] < 255)
            } else {
                alpha_src.chunks(4).any(|px| px[3] < 255)
            };
            if has_transparency {
                tracing::warn!(
                    "[snapforge] JPEG output drops the alpha channel; transparent \
                     pixels in the source will be flattened. Use PNG or WebP to \
                     preserve transparency."
                );
            }

            // Convert RGBA to RGB without cloning into DynamicImage. Rather than
            // pushing 3 bytes per pixel through the pixels() iterator (which
            // bounds-checks and reallocs), copy directly from the raw RGBA slice
            // into an exactly-sized, uninitialized buffer in 4->3 byte chunks.
            // For large frames (4K) the work is split across rayon threads.
            let src = image.as_raw(); // &[u8], len == w*h*4, packed RGBA
            let mut rgb_buf = vec![0u8; w * h * 3];

            // Below this many pixels the rayon dispatch overhead outweighs the
            // copy; do it inline on the calling thread.
            const PAR_THRESHOLD: usize = 256 * 256;
            if w * h >= PAR_THRESHOLD {
                use rayon::prelude::*;
                rgb_buf
                    .par_chunks_mut(3)
                    .zip(src.par_chunks(4))
                    .for_each(|(dst, px)| {
                        dst[0] = px[0];
                        dst[1] = px[1];
                        dst[2] = px[2];
                    });
            } else {
                for (dst, px) in rgb_buf.chunks_mut(3).zip(src.chunks(4)) {
                    dst[0] = px[0];
                    dst[1] = px[1];
                    dst[2] = px[2];
                }
            }

            let mut buf = Cursor::new(Vec::with_capacity(w * h * 3));
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            encoder
                .encode(
                    &rgb_buf,
                    image.width(),
                    image.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
            Ok(buf.into_inner())
        }
        CaptureFormat::WebP => {
            let mut buf = Cursor::new(Vec::with_capacity(w * h * 4));
            image
                .write_to(&mut buf, image::ImageFormat::WebP)
                .map_err(|e| FormatError::EncodeFailed(e.to_string()))?;
            Ok(buf.into_inner())
        }
    }
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

    #[test]
    fn encode_single_pixel_png_succeeds() {
        let img = RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]));
        let bytes = encode_image(&img, CaptureFormat::Png, 90).unwrap();
        assert_eq!(&bytes[0..4], &[0x89, b'P', b'N', b'G']);
    }

    #[test]
    fn encode_odd_dimension_jpg_succeeds() {
        // JPEG/RGB packing walks pixels manually; an odd width (3x1) exercises
        // the non-even path that the recorder pipeline normally crops away.
        let img = RgbaImage::from_pixel(3, 1, image::Rgba([10, 20, 30, 255]));
        let bytes = encode_image(&img, CaptureFormat::Jpg, 80).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn encode_zero_width_image_errors_not_panics() {
        // A zero-sized frame must surface an EncodeFailed error rather than
        // panic or emit a bogus file. RgbaImage permits 0xN buffers.
        let img = RgbaImage::new(0, 10);
        let result = encode_image(&img, CaptureFormat::Png, 90);
        assert!(matches!(result, Err(FormatError::EncodeFailed(_))));
    }

    #[test]
    fn encode_zero_height_jpg_errors_not_panics() {
        let img = RgbaImage::new(10, 0);
        let result = encode_image(&img, CaptureFormat::Jpg, 90);
        assert!(matches!(result, Err(FormatError::EncodeFailed(_))));
    }

    #[test]
    fn encode_quality_zero_clamps_and_succeeds() {
        // quality is clamped to [1,100]; 0 must not divide-by-zero or reject.
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([200, 100, 50, 255]));
        let bytes = encode_image(&img, CaptureFormat::Jpg, 0).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn encode_quality_above_max_clamps_and_succeeds() {
        let img = RgbaImage::from_pixel(8, 8, image::Rgba([200, 100, 50, 255]));
        let bytes = encode_image(&img, CaptureFormat::Jpg, 255).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn encode_jpg_large_image_roundtrips_dimensions_and_color() {
        // Exercise the parallel RGBA->RGB chunked-copy path (>= 256*256 px) and
        // verify the produced JPEG decodes back to the right dimensions and an
        // approximately correct color (JPEG is lossy, so allow a small delta).
        let (w, h) = (300u32, 300u32);
        let img = RgbaImage::from_pixel(w, h, image::Rgba([200, 100, 50, 255]));
        let bytes = encode_image(&img, CaptureFormat::Jpg, 95).unwrap();

        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
            .unwrap()
            .to_rgb8();
        assert_eq!(decoded.dimensions(), (w, h));
        let center = decoded.get_pixel(w / 2, h / 2);
        assert!((center[0] as i32 - 200).abs() <= 6);
        assert!((center[1] as i32 - 100).abs() <= 6);
        assert!((center[2] as i32 - 50).abs() <= 6);
    }

    #[test]
    fn encode_jpg_drops_alpha_without_error() {
        // A semi-transparent source still encodes (alpha is flattened, with a
        // warning logged); the JPEG magic must be intact.
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 0]));
        let bytes = encode_image(&img, CaptureFormat::Jpg, 90).unwrap();
        assert_eq!(&bytes[0..2], &[0xFF, 0xD8]);
    }
}
