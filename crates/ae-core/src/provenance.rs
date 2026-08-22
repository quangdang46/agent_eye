use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::geometry::{CoordinateTransform, HalfOpenBounds};

/// First-class source tracking attached to every `ae` output.
///
/// Guarantees an agent can always map a representation back to the exact
/// original image bytes and pixel coordinates:
///
/// * `source_hash` — SHA-256 of the **original encoded bytes** (not decoded
///   pixels), so identity survives independent of decoder behavior.
/// * `source_bounds` — the half-open rectangle of the original image this
///   output was derived from.
/// * `transform` — affine mapping from output cells back to
///   `source_bounds` pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_hash: String,
    pub source_bounds: HalfOpenBounds,
    pub transform: CoordinateTransform,
}

impl Provenance {
    /// Computes provenance for an output grid rendered from `source_bounds`
    /// of the byte stream `original_bytes`.
    pub fn compute(
        original_bytes: &[u8],
        source_bounds: HalfOpenBounds,
        transform: CoordinateTransform,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(original_bytes);
        Self {
            source_hash: hex::encode(hasher.finalize()),
            source_bounds,
            transform,
        }
    }

    /// Same as [`Provenance::compute`] but reuses a precomputed hash
    /// (e.g. when one invocation derives many outputs from one input).
    pub fn with_hash(
        source_hash: String,
        source_bounds: HalfOpenBounds,
        transform: CoordinateTransform,
    ) -> Self {
        Self {
            source_hash,
            source_bounds,
            transform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transform_for(bounds: HalfOpenBounds, w: u32, h: u32) -> CoordinateTransform {
        CoordinateTransform::new(bounds, w, h)
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256 of empty string per NIST FIPS 180-4 test vector.
        let p = Provenance::compute(
            b"",
            HalfOpenBounds::covering(10, 10),
            transform_for(HalfOpenBounds::covering(10, 10), 5, 5),
        );
        assert_eq!(
            p.source_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_is_of_bytes_not_pixels() {
        let a = Provenance::compute(
            b"identical",
            HalfOpenBounds::covering(8, 8),
            transform_for(HalfOpenBounds::covering(8, 8), 4, 4),
        );
        let b = Provenance::compute(
            b"identical",
            HalfOpenBounds::covering(8, 8),
            transform_for(HalfOpenBounds::covering(8, 8), 4, 4),
        );
        assert_eq!(a, b);
        let c = Provenance::compute(
            b"different!",
            HalfOpenBounds::covering(8, 8),
            transform_for(HalfOpenBounds::covering(8, 8), 4, 4),
        );
        assert_ne!(a.source_hash, c.source_hash);
    }

    #[test]
    fn serializes_roundtrip() {
        let bounds = HalfOpenBounds::new(300, 80, 1440, 900).unwrap();
        let t = transform_for(bounds, 80, 45);
        let p = Provenance::compute(b"bytes", bounds, t);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"source_hash\":\""));
        let back: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn with_hash_reuse() {
        let bounds = HalfOpenBounds::covering(4, 4);
        let p = Provenance::with_hash("deadbeef".into(), bounds, transform_for(bounds, 2, 2));
        assert_eq!(p.source_hash, "deadbeef");
        assert_eq!(p.transform.output_width, 2);
    }
}
