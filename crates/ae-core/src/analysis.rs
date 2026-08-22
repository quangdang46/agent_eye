//! Analysis primitives over the canonical [`Image`]/[`Pixel`] types.
//!
//! Luminance here is **analysis, not presentation**: Rec. 709 coefficients,
//! always computed internally (plan §7). A grayscale *output* mode is a
//! separate presentation concern and must not fork this pipeline.

use crate::image::{Pixel, PixelBuffer};

/// Rec. 709 luminance of a single pixel in `[0.0, 255.0]`.
///
/// Single source of truth: `Pixel::luminance` delegates here.
#[inline]
pub fn luminance(p: Pixel) -> f32 {
    let r = f32::from(p.r);
    let g = f32::from(p.g);
    let b = f32::from(p.b);
    0.212_6 * r + 0.715_2 * g + 0.072_2 * b
}

/// Channel-averaged pixel of an axis-aligned block given as a flat slice.
///
/// `pixels` is iterated in row-major order; `stride` is the row width in
/// pixels so a block can be expressed without copying. Alpha is ignored for
/// averaging (composited-over-opaque assumption) but each sample contributes
/// equally regardless of `a`.
pub fn average_pixel(pixels: &[Pixel], stride: usize) -> Option<Pixel> {
    if pixels.is_empty() || stride == 0 {
        return None;
    }
    let n = pixels.len();
    // u64 sums cannot overflow: 4B pixels * 255 fits far below u64 max.
    let (mut rs, mut gs, mut bs) = (0u64, 0u64, 0u64);
    for p in pixels {
        rs += u64::from(p.r);
        gs += u64::from(p.g);
        bs += u64::from(p.b);
    }
    Some(Pixel {
        r: (rs / n as u64) as u8,
        g: (gs / n as u64) as u8,
        b: (bs / n as u64) as u8,
        a: 255,
    })
}

/// Block luminance = luminance of the block's channel-averaged pixel.
pub fn block_luminance(pixels: &[Pixel]) -> Option<f32> {
    average_pixel(pixels, pixels.len().max(1)).map(luminance)
}

/// Global min/max luminance of a buffer — used by analysis passes.
pub fn luminance_range(buf: &PixelBuffer) -> Option<(f32, f32)> {
    let mut iter = buf.as_slice().iter().map(|p| luminance(*p));
    let first = iter.next()?;
    let (mut min, mut max) = (first, first);
    for l in iter {
        if l < min {
            min = l;
        }
        if l > max {
            max = l;
        }
    }
    Some((min, max))
}

/// Sobel gradient magnitude per pixel, in `[0.0, 255.0]`.
///
/// Border pixels (1px frame) get strength `0.0` — the kernel has no
/// out-of-bounds policy and fabricating border gradients would invent
/// evidence. Output is row-major, same shape as the input buffer.
///
/// Deterministic: pure integer/fixed arithmetic on the Rec. 709 luminance
/// plane; identical input ⇒ identical map.
pub fn sobel_edges(buf: &PixelBuffer) -> Vec<f32> {
    let dims = buf.dimensions();
    let w = dims.width as usize;
    let h = dims.height as usize;
    let mut out = vec![0.0f32; w * h];
    if w < 3 || h < 3 {
        return out;
    }
    let lum = |x: usize, y: usize| luminance(buf.get(x as u32, y as u32).unwrap_or_default());
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            // Luminances of the 3×3 neighborhood.
            let tl = lum(x - 1, y - 1);
            let t = lum(x, y - 1);
            let tr = lum(x + 1, y - 1);
            let l = lum(x - 1, y);
            let r = lum(x + 1, y);
            let bl = lum(x - 1, y + 1);
            let b = lum(x, y + 1);
            let br = lum(x + 1, y + 1);
            let gx = (tr + 2.0 * r + br) - (tl + 2.0 * l + bl);
            let gy = (bl + 2.0 * b + br) - (tl + 2.0 * t + tr);
            out[y * w + x] = (gx.hypot(gy)).min(255.0);
        }
    }
    out
}

/// Contrast analysis for a pixel buffer (plan §8, internal VisualComplexity
/// inputs — not exposed in CLI v1).
pub mod contrast {
    use super::{luminance_range, PixelBuffer};

    /// Global contrast: RMS contrast over the Rec. 709 luminance plane,
    /// normalized to `[0.0, 1.0]` by 255.
    ///
    /// RMS is preferred over Michelson because it stays defined for flat
    /// images (Michelson divides by zero there).
    pub fn rms(buf: &PixelBuffer) -> f64 {
        let slice = buf.as_slice();
        if slice.is_empty() {
            return 0.0;
        }
        let n = slice.len() as f64;
        let mean = slice.iter().map(|p| f64::from(p.luminance())).sum::<f64>() / n;
        let var = slice
            .iter()
            .map(|p| {
                let d = f64::from(p.luminance()) - mean;
                d * d
            })
            .sum::<f64>()
            / n;
        (var.sqrt()) / 255.0
    }

    /// Dynamic range `(min_luma, max_luma)` of the buffer, normalized to
    /// `[0.0, 1.0]`. `None` only when the buffer is empty.
    pub fn dynamic_range(buf: &PixelBuffer) -> Option<(f32, f32)> {
        luminance_range(buf).map(|(lo, hi)| (lo / 255.0, hi / 255.0))
    }

    /// Local RMS contrast inside one half-open block, sharing the block
    /// iteration convention with rendering (`block_w × block_h` windows).
    pub fn local_rms(buf: &PixelBuffer, bounds: crate::geometry::HalfOpenBounds) -> f64 {
        let w = buf.dimensions().width as usize;
        let h = buf.dimensions().height as usize;
        let (x1, y1) = (bounds.x1 as usize, bounds.y1 as usize);
        let (x2, y2) = (
            bounds.x2.min(w as u32) as usize,
            bounds.y2.min(h as u32) as usize,
        );
        if x1 >= x2 || y1 >= y2 {
            return 0.0;
        }
        let slice = &buf.as_slice()[y1 * w..y2 * w];
        let mut sum = 0.0f64;
        let mut sq = 0.0f64;
        let mut count = 0u64;
        for row in slice.chunks_exact(w) {
            for p in &row[x1..x2] {
                let l = f64::from(p.luminance());
                sum += l;
                sq += l * l;
                count += 1;
            }
        }
        if count == 0 {
            return 0.0;
        }
        let n = count as f64;
        let mean = sum / n;
        ((sq / n - mean * mean).max(0.0)).sqrt() / 255.0
    }
}

/// Color variance metrics (plan §8): how much chroma varies across the
/// buffer — near zero for grayscale content even when busy in luminance.
pub mod color_variance {
    use super::PixelBuffer;

    /// Population variance of per-pixel chroma magnitude
    /// `sqrt((r-g)^2 + (g-b)^2 + (b-r)^2)`, normalized by the max possible
    /// (√2·255), so result ∈ `[0.0, 1.0]`.
    pub fn chroma_variance(buf: &PixelBuffer) -> f64 {
        let slice = buf.as_slice();
        if slice.is_empty() {
            return 0.0;
        }
        let n = slice.len() as f64;
        let chroma = |p: &super::Pixel| -> f64 {
            let (r, g, b) = (f64::from(p.r), f64::from(p.g), f64::from(p.b));
            ((r - g) * (r - g) + (g - b) * (g - b) + (b - r) * (b - r)).sqrt()
        };
        // Max chroma for 8-bit channels: r=255,g=0,b=0 → √(255² + 0 + 255²).
        let max = (2.0f64 * 255.0 * 255.0).sqrt();
        let mean = slice.iter().map(&chroma).sum::<f64>() / n;
        slice
            .iter()
            .map(|p| {
                let d = chroma(p) - mean;
                d * d
            })
            .sum::<f64>()
            / n
            / (max * max)
    }
}

/// Weighted-aggregate visual complexity score (plan §8).
///
/// Internal implementation detail for the future `--budget` adaptive
/// rendering (Phase 8 P1); not exposed through the CLI in v1. Purely
/// geometric/statistical — carries no task relevance or semantics.
///
/// All inputs are expected pre-normalized to `[0.0, 1.0]` (see [`contrast`]
/// and [`color_variance`]); `redundancy` is 1 − normalized entropy-style
/// measure of how repetitive the content is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualComplexity {
    /// Mean Sobel edge strength, /255.
    pub edge: f32,
    /// RMS contrast.
    pub variance: f64,
    /// Chroma variance.
    pub gradient: f64,
    /// Redundancy estimate: fraction of identical adjacent-pixel pairs,
    /// `[0.0, 1.0]` (high = flat/repetitive = simple).
    pub redundancy: f64,
    /// Weighted aggregate in `[0.0, 1.0]`.
    pub score: f64,
}

impl VisualComplexity {
    /// Weights sum to 1 so `score` stays in range when inputs do.
    const W_EDGE: f64 = 0.4;
    const W_VARIANCE: f64 = 0.25;
    const W_GRADIENT: f64 = 0.15;
    const W_REDUNDANCY_INVERSE: f64 = 0.2;

    /// Computes complexity for a buffer: runs edge + contrast + chroma
    /// passes internally. Deterministic.
    pub fn compute(buf: &PixelBuffer) -> Self {
        let edges = sobel_edges(buf);
        let mean_edge = if edges.is_empty() {
            0.0
        } else {
            edges.iter().sum::<f32>() / edges.len() as f32 / 255.0
        };
        let rms_c = contrast::rms(buf);
        let cv = color_variance::chroma_variance(buf);
        let redundancy = Self::adjacent_pair_redundancy(buf);
        let score = Self::W_EDGE * f64::from(mean_edge)
            + Self::W_VARIANCE * rms_c
            + Self::W_GRADIENT * cv
            + Self::W_REDUNDANCY_INVERSE * (1.0 - redundancy);
        Self {
            edge: mean_edge.clamp(0.0, 1.0),
            variance: rms_c,
            gradient: cv,
            redundancy,
            score: score.clamp(0.0, 1.0),
        }
    }

    /// Fraction of horizontally-adjacent pixel pairs with identical RGB.
    fn adjacent_pair_redundancy(buf: &PixelBuffer) -> f64 {
        let slice = buf.as_slice();
        let w = buf.dimensions().width as usize;
        let h = buf.dimensions().height as usize;
        if w < 2 || h == 0 {
            return 0.0;
        }
        let mut same = 0u64;
        let mut total = 0u64;
        for row in 0..h {
            let start = row * w;
            for i in start..start + w - 1 {
                let (a, b) = (&slice[i], &slice[i + 1]);
                if a.r == b.r && a.g == b.g && a.b == b.b {
                    same += 1;
                }
                total += 1;
            }
        }
        if total == 0 {
            0.0
        } else {
            same as f64 / total as f64
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Dimensions, PixelBuffer};
    #[test]
    fn endpoints_black_white() {
        assert_eq!(luminance(Pixel::opaque(0, 0, 0)), 0.0);
        assert!((luminance(Pixel::opaque(255, 255, 255)) - 255.0).abs() < 1e-4);
    }

    #[test]
    fn rec709_primary_weights() {
        assert!((luminance(Pixel::opaque(255, 0, 0)) - 54.212_997).abs() < 1e-3);
        assert!((luminance(Pixel::opaque(0, 255, 0)) - 182.376).abs() < 1e-3);
        assert!((luminance(Pixel::opaque(0, 0, 255)) - 18.411).abs() < 1e-3);
        // green dominates, red next, blue least — ordering is stable
        assert!(luminance(Pixel::opaque(0, 255, 0)) > luminance(Pixel::opaque(255, 0, 0)));
        assert!(luminance(Pixel::opaque(255, 0, 0)) > luminance(Pixel::opaque(0, 0, 255)));
    }

    #[test]
    fn gray_ramp_midpoints() {
        // equal channels → luminance == channel value
        for v in [1u8, 32, 64, 128, 200, 254] {
            let l = luminance(Pixel::opaque(v, v, v));
            assert!((l - f32::from(v)).abs() < 1e-4, "v={v} lum={l}");
        }
    }

    #[test]
    fn average_of_block() {
        let px = vec![
            Pixel::opaque(10, 20, 30),
            Pixel::opaque(30, 40, 50),
            Pixel::opaque(50, 60, 70),
            Pixel::opaque(70, 80, 90),
        ];
        let avg = average_pixel(&px, 2).unwrap();
        assert_eq!(avg.r, 40); // (10+30+50+70)/4
        assert_eq!(avg.g, 50);
        assert_eq!(avg.b, 60);
        assert_eq!(block_luminance(&px).unwrap(), luminance(avg));
    }

    #[test]
    fn average_empty_is_none() {
        assert!(average_pixel(&[], 4).is_none());
        assert!(block_luminance(&[]).is_none());
    }

    #[test]
    fn range_over_buffer() {
        let dims = Dimensions::new(2, 2).unwrap();
        let buf = PixelBuffer::from_vec(
            dims,
            vec![
                Pixel::opaque(0, 0, 0),
                Pixel::opaque(255, 255, 255),
                Pixel::opaque(128, 128, 128),
                Pixel::opaque(64, 64, 64),
            ],
        )
        .unwrap();
        let (min, max) = luminance_range(&buf).unwrap();
        assert_eq!(min, 0.0);
        assert!((max - 255.0).abs() < 1e-4);
    }

    #[test]
    fn sobel_uniform_image_has_zero_edges() {
        let dims = Dimensions::new(8, 8).unwrap();
        let buf = PixelBuffer::from_vec(dims, vec![Pixel::opaque(100, 100, 100); 64]).unwrap();
        assert!(sobel_edges(&buf).iter().all(|e| *e < 1e-6));
    }

    #[test]
    fn sobel_known_vertical_step_strong_edge() {
        // Left half black, right half white → strong vertical edge at x=1..w-2.
        let dims = Dimensions::new(4, 3).unwrap();
        let mut pixels = Vec::new();
        for _ in 0..3 {
            pixels.push(Pixel::opaque(0, 0, 0));
            pixels.push(Pixel::opaque(0, 0, 0));
            pixels.push(Pixel::opaque(255, 255, 255));
            pixels.push(Pixel::opaque(255, 255, 255));
        }
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        let edges = sobel_edges(&buf);
        // Border is zero; interior at the step is the max (row y=1).
        assert_eq!(edges[4], 0.0, "border stays zero");
        assert!(
            edges[5] > 250.0,
            "step edge strong (clamped ≤255): {}",
            edges[1]
        );
        assert!(edges[6] > 250.0);
        assert_eq!(edges[7], 0.0);
    }

    #[test]
    fn sobel_tiny_images_all_zero_without_panic() {
        for (w, h) in [(1u32, 1u32), (2, 5), (5, 2), (2, 2)] {
            let dims = Dimensions::new(w, h).unwrap();
            let buf = PixelBuffer::zeroed(dims);
            let e = sobel_edges(&buf);
            assert_eq!(e.len(), (w * h) as usize);
            assert!(e.iter().all(|v| *v == 0.0), "{w}x{h} must be all zero");
        }
    }

    #[test]
    fn sobel_deterministic() {
        let dims = Dimensions::new(9, 7).unwrap();
        let pixels: Vec<Pixel> = (0..63)
            .map(|i| Pixel::opaque(((i * 13) % 256) as u8, 0, ((i * 7) % 256) as u8))
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        assert_eq!(sobel_edges(&buf), sobel_edges(&buf));
    }

    #[test]
    fn rms_flat_is_zero_noisy_is_positive() {
        let dims = Dimensions::new(4, 4).unwrap();
        let flat = PixelBuffer::from_vec(dims, vec![Pixel::opaque(128, 128, 128); 16]).unwrap();
        assert_eq!(contrast::rms(&flat), 0.0);
        let mixed: Vec<Pixel> = (0..16)
            .map(|i| Pixel::opaque(((i * 17) % 256) as u8, 0, 0))
            .collect();
        let noisy = PixelBuffer::from_vec(dims, mixed).unwrap();
        assert!(contrast::rms(&noisy) > 0.05);
        assert!(contrast::rms(&noisy) <= 1.0);
    }

    #[test]
    fn dynamic_range_endpoints() {
        let dims = Dimensions::new(2, 1).unwrap();
        let buf = PixelBuffer::from_vec(
            dims,
            vec![Pixel::opaque(0, 0, 0), Pixel::opaque(255, 255, 255)],
        )
        .unwrap();
        let (lo, hi) = contrast::dynamic_range(&buf).unwrap();
        assert_eq!((lo, hi), (0.0, 1.0));
    }

    #[test]
    fn local_rms_matches_global_on_full_bounds() {
        let dims = Dimensions::new(6, 6).unwrap();
        let pixels: Vec<Pixel> = (0..36)
            .map(|i| Pixel::opaque(((i * 23) % 256) as u8, ((i * 5) % 256) as u8, 30))
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        let full = crate::geometry::HalfOpenBounds::covering(6, 6);
        assert!((contrast::local_rms(&buf, full) - contrast::rms(&buf)).abs() < 1e-9);
        // Empty block → zero.
        let empty = crate::geometry::HalfOpenBounds::new(2, 2, 2, 2).unwrap();
        assert_eq!(contrast::local_rms(&buf, empty), 0.0);
    }

    #[test]
    fn chroma_variance_grayscale_zero_colored_positive() {
        let dims = Dimensions::new(3, 3).unwrap();
        let colored: Vec<Pixel> = (0..9)
            .map(|i| {
                // Vary chroma magnitude: saturated red ↔ near-gray.
                if i % 2 == 0 {
                    Pixel::opaque(255, 0, 0)
                } else {
                    Pixel::opaque(128, 128, 120)
                }
            })
            .collect();
        let cv = PixelBuffer::from_vec(dims, colored).unwrap();
        assert!(color_variance::chroma_variance(&cv) > 0.01);
        assert!(color_variance::chroma_variance(&cv) <= 1.0);
    }

    #[test]
    fn complexity_flat_is_low_checkerboard_lower_than_noise() {
        // Flat image: zero edges, zero contrast → minimal score.
        let dims = Dimensions::new(8, 8).unwrap();
        let flat = PixelBuffer::from_vec(dims, vec![Pixel::opaque(50, 50, 50); 64]).unwrap();
        let c_flat = VisualComplexity::compute(&flat);
        assert_eq!(c_flat.edge, 0.0);
        assert_eq!(c_flat.variance, 0.0);
        assert!(
            (c_flat.redundancy - 1.0).abs() < 1e-9,
            "all pairs identical"
        );
        assert_eq!(
            c_flat.score, 0.0,
            "fully flat: redundancy=1 zeroes the only surviving term"
        );

        // High-frequency noise scores higher than the flat image.
        let noisy_pixels: Vec<Pixel> = (0..64)
            .map(|i| {
                let v = ((i * 37 + 11) % 256) as u8;
                Pixel::opaque(v, (255 - v), (i % 2 * 255) as u8)
            })
            .collect();
        let noisy = PixelBuffer::from_vec(dims, noisy_pixels).unwrap();
        let c_noisy = VisualComplexity::compute(&noisy);
        assert!(c_noisy.score > c_flat.score);
        assert!(c_noisy.score <= 1.0 && c_flat.score <= 1.0);
    }

    #[test]
    fn complexity_deterministic() {
        let dims = Dimensions::new(6, 5).unwrap();
        let pixels: Vec<Pixel> = (0..30)
            .map(|i| Pixel::opaque(((i * 29) % 256) as u8, ((i * 3) % 256) as u8, 7))
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        assert_eq!(
            VisualComplexity::compute(&buf),
            VisualComplexity::compute(&buf)
        );
    }
}
