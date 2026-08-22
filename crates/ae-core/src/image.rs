use serde::{Deserialize, Serialize};

use crate::error::{invalid_dimensions, resource_limit, Result};
use crate::geometry::HalfOpenBounds;

/// Hard input ceilings enforced before any large allocation.
///
/// Values from plan §14. Decoding paths must check dimensions against
/// [`Limits::max_pixels`] *before* materializing the pixel buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_file_size: u64,
    pub max_pixels: u64,
    pub max_width: u32,
    pub max_height: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 100 * 1024 * 1024,
            max_pixels: 25_000_000,
            max_width: 100_000,
            max_height: 100_000,
        }
    }
}

impl Limits {
    /// Validates raw byte length and declared dimensions against the limits.
    ///
    /// Called with header-provided dimensions before buffer allocation so a
    /// decompression bomb is rejected by arithmetic, not OOM.
    pub fn check_input(&self, byte_len: u64, width: u32, height: u32) -> Result<()> {
        if byte_len > self.max_file_size {
            return Err(resource_limit(format!(
                "input of {byte_len} bytes exceeds max_file_size={}",
                self.max_file_size
            )));
        }
        if width == 0 || height == 0 {
            return Err(invalid_dimensions(format!(
                "zero dimension: {width}x{height}"
            )));
        }
        if width > self.max_width {
            return Err(resource_limit(format!(
                "width {width} exceeds max_width={}",
                self.max_width
            )));
        }
        if height > self.max_height {
            return Err(resource_limit(format!(
                "height {height} exceeds max_height={}",
                self.max_height
            )));
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > self.max_pixels {
            return Err(resource_limit(format!(
                "{width}x{height} = {pixels} pixels exceeds max_pixels={}",
                self.max_pixels
            )));
        }
        Ok(())
    }
}

/// Width/height pair in source-pixel units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(invalid_dimensions(format!(
                "zero dimension: {width}x{height}"
            )));
        }
        Ok(Self { width, height })
    }

    /// Full-image bounds for these dimensions.
    pub fn bounds(&self) -> HalfOpenBounds {
        HalfOpenBounds::covering(self.width, self.height)
    }
}

/// One RGBA pixel, 4 bytes on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Rec. 709 analysis luminance in `[0, 255]`.
    ///
    /// Always computed internally for analysis; grayscale *presentation* is a
    /// separate output mode. Coefficients match plan §7 exactly and are the
    /// single source of truth for luminance math in `ae`.
    pub fn luminance(&self) -> f32 {
        0.212_6 * f32::from(self.r) + 0.715_2 * f32::from(self.g) + 0.072_2 * f32::from(self.b)
    }
}

/// Flat row-major RGBA pixel storage for [`Image`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelBuffer {
    dims: Dimensions,
    data: Vec<Pixel>,
}

impl PixelBuffer {
    /// Allocates a zeroed (transparent black) buffer.
    pub fn zeroed(dims: Dimensions) -> Self {
        Self {
            dims,
            data: vec![Pixel::default(); dims.width as usize * dims.height as usize],
        }
    }

    /// Wraps existing pixels; errors if `data.len()` mismatches dimensions.
    pub fn from_vec(dims: Dimensions, data: Vec<Pixel>) -> Result<Self> {
        let expected = dims.width as usize * dims.height as usize;
        if data.len() != expected {
            return Err(invalid_dimensions(format!(
                "pixel count {} does not match {}x{} = {expected}",
                data.len(),
                dims.width,
                dims.height
            )));
        }
        Ok(Self { dims, data })
    }

    pub fn dimensions(&self) -> Dimensions {
        self.dims
    }

    pub fn as_slice(&self) -> &[Pixel] {
        &self.data
    }

    /// Mutable row-major access; used by decoders to fill the buffer.
    pub fn data_mut(&mut self) -> &mut [Pixel] {
        &mut self.data
    }

    pub fn get(&self, x: u32, y: u32) -> Option<Pixel> {
        if x >= self.dims.width || y >= self.dims.height {
            return None;
        }
        let idx = y as usize * self.dims.width as usize + x as usize;
        self.data.get(idx).copied()
    }
}

/// Container metadata carried alongside pixels.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Lowercase container/format name, e.g. `"png"`, `"jpeg"`, `"webp"`.
    pub format: Option<String>,
}

/// The canonical internal image representation.
///
/// Deliberately does **not** expose decoder-crate types: decoding converts
/// into this type at the boundary so the rest of `ae-core`/`ae-render` never
/// depends on the `image` crate's API surface.
#[derive(Clone, Debug)]
pub struct Image {
    pub dimensions: Dimensions,
    pub pixels: PixelBuffer,
    pub metadata: ImageMetadata,
}

impl Image {
    /// Builds an image from parts, validating pixel-count consistency.
    pub fn new(
        dimensions: Dimensions,
        pixels: PixelBuffer,
        metadata: ImageMetadata,
    ) -> Result<Self> {
        if pixels.dimensions() != dimensions {
            return Err(invalid_dimensions(format!(
                "pixel buffer {}x{} does not match image {}x{}",
                pixels.dimensions().width,
                pixels.dimensions().height,
                dimensions.width,
                dimensions.height
            )));
        }
        Ok(Self {
            dimensions,
            pixels,
            metadata,
        })
    }

    pub fn bounds(&self) -> HalfOpenBounds {
        self.dimensions.bounds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AeError;

    #[test]
    fn default_limits_match_plan() {
        let l = Limits::default();
        assert_eq!(l.max_file_size, 104_857_600);
        assert_eq!(l.max_pixels, 25_000_000);
        assert_eq!(l.max_width, 100_000);
        assert_eq!(l.max_height, 100_000);
    }

    #[test]
    fn limits_accept_normal_input() {
        let l = Limits::default();
        assert!(l.check_input(1024, 1440, 900).is_ok());
        // exactly at pixel ceiling
        let w = 5000u32;
        let h = 25_000_000u64 / u64::from(w);
        assert!(l.check_input(0, w, h as u32).is_ok());
    }

    #[test]
    fn limits_reject_bombs_before_allocation() {
        let l = Limits::default();
        // decompression bomb: tiny bytes, huge declared dims
        let err = l.check_input(64, 100_000, 100_000).unwrap_err();
        assert!(err.to_string().contains("max_pixels"), "{err}");
        assert!(matches!(err, AeError::ResourceLimit(_)));
    }

    #[test]
    fn limits_reject_zero_and_oversize() {
        let l = Limits::default();
        assert!(l.check_input(10, 0, 5).is_err());
        assert!(l.check_input(10, 5, 0).is_err());
        assert!(l.check_input(200_000_000, 10, 10).is_err());
        assert!(l.check_input(0, 100_001, 1).is_err());
        assert!(l.check_input(0, 1, 100_001).is_err());
    }

    #[test]
    fn dimensions_reject_zero() {
        assert!(Dimensions::new(0, 100).is_err());
        assert!(Dimensions::new(100, 0).is_err());
        let d = Dimensions::new(640, 480).unwrap();
        assert_eq!(d.bounds().to_array(), [0, 0, 640, 480]);
    }

    #[test]
    fn luminance_rec709_endpoints_and_mids() {
        assert_eq!(Pixel::opaque(0, 0, 0).luminance(), 0.0);
        assert!((Pixel::opaque(255, 255, 255).luminance() - 255.0).abs() < 1e-4);
        // pure primaries: R=54.2, G=182.4, B=18.4 (Rec.709 * 255)
        assert!((Pixel::opaque(255, 0, 0).luminance() - 54.213).abs() < 1e-3);
        assert!((Pixel::opaque(0, 255, 0).luminance() - 182.376).abs() < 1e-3);
        assert!((Pixel::opaque(0, 0, 255).luminance() - 18.411).abs() < 1e-3);
    }

    #[test]
    fn buffer_indexing_row_major() {
        let d = Dimensions::new(3, 2).unwrap();
        let mut px = PixelBuffer::zeroed(d);
        px.data_mut()[0] = Pixel::opaque(1, 0, 0);
        px.data_mut()[4] = Pixel::opaque(0, 2, 0); // x=1,y=1
        assert_eq!(px.get(0, 0), Some(Pixel::opaque(1, 0, 0)));
        assert_eq!(px.get(1, 1), Some(Pixel::opaque(0, 2, 0)));
        assert_eq!(px.get(3, 0), None, "x out of range");
        assert_eq!(px.get(0, 2), None, "y out of range");
    }

    #[test]
    fn buffer_from_vec_validates_length() {
        let d = Dimensions::new(2, 2).unwrap();
        let ok = vec![Pixel::default(); 4];
        assert!(PixelBuffer::from_vec(d, ok).is_ok());
        let bad = vec![Pixel::default(); 3];
        let err = PixelBuffer::from_vec(d, bad).unwrap_err();
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn image_new_validates_consistency() {
        let d = Dimensions::new(2, 2).unwrap();
        let other_d = Dimensions::new(4, 1).unwrap();
        let buf = PixelBuffer::zeroed(other_d);
        let err = Image::new(d, buf, ImageMetadata::default()).unwrap_err();
        assert!(err.to_string().contains("does not match image"));

        let buf = PixelBuffer::from_vec(d, vec![Pixel::opaque(9, 9, 9); 4]).unwrap();
        let img = Image::new(d, buf, ImageMetadata::default()).unwrap();
        assert_eq!(img.bounds(), d.bounds());
    }
}
