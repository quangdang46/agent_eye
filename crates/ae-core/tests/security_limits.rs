use ae_core::{decode_bytes, AeError, Limits, Pixel};

use proptest::prelude::*;

fn arbitrary_bytes(max_len: usize) -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..=max_len)
}

fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([
            ((x * 17) % 256) as u8,
            ((y * 17) % 256) as u8,
            (((x + y) * 9) % 256) as u8,
            255,
        ])
    });
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("fixture encode");
    buf
}

proptest! {
    /// Property: random bytes NEVER panic; they decode to Err or Ok, nothing else.
    #[test]
    fn random_bytes_never_panic(bytes in arbitrary_bytes(4096)) {
        let _ = decode_bytes(&bytes, &Limits::default());
    }

    /// Property: truncated valid PNGs never panic.
    #[test]
    fn truncated_png_never_panic(cut in 0usize..64, seed in any::<u8>()) {
        let full = png_fixture(16, 16);
        let end = cut.min(full.len());
        let truncated = &full[..end];
        // mutate a byte to diversify corruption
        let mut corrupted = truncated.to_vec();
        if !corrupted.is_empty() {
            let last = corrupted.len() - 1;
            corrupted[last] ^= seed | 1;
        }
        let _ = decode_bytes(&corrupted, &Limits::default());
    }

    /// Property: decode is deterministic — same bytes produce identical pixels.
    #[test]
    fn decode_is_deterministic(seed in any::<u64>()) {
        use image::{ImageBuffer, Rgba};
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(
            10,
            7,
            |x, y| Rgba([(seed % 256) as u8, x as u8 * 3, y as u8 * 5, 255]),
        );
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        let limits = Limits::default();
        let a = decode_bytes(&buf, &limits).unwrap();
        let b = decode_bytes(&buf, &limits).unwrap();
        prop_assert_eq!(a.pixels.as_slice(), b.pixels.as_slice());
        prop_assert_eq!(a.dimensions, b.dimensions);
    }

    /// Property: pixel count always matches dimensions for successful decodes.
    #[test]
    fn decoded_buffer_matches_dimensions(w in 1u32..64, h in 1u32..64) {
        let bytes = png_fixture(w, h);
        let img = decode_bytes(&bytes, &Limits::default()).unwrap();
        prop_assert_eq!(img.pixels.as_slice().len(), (w as usize) * (h as usize));
    }
}

#[test]
fn luminance_always_in_unit_range() {
    // exhaustive over channel extremes; float math must stay within [0,255]
    for r in [0u8, 64, 128, 200, 255] {
        for g in [0u8, 85, 170, 255] {
            for b in [0u8, 51, 102, 255] {
                let lum = Pixel::opaque(r, g, b).luminance();
                assert!((0.0..=255.0).contains(&lum), "lum {lum} out of range");
            }
        }
    }
}

#[test]
fn tiny_limits_reject_before_decode_work() {
    let bytes = png_fixture(32, 32);
    let tiny = Limits {
        max_file_size: 4,
        ..Limits::default()
    };
    let err = decode_bytes(&bytes, &tiny).unwrap_err();
    assert!(matches!(err, AeError::ResourceLimit(_)));
}
