pub mod analysis;
pub mod decode;
pub mod error;
pub mod geometry;
pub mod image;

pub mod provenance;
pub mod regions;

pub use analysis::{
    average_pixel, block_luminance, color_variance, contrast, luminance, luminance_range,
    sobel_edges, VisualComplexity,
};
pub use decode::decode_bytes;
pub use error::{
    decode_failed, invalid_dimensions, rendering, resource_limit, serialization,
    unsupported_format, AeError, Result,
};
pub use geometry::{CoordinateTransform, HalfOpenBounds};
pub use image::{Dimensions, Image, ImageMetadata, Limits, Pixel, PixelBuffer};
pub use provenance::Provenance;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(super::CRATE_NAME, "ae-core");
    }

    #[test]
    fn error_display_messages() {
        use super::AeError;
        let cases: Vec<(AeError, &str)> = vec![
            (super::decode_failed("bad png"), "decode failed: bad png"),
            (super::invalid_dimensions("0x0"), "invalid dimensions: 0x0"),
            (
                super::unsupported_format("tiff"),
                "unsupported format: tiff",
            ),
            (
                super::resource_limit("too many pixels"),
                "resource limit exceeded: too many pixels",
            ),
            (
                super::rendering("charset empty"),
                "rendering failed: charset empty",
            ),
            (super::serialization("json"), "serialization failed: json"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }
}
