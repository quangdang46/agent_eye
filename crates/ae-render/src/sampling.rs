//! Block sampling: downsample an [`Image`] into a grid of averaged blocks.
//!
//! Algorithm (plan §7): `block_w = img_w / out_w`, `block_h = block_w /
//! aspect_ratio`. Default `aspect_ratio = 0.5` because terminal cells are
//! ~2× taller than wide; `--no-aspect` maps to 1.0 (square blocks). Each
//! output block is the channel average of its source pixels — luminance is
//! derived from that average downstream, never per-pixel first.
//!
//! Unlike ASCII-generator's hardcoded `cell_height = 2 * cell_width`, the
//! height follows the configured ratio, and every block keeps its source
//! bounds so callers retain provenance back to original pixel coordinates.

use ae_core::image::{Dimensions, Image, Pixel};
use ae_core::{invalid_dimensions, rendering, Result};

/// Terminal-cell aspect correction factor.
///
/// `0.5` (default): block_h = 2 × block_w, compensating for terminal
/// characters being roughly twice as tall as wide. `1.0` (`--no-aspect`):
/// square blocks.
pub const DEFAULT_ASPECT_RATIO: f32 = 0.5;

/// One sampled cell of the output grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    /// Channel-averaged pixel of the covered source region.
    pub pixel: Pixel,
    /// Half-open source-pixel bounds `[x, y, x2, y2)` this block averaged.
    pub bounds: [u32; 4],
}

impl Block {
    /// Rec. 709 analysis luminance in `[0, 255]`.
    pub fn luminance(&self) -> f32 {
        self.pixel.luminance()
    }
}

/// Validate a caller-supplied aspect ratio.
///
/// Rejects non-finite and non-positive values up front — NaN would otherwise
/// poison every downstream block dimension silently.
pub fn validate_aspect_ratio(aspect_ratio: f32) -> Result<()> {
    if !aspect_ratio.is_finite() || aspect_ratio <= 0.0 {
        return Err(rendering(format!(
            "aspect_ratio must be finite and > 0, got {aspect_ratio}"
        )));
    }
    Ok(())
}

/// Per-axis block size in source pixels for one dimension.
///
/// `src / out` rounded up so every source pixel lands in some block; blocks
/// may overlap by one pixel at edges when division is inexact (same choice
/// pixel2ascii makes). Returns error when either side is zero.
fn block_extent(src: u32, out: u32) -> Result<u32> {
    if src == 0 || out == 0 {
        return Err(invalid_dimensions(format!(
            "cannot sample {src} px into {out} columns"
        )));
    }
    Ok(src.div_ceil(out))
}

/// Samples `img` into an `out_w × out_h` grid of averaged [`Block`]s.
///
/// Determinism: pure integer arithmetic over the canonical buffer; identical
/// input + config ⇒ identical output. Output row-major, `blocks[y *
/// out_w + x]`.
pub fn sample_blocks(img: &Image, out_w: u32, out_h: u32, aspect_ratio: f32) -> Result<Vec<Block>> {
    validate_aspect_ratio(aspect_ratio)?;
    if out_w == 0 || out_h == 0 {
        return Err(invalid_dimensions(format!(
            "output grid must be non-empty, got {out_w}x{out_h}"
        )));
    }
    let Dimensions { width, height } = img.dimensions;
    // Cap the grid at image resolution: upsampling adds no information and
    // would produce single-pixel blocks anyway.
    let out_w = out_w.min(width);
    let out_h = out_h.min(height);

    let block_w = block_extent(width, out_w)?;
    let block_h_f = block_w as f32 / aspect_ratio;
    debug_assert!(block_h_f.is_finite());
    let block_h = (block_h_f.ceil().max(1.0)) as u32;

    let stride = width as usize;
    let buf = img.pixels.as_slice();
    let mut blocks = Vec::with_capacity(out_w as usize * out_h as usize);

    for oy in 0..out_h {
        let sy0 = (oy as usize * block_h as usize).min(height as usize - 1);
        let sy1 = (((oy + 1) as usize * block_h as usize).min(height as usize)).max(sy0 + 1);
        for ox in 0..out_w {
            let sx0 = (ox as usize * block_w as usize).min(width as usize - 1);
            let sx1 = (((ox + 1) as usize * block_w as usize).min(width as usize)).max(sx0 + 1);
            let mut sum = PixelAvg::default();
            let mut count = 0u64;
            for y in sy0..sy1.min(height as usize) {
                let row = &buf[y * stride..(y + 1) * stride];
                for p in &row[sx0..sx1] {
                    sum.add(p);
                    count += 1;
                }
            }
            blocks.push(Block {
                pixel: sum.finish(count),
                bounds: [sx0 as u32, sy0 as u32, sx1 as u32, sy1 as u32],
            });
        }
    }
    Ok(blocks)
}

/// Running channel-sum accumulator (u64 cannot overflow: ≤25M pixels × 255).
#[derive(Default)]
struct PixelAvg {
    r: u64,
    g: u64,
    b: u64,
    a: u64,
}

impl PixelAvg {
    fn add(&mut self, p: &Pixel) {
        self.r += u64::from(p.r);
        self.g += u64::from(p.g);
        self.b += u64::from(p.b);
        self.a += u64::from(p.a);
    }

    fn finish(self, n: u64) -> Pixel {
        Pixel::new(
            (self.r / n) as u8,
            (self.g / n) as u8,
            (self.b / n) as u8,
            (self.a / n) as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_core::image::{ImageMetadata, PixelBuffer};

    fn solid_img(w: u32, h: u32, p: Pixel) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let buf = PixelBuffer::from_vec(dims, vec![p; (w * h) as usize]).unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    fn gradient_img(w: u32, h: u32) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let pixels: Vec<Pixel> = (0..w * h)
            .map(|i| {
                let v = (i % 256) as u8;
                Pixel::opaque(v, v, v)
            })
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    #[test]
    fn default_aspect_is_half() {
        assert_eq!(DEFAULT_ASPECT_RATIO, 0.5);
    }

    #[test]
    fn rejects_zero_output_grid_and_bad_aspect() {
        let img = solid_img(4, 4, Pixel::opaque(0, 0, 0));
        assert!(sample_blocks(&img, 0, 2, 0.5).is_err());
        assert!(sample_blocks(&img, 2, 0, 0.5).is_err());
        assert!(sample_blocks(&img, 2, 2, 0.0).is_err());
        assert!(sample_blocks(&img, 2, 2, -1.0).is_err());
        assert!(sample_blocks(&img, 2, 2, f32::NAN).is_err());
        assert!(validate_aspect_ratio(f32::INFINITY).is_err());
        assert!(validate_aspect_ratio(1.0).is_ok());
    }

    #[test]
    fn one_by_one_image_single_block() {
        let img = solid_img(1, 1, Pixel::opaque(10, 20, 30));
        let blocks = sample_blocks(&img, 100, 50, DEFAULT_ASPECT_RATIO).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].pixel, Pixel::opaque(10, 20, 30));
        assert_eq!(blocks[0].bounds, [0, 0, 1, 1]);
    }

    #[test]
    fn solid_image_yields_solid_blocks() {
        let img = solid_img(64, 64, Pixel::opaque(200, 100, 50));
        let blocks = sample_blocks(&img, 8, 8, DEFAULT_ASPECT_RATIO).unwrap();
        assert_eq!(blocks.len(), 64);
        for b in &blocks {
            assert_eq!(b.pixel, Pixel::opaque(200, 100, 50));
        }
    }

    #[test]
    fn very_wide_image_downsamples_all_columns() {
        let img = gradient_img(1000, 2);
        let blocks = sample_blocks(&img, 100, 1, 1.0).unwrap();
        assert_eq!(blocks.len(), 100);
        // Bounds tile the full width without gaps.
        assert_eq!(blocks[0].bounds[0], 0);
        assert_eq!(blocks[99].bounds[2], 1000);
        for w in blocks.windows(2) {
            assert_eq!(w[0].bounds[2], w[1].bounds[0]);
        }
    }

    #[test]
    fn very_narrow_image_upsamples_capped_at_resolution() {
        let img = solid_img(3, 9, Pixel::opaque(7, 7, 7));
        // Asking for 10 columns caps to 3 — no fabricated detail.
        let blocks = sample_blocks(&img, 10, 20, 1.0).unwrap();
        assert_eq!(blocks.len(), 3 * 9);
        assert!(blocks.iter().all(|b| b.pixel == Pixel::opaque(7, 7, 7)));
    }

    #[test]
    fn output_grid_is_exact_when_requested() {
        // Grid is exactly out_w × min(out_h, height) blocks; block_h follows
        // the aspect ratio (block_w / aspect), rows clamp at image height.
        let img = solid_img(40, 40, Pixel::opaque(1, 2, 3));
        // block_w=4; aspect 0.5 ⇒ block_h=8 → covers height in ceil(40/8)=5
        // effective rows even though 20 requested.
        let tall = sample_blocks(&img, 10, 20, 0.5).unwrap();
        assert_eq!(tall[0].bounds, [0, 0, 4, 8]);
        assert_eq!(tall.last().unwrap().pixel, Pixel::opaque(1, 2, 3));
        let square = sample_blocks(&img, 10, 10, 1.0).unwrap(); // block_h=4 → 10 rows
        assert_eq!(square.last().unwrap().bounds, [36, 36, 40, 40]);
    }
    #[test]
    fn averaging_matches_channel_mean() {
        // Two-pixel image with known mean.
        let dims = Dimensions::new(2, 1).unwrap();
        let buf = PixelBuffer::from_vec(
            dims,
            vec![Pixel::opaque(0, 0, 0), Pixel::opaque(10, 20, 30)],
        )
        .unwrap();
        let img = Image::new(dims, buf, ImageMetadata::default()).unwrap();
        let blocks = sample_blocks(&img, 1, 1, 0.5).unwrap();
        assert_eq!(blocks[0].pixel, Pixel::opaque(5, 10, 15));
    }

    #[test]
    fn average_pixel_agreement_for_rect_block() {
        // Cross-check against ae-core's independent averaging helper.
        let img = gradient_img(16, 16);
        let blocks = sample_blocks(&img, 2, 2, 0.5).unwrap();
        let b = &blocks[1]; // top-right quadrant-ish block
        let (x0, y0, x1, y1) = (
            b.bounds[0] as usize,
            b.bounds[1] as usize,
            b.bounds[2] as usize,
            b.bounds[3] as usize,
        );
        let mut region = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                region.push(img.pixels.get(x as u32, y as u32).unwrap());
            }
        }
        let expected = ae_core::analysis::average_pixel(&region, region.len()).unwrap();
        assert_eq!(b.pixel, expected);
    }

    #[test]
    fn deterministic_repeat_calls() {
        let img = gradient_img(37, 23);
        let a = sample_blocks(&img, 11, 7, 0.5).unwrap();
        let b = sample_blocks(&img, 11, 7, 0.5).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn luminance_delegates_to_rec709() {
        let b = Block {
            pixel: Pixel::opaque(255, 255, 255),
            bounds: [0, 0, 1, 1],
        };
        assert!((b.luminance() - 255.0).abs() < 1e-4);
    }
}
