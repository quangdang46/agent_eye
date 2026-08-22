//! Render configuration types (`--renderer`, `--width`, `--aspect`,
//! `--invert`, `--charset` CLI surface, plan §7).
//!
//! [`RenderConfig`] here is the *complete* request shape; the render engine
//! ([`crate::render`]) consumes it. [`ColorMode`] is declared now so the
//! config surface is stable — v1 only implements `None` (plain text); the
//! others are Phase 3/8 presentation modes and are rejected by name until
//! implemented (no silent no-ops).

use crate::charset::{presets, Charset};
use crate::sampling::DEFAULT_ASPECT_RATIO;
use ae_core::{rendering, Result};

/// Output width default: plan §6 (`--width` default 100 chars).
pub const DEFAULT_RENDER_WIDTH: u32 = 100;

/// Renderer selection (`--renderer ascii|blocks`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RendererType {
    /// Classic ASCII character ramp.
    #[default]
    Ascii,
    /// Unicode block shading (█▓▒░) — higher spatial density.
    Blocks,
}

impl RendererType {
    /// Parses the CLI flag value.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "ascii" => Ok(Self::Ascii),
            "blocks" => Ok(Self::Blocks),
            other => Err(rendering(format!(
                "unknown renderer '{other}' (expected 'ascii' or 'blocks')"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Blocks => "blocks",
        }
    }

    /// Default charset preset for this renderer.
    pub fn default_charset(&self) -> Result<Charset> {
        match self {
            Self::Ascii => presets::standard(),
            Self::Blocks => presets::blocks(),
        }
    }
}

/// Color presentation mode. v1 ships `None`; Grayscale/TrueColor land in
/// later phases (plan §8 P1 items) — constructing them is allowed but the
/// render engine rejects them with a clear error instead of silently
/// downgrading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorMode {
    /// Plain text — agent default, no escape sequences.
    #[default]
    None,
    /// ANSI 256-level grayscale (Phase 3).
    Grayscale,
    /// `\x1b[38;2;R;G;Bm` truecolor (Phase 8 compatibility).
    TrueColor,
}

impl ColorMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(Self::None),
            "grayscale" => Ok(Self::Grayscale),
            "truecolor" => Ok(Self::TrueColor),
            other => Err(rendering(format!(
                "unknown color mode '{other}' (expected none|grayscale|truecolor)"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Grayscale => "grayscale",
            Self::TrueColor => "truecolor",
        }
    }

    /// v1 support matrix: only `None` renders today.
    pub fn supported_in_v1(self) -> bool {
        matches!(self, Self::None)
    }
}

/// Full render request.
///
/// `out_h` is derived from `width` + `aspect_ratio` at sampling time unless
/// explicitly pinned; `charset_override` corresponds to the raw `--charset`
/// string (preset name or custom ramp), resolved once before rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderConfig {
    pub renderer: RendererType,
    /// Requested output width in characters (capped at image resolution).
    pub width: u32,
    /// Terminal-cell correction; 0.5 default, 1.0 = square (`--no-aspect`).
    pub aspect_ratio: f32,
    pub invert: bool,
    /// Requested output height in rows; `None` derives from width + aspect.
    pub height: Option<u32>,
    /// Raw `--charset` value: preset name or custom glyph string.
    pub charset_override: Option<String>,
    pub color: ColorMode,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            renderer: RendererType::default(),
            width: DEFAULT_RENDER_WIDTH,
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            invert: false,
            height: None,
            charset_override: None,
            color: ColorMode::default(),
        }
    }
}

impl RenderConfig {
    /// ASCII defaults at `width`.
    pub fn ascii(width: u32) -> Self {
        Self {
            width,
            ..Default::default()
        }
    }

    /// Blocks renderer defaults at `width`.
    pub fn blocks(width: u32) -> Self {
        Self {
            renderer: RendererType::Blocks,
            width,
            ..Default::default()
        }
    }

    /// Validates numeric invariants independent of any image.
    pub fn validate(&self) -> Result<()> {
        if !self.aspect_ratio.is_finite() || self.aspect_ratio <= 0.0 {
            return Err(rendering(format!(
                "aspect_ratio must be finite and > 0, got {}",
                self.aspect_ratio
            )));
        }
        if self.width == 0 {
            return Err(rendering("width must be > 0"));
        }
        if self.width > 10_000 {
            // Plan §14 output limits: max render width.
            return Err(rendering(format!(
                "width {} exceeds max render width 10000",
                self.width
            )));
        }
        if !self.color.supported_in_v1() {
            return Err(rendering(format!(
                "color mode '{}' not supported in v1",
                self.color.as_str()
            )));
        }
        Ok(())
    }

    /// Resolves the effective charset: override (preset name or custom
    /// string) else the renderer's preset.
    pub fn resolve_charset(&self) -> Result<Charset> {
        match &self.charset_override {
            Some(spec) => presets::resolve(spec),
            None => self.renderer.default_charset(),
        }
    }

    /// `--no-aspect` convenience: square blocks.
    pub fn without_aspect(mut self) -> Self {
        self.aspect_ratio = 1.0;
        self
    }

    /// `--invert` convenience: flip luminance mapping.
    pub fn with_invert(mut self) -> Self {
        self.invert = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_plan() {
        let c = RenderConfig::default();
        assert_eq!(c.renderer, RendererType::Ascii);
        assert_eq!(c.width, 100);
        assert_eq!(c.aspect_ratio, 0.5);
        assert!(!c.invert);
        assert_eq!(c.charset_override, None);
        assert_eq!(c.color, ColorMode::None);
    }

    #[test]
    fn renderer_parse_roundtrip() {
        assert_eq!(RendererType::parse("ascii").unwrap(), RendererType::Ascii);
        assert_eq!(RendererType::parse("blocks").unwrap(), RendererType::Blocks);
        assert_eq!(RendererType::Ascii.as_str(), "ascii");
        assert_eq!(RendererType::Blocks.as_str(), "blocks");
        assert!(RendererType::parse("braille").is_err());
        assert!(RendererType::parse("").is_err());
    }

    #[test]
    fn renderer_default_charsets_differ() {
        let a = RendererType::Ascii.default_charset().unwrap();
        let b = RendererType::Blocks.default_charset().unwrap();
        assert_eq!(a.name, "standard");
        assert_eq!(b.name, "blocks");
        assert_ne!(a.glyphs, b.glyphs);
    }

    #[test]
    fn color_mode_parse_and_v1_support() {
        assert_eq!(ColorMode::parse("none").unwrap(), ColorMode::None);
        assert_eq!(ColorMode::parse("grayscale").unwrap(), ColorMode::Grayscale);
        assert_eq!(ColorMode::parse("truecolor").unwrap(), ColorMode::TrueColor);
        assert!(ColorMode::parse("rgb").is_err());
        assert!(ColorMode::None.supported_in_v1());
        assert!(!ColorMode::Grayscale.supported_in_v1());
        assert!(!ColorMode::TrueColor.supported_in_v1());
    }

    #[test]
    fn validate_rejects_bad_numeric_config() {
        assert!(RenderConfig::ascii(80).validate().is_ok());
        let mut c = RenderConfig::ascii(10);
        c.aspect_ratio = 0.0;
        assert!(c.validate().is_err());
        c.aspect_ratio = f32::NAN;
        assert!(c.validate().is_err());
        c.aspect_ratio = -2.0;
        assert!(c.validate().is_err());
        c.aspect_ratio = 0.5;
        c.width = 0;
        assert!(c.validate().is_err());
        c.width = 10_001;
        assert!(c.validate().is_err());
        c.width = 10_000;
        assert!(c.validate().is_ok()); // boundary inclusive
    }

    #[test]
    fn validate_rejects_unsupported_color_modes() {
        let mut c = RenderConfig::ascii(10);
        c.color = ColorMode::TrueColor;
        assert!(c.validate().is_err());
        c.color = ColorMode::Grayscale;
        assert!(c.validate().is_err());
    }

    #[test]
    fn charset_resolution_preset_custom_fallback() {
        let mut c = RenderConfig::default();
        assert_eq!(c.resolve_charset().unwrap().name, "standard");
        c.charset_override = Some("blocks".into());
        assert_eq!(c.resolve_charset().unwrap().name, "blocks");
        c.charset_override = Some("@# ".into());
        let custom = c.resolve_charset().unwrap();
        assert_eq!(custom.name, "custom");
        assert_eq!(custom.len(), 3);
        c.charset_override = Some("x".into()); // single glyph rejected
        assert!(c.resolve_charset().is_err());
    }

    #[test]
    fn builder_conveniences_flip_fields() {
        let c = RenderConfig::ascii(40).without_aspect().with_invert();
        assert_eq!(c.aspect_ratio, 1.0);
        assert!(c.invert);
        let b = RenderConfig::blocks(40).with_invert();
        assert_eq!(b.renderer, RendererType::Blocks);
        assert!(b.invert);
    }

    #[test]
    fn config_combination_matrix_is_consistent() {
        for renderer in [RendererType::Ascii, RendererType::Blocks] {
            for invert in [false, true] {
                for aspect in [0.5f32, 1.0] {
                    let c = RenderConfig {
                        renderer,
                        invert,
                        aspect_ratio: aspect,
                        ..Default::default()
                    };
                    c.validate().unwrap();
                    c.resolve_charset().unwrap();
                }
            }
        }
    }
}
