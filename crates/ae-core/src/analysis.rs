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
}
