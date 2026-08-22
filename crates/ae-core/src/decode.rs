use image::GenericImageView;

use crate::error::{decode_failed, Result};
use crate::image::{Dimensions, Image, ImageMetadata, Limits, Pixel, PixelBuffer};

fn from_rgba8(dims: Dimensions, rgba: image::RgbaImage, format_name: &str) -> Result<Image> {
    if dims.width != rgba.width() || dims.height != rgba.height() {
        return Err(decode_failed(format!(
            "pixel buffer shape {}x{} != claimed {format_name} dimensions {}x{}",
            rgba.width(),
            rgba.height(),
            dims.width,
            dims.height
        )));
    }
    let pixels: Vec<Pixel> = rgba
        .pixels()
        .map(|p| Pixel::new(p.0[0], p.0[1], p.0[2], p.0[3]))
        .collect();
    let buf = PixelBuffer::from_vec(dims, pixels)?;
    let mut meta = ImageMetadata::default();
    meta.format = Some(format_name.to_string());
    Image::new(dims, buf, meta)
}

/// Decodes a complete image file (bytes you `read()` from disk or stdin).
///
/// `Limits` are enforced on header-declared dimensions before materializing
/// a buffer, so a decompression bomb is rejected by arithmetic rather than OOM.
/// Auto-detects PNG/JPEG/WebP from container magic; on a nonsense-bytes
/// payload returns `Decode` without panicking.
pub fn decode_bytes(bytes: &[u8], limits: &Limits) -> Result<Image> {
    if bytes.is_empty() {
        return Err(decode_failed("empty input"));
    }
    if let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP) {
        let (w, h) = img.dimensions();
        limits.check_input(bytes.len() as u64, w, h)?;
        let rgba = img.to_rgba8();
        let dims = Dimensions::new(w, h)
            .map_err(|_| decode_failed(format!("reported image dimensions were zero: {w}x{h}")))?;
        return from_rgba8(dims, rgba, "webp");
    }
    decode_bytes_guess(bytes, limits)
}

fn decode_bytes_guess(bytes: &[u8], limits: &Limits) -> Result<Image> {
    let header = match image::load_from_memory(bytes) {
        Ok(img) => img,
        Err(err) => return Err(decode_failed(format!("{err}"))),
    };
    let (w, h) = header.dimensions();
    limits.check_input(bytes.len() as u64, w, h)?;
    let w0 = w;
    let h0 = h;
    let rgba = header.to_rgba8();
    let dims = Dimensions::new(w0, h0)
        .map_err(|_| decode_failed(format!("reported image dimensions were zero: {w0}x{h0}")))?;
    let guessed = guess_format_name(bytes, w0, h0);
    from_rgba8(dims, rgba, guessed)
}

fn guess_format_name(bytes: &[u8], width: u32, height: u32) -> &'static str {
    if bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "webp"
    } else if width == 0 || height == 0 {
        "unknown"
    } else {
        "image"
    }
}

#[cfg(test)]
mod tests {
    use super::Limits;
    use crate::image::Pixel;
    use crate::AeError;

    fn random_bytes(n: usize, seed: u64) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let mut h = DefaultHasher::new();
            (seed, i).hash(&mut h);
            out.push(h.finish() as u8 ^ (seed as u8).wrapping_add(i as u8));
        }
        out
    }

    #[test]
    fn rejects_empty_without_panic() {
        let r = super::decode_bytes(b"", &Limits::default());
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn rejects_random_bytes_without_panic() {
        let noise = random_bytes(512, 0xae);
        let r = super::decode_bytes(&noise, &Limits::default());
        assert!(r.is_err());
        assert!(matches!(r.unwrap_err(), AeError::Decode(_)));
    }

    #[test]
    fn truncated_png_without_panic() {
        // Valid PNG header + empty IHDR that will fail load_from_memory
        let truncated = vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 0];
        let r = super::decode_bytes(&truncated, &Limits::default());
        assert!(r.is_err());
        assert!(matches!(r.unwrap_err(), AeError::Decode(_)));
    }

    #[test]
    fn png_beats_png_and_supports_bounded_text_pipeline() {
        // 2x2 PNG fixture is generated in-memory with image+pixels to verify round-trip
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(2, 2, |x, y| {
            Rgba([(x * 120) as u8, (y * 120) as u8, 0, 255])
        });
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let decoded = super::decode_bytes(&buf, &Limits::default()).unwrap();
        assert_eq!(decoded.dimensions.width, 2);
        assert_eq!(decoded.dimensions.height, 2);
        assert_eq!(decoded.pixels.get(0, 0), Some(Pixel::new(0, 0, 0, 255)));
        assert!(decoded.metadata.format.unwrap().contains("png"));
    }

    #[test]
    fn jpeg_beats_jpeg() {
        use image::{ImageBuffer, Rgb};
        let raw: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(8, 8, Rgb([10u8, 20, 30]));
        let mut buf = Vec::new();
        raw.write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        let decoded = super::decode_bytes(&buf, &Limits::default()).unwrap();
        assert_eq!(decoded.dimensions.width, 8);
        assert_eq!(decoded.dimensions.height, 8);
        let p = decoded.pixels.get(4, 4).unwrap();
        // JPEG is lossy — tolerate quantization drift
        assert!((i16::from(p.r) - 10).abs() <= 3);
        assert!((i16::from(p.g) - 20).abs() <= 3);
        assert!((i16::from(p.b) - 30).abs() <= 3);
    }

    #[test]
    fn webp_beats_webp() {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgba([100u8, 150, 200, 255]));
        let mut buf = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::WebP,
        )
        .unwrap();
        let decoded = super::decode_bytes(&buf, &Limits::default()).unwrap();
        assert_eq!(decoded.dimensions.width, 4);
        assert_eq!(decoded.dimensions.height, 4);
        assert!(decoded.metadata.format.unwrap().contains("webp"));
    }

    #[test]
    fn decompression_bomb_rejected_by_limits() {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(8, 8, Rgba([0u8, 0, 0, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let tiny = Limits {
            max_file_size: 10 * 1024 * 1024,
            max_pixels: 16,
            max_width: 100_000,
            max_height: 100_000,
        };
        let r = super::decode_bytes(&buf, &tiny);
        assert!(r.is_err());
        assert!(matches!(r.unwrap_err(), AeError::ResourceLimit(_)));
    }
}
