use serde::{Deserialize, Serialize};

use crate::error::{invalid_dimensions, AeError, Result};

/// Half-open rectangle in source-pixel space: `[x1, y1) × [y1, y2)`.
///
/// `x1`/`y1` are inclusive, `x2`/`y2` are exclusive. This convention makes
/// area arithmetic exact (`width = x2 - x1`) and adjacent rectangles
/// non-overlapping by construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HalfOpenBounds {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

impl HalfOpenBounds {
    /// Full-image bounds for the given dimensions.
    pub fn covering(width: u32, height: u32) -> Self {
        Self {
            x1: 0,
            y1: 0,
            x2: width,
            y2: height,
        }
    }

    /// Constructs bounds, validating the half-open invariant.
    pub fn new(x1: u32, y1: u32, x2: u32, y2: u32) -> Result<Self> {
        let b = Self { x1, y1, x2, y2 };
        b.validate()?;
        Ok(b)
    }

    /// Validates `x1 <= x2`, `y1 <= y2`. Empty (zero-area) is allowed.
    pub fn validate(&self) -> Result<()> {
        if self.x1 > self.x2 || self.y1 > self.y2 {
            return Err(invalid_dimensions(format!(
                "half-open bounds violated: x1={x1} x2={x2} y1={y1} y2={y2}",
                x1 = self.x1,
                x2 = self.x2,
                y1 = self.y1,
                y2 = self.y2,
            )));
        }
        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.x2.saturating_sub(self.x1)
    }

    pub fn height(&self) -> u32 {
        self.y2.saturating_sub(self.y1)
    }

    /// Pixel-count area (`width * height`), saturating on overflow.
    pub fn area(&self) -> u64 {
        u64::from(self.width()) * u64::from(self.height())
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x1 && x < self.x2 && y >= self.y1 && y < self.y2
    }

    /// `[x1, y1, x2, y2]` array form.
    pub fn to_array(&self) -> [u32; 4] {
        [self.x1, self.y1, self.x2, self.y2]
    }

    /// Lexicographic tie-break ordering for deterministic region sorts.
    pub fn cmp_key(&self, other: &Self) -> std::cmp::Ordering {
        self.x1
            .cmp(&other.x1)
            .then(self.y1.cmp(&other.y1))
            .then(self.x2.cmp(&other.x2))
            .then(self.y2.cmp(&other.y2))
    }
}

impl TryFrom<[u32; 4]> for HalfOpenBounds {
    type Error = AeError;

    fn try_from(value: [u32; 4]) -> Result<Self> {
        Self::new(value[0], value[1], value[2], value[3])
    }
}

impl From<HalfOpenBounds> for [u32; 4] {
    fn from(b: HalfOpenBounds) -> Self {
        [b.x1, b.y1, b.x2, b.y2]
    }
}

/// Affine mapping from output grid coordinates back to source pixels.
///
/// `source_x = output_x * scale_x + offset_x`
/// `source_y = output_y * scale_y + offset_y`
///
/// Crop/region change only offsets; resize/render change scales; zoom changes
/// both. Every machine-readable `ae` output carries one of these so an agent
/// can map any output cell back to original image coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoordinateTransform {
    pub source_bounds: HalfOpenBounds,
    pub output_width: u32,
    pub output_height: u32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

impl CoordinateTransform {
    /// Builds the transform for rendering `source_bounds` into an
    /// `output_width × output_height` grid.
    ///
    /// Zero-width/height outputs yield zero scales (degenerate but valid).
    pub fn new(source_bounds: HalfOpenBounds, output_width: u32, output_height: u32) -> Self {
        let src_w = f64::from(source_bounds.width());
        let src_h = f64::from(source_bounds.height());
        let (out_w, out_h) = (f64::from(output_width), f64::from(output_height));
        Self {
            source_bounds,
            output_width,
            output_height,
            scale_x: if out_w > 0.0 { src_w / out_w } else { 0.0 },
            scale_y: if out_h > 0.0 { src_h / out_h } else { 0.0 },
            offset_x: f64::from(source_bounds.x1),
            offset_y: f64::from(source_bounds.y1),
        }
    }

    /// Maps an output column to source pixel x.
    pub fn source_x(&self, output_x: u32) -> f64 {
        f64::from(output_x).mul_add(self.scale_x, self.offset_x)
    }

    /// Maps an output row to source pixel y.
    pub fn source_y(&self, output_y: u32) -> f64 {
        f64::from(output_y).mul_add(self.scale_y, self.offset_y)
    }

    /// Maps an output cell center to source pixel coordinates.
    pub fn source_center(&self, output_x: u32, output_y: u32) -> (f64, f64) {
        (
            self.source_x(output_x) + self.scale_x / 2.0,
            self.source_y(output_y) + self.scale_y / 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_basic_invariants() {
        let b = HalfOpenBounds::new(10, 20, 30, 50).unwrap();
        assert_eq!(b.width(), 20);
        assert_eq!(b.height(), 30);
        assert_eq!(b.area(), 600);
        assert!(!b.is_empty());
    }

    #[test]
    fn bounds_empty_allowed() {
        let b = HalfOpenBounds::new(5, 5, 5, 5).unwrap();
        assert!(b.is_empty());
        assert_eq!(b.area(), 0);
    }

    #[test]
    fn bounds_rejects_inverted() {
        assert!(HalfOpenBounds::new(6, 0, 5, 4).is_err());
        assert!(HalfOpenBounds::new(0, 6, 4, 5).is_err());
        let err = HalfOpenBounds::new(9, 0, 3, 3).unwrap_err();
        assert_eq!(
            err.to_string(),
            "invalid dimensions: half-open bounds violated: x1=9 x2=3 y1=0 y2=3"
        );
    }

    #[test]
    fn bounds_contains_half_open() {
        let b = HalfOpenBounds::new(0, 0, 10, 10).unwrap();
        assert!(b.contains(0, 0));
        assert!(b.contains(9, 9));
        assert!(!b.contains(10, 5), "x2 is exclusive");
        assert!(!b.contains(5, 10), "y2 is exclusive");
    }

    #[test]
    fn bounds_array_roundtrip() {
        let b = HalfOpenBounds::try_from([3, 4, 7, 11]).unwrap();
        let arr: [u32; 4] = b.into();
        assert_eq!(arr, [3, 4, 7, 11]);
        assert_eq!(
            HalfOpenBounds::covering(1440, 900).to_array(),
            [0, 0, 1440, 900]
        );
    }

    #[test]
    fn transform_identity() {
        let src = HalfOpenBounds::covering(100, 80);
        let t = CoordinateTransform::new(src, 100, 80);
        assert_eq!(t.scale_x, 1.0);
        assert_eq!(t.scale_y, 1.0);
        assert_eq!(t.offset_x, 0.0);
        assert_eq!(t.source_center(50, 40), (50.5, 40.5));
    }

    #[test]
    fn transform_crop_changes_offset_only() {
        // crop → offset changes, scale unchanged
        let src = HalfOpenBounds::new(300, 80, 500, 280).unwrap();
        let t = CoordinateTransform::new(src, 200, 200);
        assert_eq!(t.scale_x, 1.0);
        assert_eq!(t.offset_x, 300.0);
        assert_eq!(t.offset_y, 80.0);
        assert_eq!(t.source_x(0), 300.0);
        assert_eq!(t.source_y(199), 279.0);
    }

    #[test]
    fn transform_resize_changes_scale() {
        // resize → scale changes
        let src = HalfOpenBounds::covering(1440, 900);
        let t = CoordinateTransform::new(src, 80, 54);
        assert!((t.scale_x - 18.0).abs() < f64::EPSILON);
        assert!((t.scale_y - 900.0 / 54.0).abs() < 1e-12);
        // cell 79 spans source [1422, 1440)
        assert_eq!(t.source_x(79), 1422.0);
    }

    #[test]
    fn transform_zoom_scale_and_offset() {
        // zoom → both change (crop + resample)
        let src = HalfOpenBounds::new(300, 80, 1440, 900).unwrap();
        let t = CoordinateTransform::new(src, 80, 45);
        assert_eq!(t.output_width, 80);
        assert!((t.scale_x - 14.25).abs() < 1e-12);
        assert!((t.scale_y - (820.0 / 45.0)).abs() < 1e-12);
        let (cx, cy) = t.source_center(40, 22);
        assert!((cx - 877.125).abs() < 1e-9);
        assert!((cy - 490.0).abs() < 1e-9);
    }

    #[test]
    fn transform_zero_output_is_degenerate_not_panic() {
        let src = HalfOpenBounds::covering(100, 100);
        let t = CoordinateTransform::new(src, 0, 0);
        assert_eq!(t.scale_x, 0.0);
        assert_eq!(t.source_x(0), 0.0);
    }
}
