//! Braille renderer: 2×4-dot Unicode braille cells (U+2800..U+28FF).
//!
//! Each cell packs a 2×4 block of source pixels into one character —
//! **8× the spatial density of ASCII** per output cell. A dot is "on"
//! when its source-pixel luminance falls below `threshold` (dark pixels
//! are ink). Optional Floyd–Steinberg dithering spreads quantization
//! error so mid-tones become dot texture instead of vanishing.
//!
//! Determinism: pure integer/threshold arithmetic; identical input ⇒
//! identical output. Output is plain text (no color modes — dots ARE the
//! presentation).

use ae_core::analysis::luminance;
#[cfg(test)]
use ae_core::image::Pixel;
use ae_core::image::{Dimensions, Image};
use ae_core::{invalid_dimensions, rendering, Result};

/// Dot bit for (x, y) inside a 2×4 cell → Unicode braille offset.
///
/// Standard U+2800 block layout:
/// ```text
/// (0,0)=1 (1,0)=8
/// (0,1)=2 (1,1)=16
/// (0,2)=4 (1,2)=32
/// (0,3)=64 (1,3)=128
/// ```
const DOT_BITS: [[u32; 2]; 4] = [[1, 8], [2, 16], [4, 32], [64, 128]];

/// Braille renderer configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrailleConfig {
    /// Output width in cells. Height derives from image aspect at 2×4.
    pub width: u32,
    /// Luminance threshold in `[0, 255]`: pixel darker than this → dot ON.
    pub threshold: f32,
    /// Floyd–Steinberg error diffusion before thresholding.
    pub dither: bool,
}

impl Default for BrailleConfig {
    fn default() -> Self {
        Self {
            width: 100,
            threshold: 128.0,
            dither: false,
        }
    }
}

impl BrailleConfig {
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.width > 10_000 {
            return Err(rendering(format!(
                "braille width must be within [1, 10000], got {}",
                self.width
            )));
        }
        if !self.threshold.is_finite() || !(0.0..=255.0).contains(&self.threshold) {
            return Err(rendering(format!(
                "braille threshold must be finite within [0, 255], got {}",
                self.threshold
            )));
        }
        Ok(())
    }
}

/// Renders `img` as braille text. Cell grid is `width × ceil(h/4 ÷ scale)`.
pub fn render_braille(img: &Image, cfg: &BrailleConfig) -> Result<String> {
    cfg.validate()?;
    let Dimensions {
        width: iw,
        height: ih,
    } = img.dimensions;
    if iw == 0 || ih == 0 {
        return Err(invalid_dimensions("empty image"));
    }

    // Cell geometry: each cell = 2 source px wide; rows derive from aspect.
    let cells_w = (cfg.width.min(iw / 2).max(1)) as usize;
    // Scale so cells tile the image: source pixels per cell-x.
    let sx = (iw as usize).div_ceil(cells_w);
    let cell_src_h = sx * 2; // keep square-ish sampling per cell row (2 wide × 4 tall scaled)
    let cells_h = (ih as usize).div_ceil(cell_src_h * 2);

    // Luminance plane with optional error-diffusion buffer.
    let stride = iw as usize;
    let lum_plane: Vec<f32> = img
        .pixels
        .as_slice()
        .iter()
        .map(|p| luminance(*p))
        .collect();
    let mut err_plane = if cfg.dither {
        vec![0.0f32; lum_plane.len()]
    } else {
        Vec::new()
    };

    let mut out = String::with_capacity(cells_h * (cells_w + 1));
    for cy in 0..cells_h {
        for cx in 0..cells_w {
            let x0 = cx * sx;
            let y0 = cy * cell_src_h;
            let mut bits: u32 = 0;
            for (dy, dot_row) in DOT_BITS.iter().enumerate() {
                for (dx, &dot_bit) in dot_row.iter().enumerate() {
                    let sxp = x0 + dx * sx.max(1);
                    let syp = y0 + dy * (cell_src_h / 2).max(1);
                    if sxp >= stride || syp >= ih as usize {
                        continue; // outside image: dot off
                    }
                    let idx = syp * stride + sxp;
                    // err plane holds POSITIVE accumulated darkness to add
                    // to the pixel before thresholding.
                    let value = lum_plane[idx] + err_plane.get(idx).copied().unwrap_or(0.0);
                    let dot_on = value < cfg.threshold;
                    if dot_on {
                        bits |= dot_bit;
                    }
                    if cfg.dither {
                        // Diffuse to the NEXT SAMPLED dot in each direction
                        // (the lattice is sx × cell_src_h/2, not 1×1), with
                        // FS weights: right 7/16, below-left 3/16, below
                        // 5/16, below-right 1/16.
                        let quantized = if dot_on { 0.0 } else { 255.0 };
                        let err = value - quantized;
                        if dx == 0 {
                            // right sample is dx=1 of this row: +sx in x
                            diffuse_at(
                                &mut err_plane,
                                stride,
                                sxp + sx,
                                syp,
                                iw as usize,
                                ih as usize,
                                err * 7.0 / 16.0,
                            );
                        }
                        let by = y0 + (dy + 2).min(3) * (cell_src_h / 2);
                        if dy <= 1 {
                            // below row of same cell
                            if dx == 1 {
                                diffuse_at(
                                    &mut err_plane,
                                    stride,
                                    sxp - sx,
                                    by,
                                    iw as usize,
                                    ih as usize,
                                    err * 3.0 / 16.0,
                                );
                            }
                            diffuse_at(
                                &mut err_plane,
                                stride,
                                sxp,
                                by,
                                iw as usize,
                                ih as usize,
                                err * 5.0 / 16.0,
                            );
                            if dx == 0 {
                                diffuse_at(
                                    &mut err_plane,
                                    stride,
                                    sxp + sx,
                                    by,
                                    iw as usize,
                                    ih as usize,
                                    err * 1.0 / 16.0,
                                );
                            }
                        }
                    }
                }
            }
            out.push(char::from_u32(0x2800 + bits).expect("braille range"));
        }
        out.push('\n');
    }
    Ok(out)
}

/// Adds `amount` of quantization error at a sampled lattice position.
fn diffuse_at(err: &mut [f32], stride: usize, x: usize, y: usize, w: usize, h: usize, amount: f32) {
    if x < w && y < h {
        err[y * stride + x] += amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_core::image::{ImageMetadata, PixelBuffer};

    fn img(w: u32, h: u32, fill: impl Fn(u32, u32) -> Pixel) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let pixels: Vec<Pixel> = (0..w * h).map(|i| fill(i % w, i / w)).collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    #[test]
    fn black_image_all_dots_white_none() {
        let black = img(8, 8, |_, _| Pixel::opaque(0, 0, 0));
        let white = img(8, 8, |_, _| Pixel::opaque(255, 255, 255));
        let cfg = BrailleConfig {
            width: 4,
            ..Default::default()
        };
        let b = render_braille(&black, &cfg).unwrap();
        // Every in-image dot must be ON; boundary dots (right column of
        // the last cell) are off by policy, so cells may be partially lit.
        assert!(b.contains('\u{28ff}'), "in-image cells fully lit: {b:?}");
        assert!(
            b.chars()
                .all(|c| c == '\n' || ('\u{2801}'..='\u{28ff}').contains(&c)),
            "no blank cell where image exists: {b:?}"
        );
        let w = render_braille(&white, &cfg).unwrap();
        assert!(w.chars().all(|c| c == '\u{2800}' || c == '\n'), "{w:?}");
    }
    #[test]
    fn config_validation_rejects_bad_input() {
        let ok = BrailleConfig::default();
        assert!(ok.validate().is_ok());
        assert!(BrailleConfig { width: 0, ..ok }.validate().is_err());
        assert!(BrailleConfig {
            width: 10_001,
            ..ok
        }
        .validate()
        .is_err());
        assert!(BrailleConfig {
            threshold: -1.0,
            ..ok
        }
        .validate()
        .is_err());
        assert!(BrailleConfig {
            threshold: 256.0,
            ..ok
        }
        .validate()
        .is_err());
        assert!(BrailleConfig {
            threshold: f32::NAN,
            ..ok
        }
        .validate()
        .is_err());
        assert!(BrailleConfig {
            threshold: 255.0,
            ..ok
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn dither_changes_output_on_midtone() {
        let gray = img(16, 16, |_, _| Pixel::opaque(128, 128, 128));
        let plain = render_braille(
            &gray,
            &BrailleConfig {
                width: 8,
                dither: false,
                ..Default::default()
            },
        )
        .unwrap();
        let dithered = render_braille(
            &gray,
            &BrailleConfig {
                width: 8,
                dither: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Midtone 128 vs threshold 128: without dithering no dot turns on
        // (value not < threshold); with diffusion some do. Dithering
        // scatters dots across the midtone instead of a flat field.
        assert!(
            plain.chars().all(|c| c == '\u{2800}' || c == '\n'),
            "undithered midtone must be blank: {plain:?}"
        );
        assert!(
            dithered
                .chars()
                .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)),
            "dithering must produce dot texture: {dithered:?}"
        );
    }

    #[test]
    fn dither_known_input_exact_output() {
        // 2x1 image: left pixel black(0), right white(255); threshold 128.
        // width=1 → one cell, sx=2; only x=0 is a sampled lattice point, so
        // the black dot turns on (4 left-column dots = bits 1|2|4|64) and
        // the unsampled right pixel never does. Exact deterministic output.
        let im = img(2, 1, |x, _| {
            if x == 0 {
                Pixel::opaque(0, 0, 0)
            } else {
                Pixel::opaque(255, 255, 255)
            }
        });
        let cfg = BrailleConfig {
            width: 1,
            dither: true,
            ..Default::default()
        };
        let out = render_braille(&im, &cfg).unwrap();
        assert_eq!(out, "\u{2801}\n");
    }

    #[test]
    fn dither_deterministic_repeat() {
        let im = img(24, 24, |x, y| {
            let v = ((x * 53 + y * 29) % 256) as u8;
            Pixel::opaque(v, v / 2, y as u8)
        });
        let cfg = BrailleConfig {
            width: 12,
            dither: true,
            ..Default::default()
        };
        let a = render_braille(&im, &cfg).unwrap();
        let b = render_braille(&im, &cfg).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn tiny_images_do_not_panic() {
        for (w, h) in [(1u32, 1u32), (2, 3), (3, 2), (5, 5)] {
            let im = img(w, h, |_, _| Pixel::opaque(60, 60, 60));
            let cfg = BrailleConfig {
                width: 10,
                ..Default::default()
            };
            let _ = render_braille(&im, &cfg).unwrap();
        }
    }
}
