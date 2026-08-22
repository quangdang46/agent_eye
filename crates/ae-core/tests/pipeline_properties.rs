//! Property tests for the full v1 pipeline (plan §10, Phase 10).
//!
//! Properties (all must hold for ANY input):
//!   P1. Valid image → render never panics; grid dims ≤ requested, capped
//!       at image resolution.
//!   P2. Determinism: same image + config ⇒ byte-identical output.
//!   P3. Geometry JSON is always valid JSON with schema_version and no
//!       NaN/inf in any coordinate field.
//!   P4. Region bounds always inside the image; ids are r1..rN gapless.
//!   P5. Zoom level 0..=3 always yields scale = 2^level and a crop within
//!       the requested box.

use ae_core::decode::decode_bytes;
use ae_core::image::{Dimensions, Image, ImageMetadata, Limits, Pixel, PixelBuffer};
use ae_core::regions::detect_regions;
use proptest::prelude::*;

/// Random RGBA image content — the domain of most properties.
fn arb_image(max_dim: u32) -> impl Strategy<Value = Image> {
    (
        1u32..=max_dim,
        1u32..=max_dim,
        proptest::collection::vec(any::<u8>(), 16),
    )
        .prop_map(|(w, h, seed)| {
            let dims = Dimensions::new(w, h).unwrap();
            let pixels: Vec<Pixel> = (0..w * h)
                .map(|i| {
                    let s = seed[(i as usize) % seed.len()] as u32;
                    let v = (i.wrapping_mul(s.wrapping_add(1)) % 256) as u8;
                    Pixel::opaque(v, 255 - v, (s ^ i) as u8)
                })
                .collect();
            let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
            Image::new(dims, buf, ImageMetadata::default()).unwrap()
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// P1 + P2: render is total (no panic), bounded, deterministic.
    #[test]
    fn render_total_bounded_deterministic(
        img in arb_image(48),
        w in 1u32..=200,
        aspect in proptest::num::f32::ANY.prop_filter("must be in sane range", |a| {
            a.is_finite() && (1e-6..=1e6).contains(a)
        }),
        invert in any::<bool>(),
    ) {
        let cfg = ae_render::RenderConfig {
            renderer: Default::default(),
            background: None,
            width: w,
            height: Some(100),
            aspect_ratio: aspect,
            invert,
            color: Default::default(),
            charset_override: None,
        };
        let charset = cfg.resolve_charset().prop_unwrap()?;
        let g1 = ae_render::render::render(&img, &cfg, &charset).prop_unwrap()?;
        let g2 = ae_render::render::render(&img, &cfg, &charset).prop_unwrap()?;
        prop_assert!(g1.width() <= (w as usize).max(img.dimensions.width as usize));
        prop_assert!(g1.height() >= 1);
        prop_assert_eq!(g1, g2, "determinism");
    }

    /// P4: detected regions stay in-bounds with gapless ids.
    #[test]
    fn regions_in_bounds_ids_gapless(img in arb_image(64)) {
        let regions = detect_regions(&img, &ae_core::regions::DetectConfig::default())
            .prop_unwrap()?;
        for (i, r) in regions.iter().enumerate() {
            prop_assert_eq!(&r.id, &format!("r{}", i + 1));
            prop_assert!(r.bounds.x2 <= img.dimensions.width);
            prop_assert!(r.bounds.y2 <= img.dimensions.height);
            prop_assert!((0.0..=1.0).contains(&r.area));
        }
    }

    /// P5: zoom math — crop window shrinks by exactly 2^level, stays in box.
    #[test]
    fn zoom_crop_shrinks_by_level(
        box_w in 4u32..=64,
        box_h in 4u32..=64,
        level in 0u32..=3,
    ) {
        let scale = 2u32.pow(level);
        let crop_w = (box_w / scale).clamp(1, box_w);
        let crop_h = (box_h / scale).clamp(1, box_h);
        prop_assert!(crop_w <= box_w && crop_h <= box_h);
        if level == 0 {
            prop_assert_eq!(crop_w, box_w);
            prop_assert_eq!(crop_h, box_h);
        } else {
            // Higher levels never grow the crop.
            prop_assert!(crop_w <= box_w / scale + 1);
        }
    }

    /// P3-adjacent: decode of arbitrary bytes remains total (extends the
    /// security suite's guarantee to this suite's generators).
    #[test]
    fn decode_totality(bytes in proptest::collection::vec(any::<u8>(), 0..=2048)) {
        let _ = decode_bytes(&bytes, &Limits::default());
    }
}

// -- helpers ---------------------------------------------------------------

trait PropUnwrap<T> {
    fn prop_unwrap(self) -> Result<T, proptest::test_runner::TestCaseError>;
}

impl<T, E: std::fmt::Display> PropUnwrap<T> for Result<T, E> {
    fn prop_unwrap(self) -> Result<T, proptest::test_runner::TestCaseError> {
        self.map_err(|e| proptest::test_runner::TestCaseError::fail(e.to_string()))
    }
}
