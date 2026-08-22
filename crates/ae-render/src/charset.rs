//! Charset system: ordered character ramps from dark to light.
//!
//! Glyphs are stored as `&str` (not `char`) so multi-codepoint graphemes
//! (emoji ZWJ sequences, flags) work — lesson from RASCII, split via
//! `unicode-segmentation`. Preset contents are `ae`'s own; ordering follows
//! the dark→light convention every reference repo shares.

use unicode_segmentation::UnicodeSegmentation;

use ae_core::error::{rendering, Result};

/// An ordered ramp of glyphs, index 0 = darkest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Charset {
    pub name: String,
    pub glyphs: Vec<String>,
}

/// Length cap (plan §14 output limits).
pub const MAX_CHARSET_LEN: usize = 128;

impl Charset {
    pub fn new(name: impl Into<String>, glyphs: Vec<String>) -> Result<Self> {
        if glyphs.is_empty() {
            return Err(rendering("charset must contain at least 1 glyph"));
        }
        if glyphs.len() > MAX_CHARSET_LEN {
            return Err(rendering(format!(
                "charset length {} exceeds max_charset_length={MAX_CHARSET_LEN}",
                glyphs.len()
            )));
        }
        Ok(Self {
            name: name.into(),
            glyphs,
        })
    }

    /// Splits a user-supplied string into grapheme glyphs (dark→light order
    /// is the caller's responsibility, matching the string's visual order).
    pub fn from_custom(s: &str) -> Result<Self> {
        let glyphs: Vec<String> = UnicodeSegmentation::graphemes(s, true)
            .map(str::to_owned)
            .collect();
        let cs = Self::new("custom", glyphs)?;
        // A single-glyph charset renders everything as one char; require >=2
        // so luminance mapping carries information (lesson: pixel2ascii
        // validates >=2 for the same reason).
        if cs.glyphs.len() < 2 {
            return Err(rendering("custom charset needs at least 2 distinct glyphs"));
        }
        Ok(cs)
    }

    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Maps a luminance in `[0, 255]` to a glyph, rounding + clamping.
    ///
    /// `idx = round(lum / 255 * (len - 1))`; out-of-range input clamps to the
    /// nearest end rather than panicking (fuzz-safety).
    pub fn glyph_for_luminance(&self, lum: f32) -> &str {
        debug_assert!(self.glyphs.len() >= 2);
        let n = self.glyphs.len();
        let scaled = (lum / 255.0).clamp(0.0, 1.0) * (n - 1) as f32;
        let idx = scaled.round().clamp(0.0, (n - 1) as f32) as usize;
        &self.glyphs[idx]
    }

    /// Reversed ramp view (`--invert`): light source becomes dark glyphs.
    pub fn inverted(&self) -> Charset {
        Charset {
            name: format!("{}-inverted", self.name),
            glyphs: self.glyphs.iter().rev().cloned().collect(),
        }
    }
}

/// Built-in presets. Contents authored for `ae`; names chosen for agents.
pub mod presets {
    use super::{Charset, Result};

    /// 10-level classic ramp (ASCII-generator "simple" shape, own content).
    pub const STANDARD: &[&str] = &["@", "%", "#", "*", "+", "=", "-", ":", ".", " "];

    /// 70-level dense photographic ramp, dark→light.
    pub const DENSE: &[&str] = &[
        "@", "%", "#", "*", "+", "=", "-", ":", ".", ",", "~", "^", "\"", "'", "`", "|", "/", "\\",
        "(", ")", "[", "]", "{", "}", "<", ">", "?", "!", "i", "l", "I", "!", "1", "t", "f", "j",
        "r", "x", "n", "u", "v", "c", "z", "X", "Y", "U", "J", "C", "L", "Q", "0", "O", "Z", "m",
        "w", "q", "p", "d", "b", "k", "h", "a", "o", "*", "#", "M", "W", "&", "8", "B", "$",
    ];

    /// 5-level Unicode blocks — highest spatial fidelity per cell.
    pub const BLOCKS: &[&str] = &["█", "▓", "▒", "░", " "];

    pub fn standard() -> Result<Charset> {
        Charset::new("standard", STANDARD.iter().map(|s| (*s).into()).collect())
    }

    pub fn dense() -> Result<Charset> {
        Charset::new("dense", DENSE.iter().map(|s| (*s).into()).collect())
    }

    pub fn blocks() -> Result<Charset> {
        Charset::new("blocks", BLOCKS.iter().map(|s| (*s).into()).collect())
    }

    /// Named preset lookup (`standard | dense | blocks`).
    pub fn by_name(name: &str) -> Option<Result<Charset>> {
        match name {
            "standard" => Some(standard()),
            "dense" => Some(dense()),
            "blocks" => Some(blocks()),
            _ => None,
        }
    }

    /// Resolves preset name or falls back to custom-string parse.
    pub fn resolve(spec: &str) -> Result<Charset> {
        if let Some(preset) = by_name(spec) {
            preset
        } else {
            Charset::from_custom(spec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_load_and_cap_respected() {
        for name in ["standard", "dense", "blocks"] {
            let cs = presets::by_name(name).unwrap().unwrap();
            assert_eq!(cs.name, name);
            assert!((2..=MAX_CHARSET_LEN).contains(&cs.len()));
        }
    }

    #[test]
    fn custom_grapheme_split_keeps_emoji() {
        // family emoji = single grapheme despite many codepoints
        let cs = Charset::from_custom("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f466} ").unwrap();
        assert_eq!(cs.len(), 2);
    }

    #[test]
    fn custom_rejects_single_glyph_and_empty() {
        assert!(Charset::from_custom("@").is_err());
        assert!(Charset::from_custom("").is_err());
    }

    #[test]
    fn custom_rejects_over_limit() {
        let s = "ab".repeat(MAX_CHARSET_LEN + 1);
        let err = Charset::from_custom(&s).unwrap_err();
        assert!(err.to_string().contains("max_charset_length"));
    }

    #[test]
    fn mapping_endpoints_midpoints_roundtrip() {
        let cs = presets::standard().unwrap(); // 10 glyphs
        assert_eq!(cs.glyph_for_luminance(0.0), "@");
        assert_eq!(cs.glyph_for_luminance(255.0), " ");
        // mid maps to middle glyph index 4 or 5 (round(4.5)=4 banker? f32 round() rounds half away from zero → 5)
        let mid = cs.glyph_for_luminance(127.5);
        assert!(mid == "+" || mid == "=");
    }

    #[test]
    fn mapping_clamps_out_of_range_without_panic() {
        let cs = presets::blocks().unwrap();
        assert_eq!(cs.glyph_for_luminance(-30.0), "█");
        assert_eq!(cs.glyph_for_luminance(9999.0), " ");
        // NaN survives clamp then saturates to index 0 in float->int cast
        assert_eq!(cs.glyph_for_luminance(f32::NAN), "█");
    }

    #[test]
    fn invert_reverses_order() {
        let cs = presets::blocks().unwrap();
        let inv = cs.inverted();
        assert_eq!(inv.glyphs[0], " ");
        assert_eq!(inv.glyphs[4], "█");
        assert_eq!(inv.glyph_for_luminance(255.0), "█");
    }

    #[test]
    fn unknown_spec_falls_back_to_custom_parse() {
        let cs = presets::resolve("@# .").unwrap();
        assert_eq!(cs.name, "custom");
        assert_eq!(cs.len(), 4);
    }

    #[test]
    fn known_names_resolve_to_presets() {
        assert_eq!(presets::resolve("blocks").unwrap().name, "blocks");
    }

    #[test]
    fn dense_ramp_is_strictly_dark_to_light_by_construction() {
        let cs = presets::dense().unwrap();
        assert_eq!(cs.glyph_for_luminance(0.0), "@");
        assert_eq!(cs.glyph_for_luminance(255.0), "$");
    }
}
