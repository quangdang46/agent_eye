//! Renderers: map sampled block luminance to glyphs.
//!
//! Both renderers share the same sampling pipeline ([`crate::sampling`]) and
//! differ only in charset mapping. Output is exposed as [`RenderedGrid`]
//! (row-major glyph strings) so callers can emit text or JSON identically.
//! Determinism contract: same image + same config ⇒ byte-identical output.

use crate::charset::{presets, Charset};
use crate::config::{ColorMode, RenderConfig, RendererType};
use crate::sampling::{sample_blocks, Block};
use ae_core::image::Image;
use ae_core::Result;

/// A rendered output grid: rows of grapheme-cluster strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedGrid {
    pub renderer: RendererType,
    pub charset_name: String,
    /// Presentation mode the grid was rendered with.
    pub color: ColorMode,
    /// Row-major; each entry is one glyph cluster (may be multi-codepoint).
    pub rows: Vec<Vec<String>>,
}

impl RenderedGrid {
    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    /// Plain-text rendering joined with newlines.
    pub fn to_text(&self) -> String {
        self.rows
            .iter()
            .map(|r| r.concat())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Samples `img` and maps each block's luminance through `charset`.
///
/// Shared path for both renderers — the only difference is the charset the
/// caller resolves beforehand (ASCII ramp vs blocks ramp), mirroring how
/// pixel2ascii/ASCII-generator structure the split.
pub fn render(img: &Image, cfg: &RenderConfig, charset: &Charset) -> Result<RenderedGrid> {
    let effective = if cfg.invert {
        charset.inverted()
    } else {
        charset.clone()
    };
    cfg.validate()?;
    let out_h = cfg
        .height
        .unwrap_or((cfg.width as f32 / cfg.aspect_ratio) as u32)
        .max(1);
    let blocks = sample_blocks(img, cfg.width, out_h, cfg.aspect_ratio)?;
    let cols = effective_row_width(&blocks, img, cfg);
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(blocks.len() / cols.max(1));
    let mut row = Vec::with_capacity(cols);
    // Style cache: emit an escape only when the color actually changes.
    // Flat-color regions collapse from per-cell escapes to a handful of
    // transitions (lesson from RASCII: ~1800% output reduction there).
    let bg = cfg
        .background
        .map(|(r, g, b)| format!("\u{1b}[48;2;{r};{g};{b}m"))
        .unwrap_or_default();
    let mut last_style: Option<String> = None;
    let mut last_glyph: &str = "";
    for b in &blocks {
        let lum = b.luminance();
        let glyph = effective.glyph_for_luminance(lum);
        let fg = match cfg.color {
            ColorMode::None => String::new(),
            ColorMode::Grayscale => {
                let level = (lum / 255.0).clamp(0.0, 1.0);
                let shade = 232 + (level * 23.0).round() as u8; // ANSI 232..=255
                format!("\u{1b}[38;5;{shade}m")
            }
            ColorMode::TrueColor => {
                // Block-average RGB IS the pixel-true color at this scale —
                // no quantization beyond the sampling itself.
                let p = b.pixel;
                format!("\u{1b}[38;2;{};{};{}m", p.r, p.g, p.b)
            }
        };
        // Background is constant across cells; fold it into the style key
        // so the first colored cell paints both planes.
        let style = format!("{bg}{fg}");
        let cell = if cfg.color == ColorMode::None {
            glyph.to_owned()
        } else if Some(&style) == last_style.as_ref() && glyph == last_glyph {
            String::new() // repeat: same glyph, same color
        } else {
            last_style = Some(style.clone());
            format!("{style}{glyph}\u{1b}[0m")
        };
        last_glyph = glyph;
        row.push(cell);
        if row.len() == cols {
            rows.push(std::mem::replace(&mut row, Vec::with_capacity(cols)));
        }
    }
    Ok(RenderedGrid {
        renderer: cfg.renderer,
        charset_name: effective.name,
        color: cfg.color,
        rows,
    })
}

/// Column count of the sampled grid: capped at image resolution by
/// `sample_blocks`, so derive it rather than trusting the request.
fn effective_row_width(blocks: &[Block], img: &Image, cfg: &RenderConfig) -> usize {
    let requested = cfg.width as usize;
    let max_cols = img.dimensions.width as usize;
    requested.min(max_cols).min(blocks.len().max(1))
}

/// Convenience: ASCII renderer with its default preset.
pub fn render_ascii(img: &Image, cfg: &RenderConfig) -> Result<RenderedGrid> {
    render(img, cfg, &presets::standard()?)
}

/// Convenience: blocks renderer with its default preset.
pub fn render_blocks(img: &Image, cfg: &RenderConfig) -> Result<RenderedGrid> {
    render(img, cfg, &presets::blocks()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae_core::image::{Dimensions, ImageMetadata, PixelBuffer};

    fn gradient_img(w: u32, h: u32) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let pixels: Vec<ae_core::image::Pixel> = (0..w * h)
            .map(|i| {
                let v = ((i * 7) % 256) as u8;
                ae_core::image::Pixel::opaque(v, v, v)
            })
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    fn solid_img(w: u32, h: u32, v: u8) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let buf = PixelBuffer::from_vec(
            dims,
            vec![ae_core::image::Pixel::opaque(v, v, v); (w * h) as usize],
        )
        .unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    #[test]
    fn ascii_black_to_white_spans_ramp() {
        // Fully dark image → darkest glyph; fully light → lightest.
        let black = solid_img(32, 32, 0);
        let white = solid_img(32, 32, 255);
        let cfg = RenderConfig {
            width: 16,
            height: Some(8),
            ..Default::default()
        };
        let dark_grid = render_ascii(&black, &cfg).unwrap();
        let light_grid = render_ascii(&white, &cfg).unwrap();
        assert_eq!(dark_grid.rows[0][0], "@");
        assert_eq!(light_grid.rows[0][0], " ");
    }

    #[test]
    fn grid_shape_matches_request() {
        let img = gradient_img(64, 64);
        let cfg = RenderConfig {
            width: 20,
            height: Some(10),
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert_eq!(g.height(), 10);
        assert_eq!(g.width(), 20);
        assert!(g.rows.iter().all(|r| r.len() == 20));
    }

    #[test]
    fn deterministic_identical_output() {
        let img = gradient_img(101, 57);
        let cfg = RenderConfig {
            width: 40,
            height: Some(20),
            ..Default::default()
        };
        let a = render_ascii(&img, &cfg).unwrap();
        let b = render_ascii(&img, &cfg).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_text(), b.to_text());
    }

    #[test]
    fn invert_flips_mapping() {
        let img = solid_img(16, 16, 0); // dark source
        let cfg = RenderConfig {
            width: 8,
            height: Some(4),
            invert: true,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        // Inverted charset maps luminance 0 → lightest glyph (" ").
        assert_eq!(g.rows[0][0], " ");
        assert_eq!(g.charset_name, "standard-inverted");
    }

    #[test]
    fn grayscale_wraps_glyph_in_ansi_shade() {
        let black = solid_img(16, 16, 0);
        let white = solid_img(16, 16, 255);
        let cfg = RenderConfig {
            width: 4,
            height: Some(2),
            color: ColorMode::Grayscale,
            ..Default::default()
        };
        let dark = render_ascii(&black, &cfg).unwrap();
        let light = render_ascii(&white, &cfg).unwrap();
        // Darkest ANSI gray ramp entry for black, lightest for white; the
        // glyph itself is unchanged inside the escape wrapper.
        assert!(dark.rows[0][0].contains("\u{1b}[38;5;232m"));
        assert!(dark.rows[0][0].ends_with("m@\u{1b}[0m"));
        assert!(light.rows[0][0].contains("\u{1b}[38;5;255m"));
        assert_eq!(dark.color, ColorMode::Grayscale);
    }

    #[test]
    fn plain_mode_has_no_escape_sequences() {
        let img = gradient_img(32, 32);
        let cfg = RenderConfig {
            width: 8,
            height: Some(4),
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert!(g.to_text().bytes().all(|b| b != 0x1b));
        assert_eq!(g.color, ColorMode::None);
    }

    #[test]
    fn truecolor_wraps_glyph_in_rgb_escape() {
        // Pure red image → \x1b[38;2;255;0;0m prefix.
        let dims = Dimensions::new(8, 8).unwrap();
        let buf = PixelBuffer::from_vec(dims, vec![ae_core::image::Pixel::opaque(255, 0, 0); 64])
            .unwrap();
        let img = Image::new(dims, buf, ImageMetadata::default()).unwrap();
        let cfg = RenderConfig {
            width: 4,
            height: Some(2),
            color: ColorMode::TrueColor,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert!(g.rows[0][0].starts_with("\u{1b}[38;2;255;0;0m#"));
        assert_eq!(g.color, ColorMode::TrueColor);
    }

    #[test]
    fn background_painted_in_first_style() {
        let img = solid_img(8, 8, 0);
        let cfg = RenderConfig {
            width: 4,
            height: Some(2),
            color: ColorMode::TrueColor,
            background: Some((10, 20, 30)),
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        // First cell opens with bg then fg escapes; reset closes.
        assert!(g.rows[0][0].starts_with("\u{1b}[48;2;10;20;30m\u{1b}[38;2;0;0;0m@"));
        assert!(g.rows[0][0].ends_with("\u{1b}[0m"));
        // None background → no 48;2 sequences anywhere in plain mode.
        let plain_cfg = RenderConfig {
            width: 4,
            height: Some(2),
            background: Some((10, 20, 30)),
            ..Default::default()
        };
        let gp = render_ascii(&img, &plain_cfg).unwrap();
        assert!(!gp.to_text().contains("48;2"));
    }

    #[test]
    fn style_cache_collapses_flat_regions() {
        // A solid image renders every cell identically: with caching the
        // first cell carries the escape, all repeats are empty strings.
        let img = solid_img(32, 32, 200);
        let cfg = RenderConfig {
            width: 8,
            height: Some(4),
            color: ColorMode::Grayscale,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        let flat: Vec<&String> = g.rows.iter().flatten().collect();
        let escapes = flat.iter().filter(|c| c.contains("\u{1b}")).count();
        assert_eq!(
            escapes, 1,
            "flat region must emit exactly one escape sequence"
        );
        assert!(flat[1..].iter().all(|c| c.is_empty()));
    }

    #[test]
    fn blocks_renderer_uses_block_charset() {
        let img = solid_img(16, 16, 255);
        let cfg = RenderConfig {
            renderer: RendererType::Blocks,
            width: 8,
            height: Some(4),
            ..Default::default()
        };
        let g = render_blocks(&img, &cfg).unwrap();
        assert_eq!(g.renderer, RendererType::Blocks);
        assert_eq!(g.rows[0][0], " "); // white → lightest block glyph
        let dark = solid_img(16, 16, 0);
        let gd = render_blocks(&dark, &cfg).unwrap();
        assert_eq!(gd.rows[0][0], "█");
    }

    #[test]
    fn custom_charset_flows_through() {
        let img = solid_img(10, 10, 128);
        let cs = presets::resolve("+#").unwrap(); // 2-glyph custom
        let cfg = RenderConfig {
            width: 5,
            height: Some(5),
            charset_override: Some("+#".into()),
            ..Default::default()
        };
        let g = render(&img, &cfg, &cs).unwrap();
        // lum 128/255 → round(1*0.502)=1 → "#"
        assert_eq!(g.rows[0][0], "#");
    }

    #[test]
    fn to_text_rows_join_with_newline() {
        let img = solid_img(4, 4, 0);
        let cfg = RenderConfig {
            width: 2,
            height: Some(2),
            aspect_ratio: 1.0,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert_eq!(g.to_text(), "@@\n@@");
    }

    #[test]
    fn tiny_image_single_cell() {
        let img = solid_img(1, 1, 255);
        let cfg = RenderConfig {
            width: 100,
            height: Some(50),
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert_eq!((g.width(), g.height()), (1, 1));
        assert_eq!(g.to_text(), " ");
    }

    #[test]
    fn golden_known_gradient_pattern() {
        // Left half black, right half white → left column '@', right ' '.
        let dims = Dimensions::new(2, 1).unwrap();
        let buf = PixelBuffer::from_vec(
            dims,
            vec![
                ae_core::image::Pixel::opaque(0, 0, 0),
                ae_core::image::Pixel::opaque(255, 255, 255),
            ],
        )
        .unwrap();
        let img = Image::new(dims, buf, ImageMetadata::default()).unwrap();
        let cfg = RenderConfig {
            width: 2,
            height: Some(1),
            aspect_ratio: 1.0,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert_eq!(g.to_text(), "@ ");
    }

    #[test]
    fn wide_image_full_width_coverage() {
        let img = gradient_img(500, 3);
        let cfg = RenderConfig {
            width: 100,
            height: Some(2),
            aspect_ratio: 1.0,
            ..Default::default()
        };
        let g = render_ascii(&img, &cfg).unwrap();
        assert_eq!(g.width(), 100);
        assert_eq!(g.height(), 2);
    }
}
