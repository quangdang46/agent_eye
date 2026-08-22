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
}
