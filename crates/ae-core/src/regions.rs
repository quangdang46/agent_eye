//! CandidateRegion contract (plan §8, Phase 5A).
//!
//! > A region is a deterministic geometric candidate produced by image
//! > analysis. It is NOT guaranteed to correspond to a semantic object or a
//! > human-perceived layout region. The agent interprets meaning.
//!
//! Contract, fixed before any heuristic implementation (Phase 5B):
//!
//! * **Type name is `CandidateRegion`** — never `Region` — to keep the
//!   epistemic status in the API.
//! * `id` values are stable strings (`r1`, `r2`, …) assigned after a
//!   deterministic sort: y1 → x1 → area descending → id order. No hash or
//!   random component anywhere.
//! * `bounds` are half-open `[x1, y1) × [y1, y2)` source pixels; provenance
//!   to the original image is exact by construction.
//! * Metrics are pure functions of the pixel data inside `bounds`:
//!   `area` = w·h / total_pixels ∈ [0,1], `edge_density` = mean Sobel
//!   strength /255 ∈ [0,1], `color_variance` = chroma variance ∈ [0,1].
//! * Stability requirement: the same input bytes and config MUST produce
//!   byte-identical regions across runs (asserted 10× in tests).
//! * Forbidden forever: labels, classes, confidence scores, importance,
//!   descriptions.

use crate::analysis::{color_variance, sobel_edges};
use crate::geometry::HalfOpenBounds;
use crate::image::Image;
use serde::{Deserialize, Serialize};

/// One deterministic geometric candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CandidateRegion {
    /// Stable id: `"r1"`, `"r2"`, … assigned in deterministic sort order.
    pub id: String,
    /// Half-open source-pixel rectangle `[x1,y1,x2,y2)`.
    pub bounds: HalfOpenBounds,
    /// Fraction of total image area, `[0.0, 1.0]`.
    pub area: f32,
    /// Mean Sobel edge strength within bounds, normalized `/255`, `[0.0, 1.0]`.
    pub edge_density: f32,
    /// Chroma variance within bounds, `[0.0, 1.0]`.
    pub color_variance: f64,
}

impl CandidateRegion {
    /// Computes metrics for `bounds` against `img`. Pure function of pixels.
    pub fn measure(img: &Image, bounds: HalfOpenBounds) -> crate::error::Result<Self> {
        bounds.validate()?;
        let img_w = img.dimensions.width;
        let img_h = img.dimensions.height;
        if bounds.x2 > img_w || bounds.y2 > img_h {
            return Err(crate::invalid_dimensions(format!(
                "region {bounds:?} exceeds image {img_w}x{img_h}"
            )));
        }
        let total = u64::from(img_w) * u64::from(img_h);
        let area = if total == 0 {
            0.0
        } else {
            bounds.area() as f32 / total as f32
        };
        let edges = sobel_edges(&img.pixels);
        let edge_density =
            mean_edge_in(&edges, img.dimensions.width as usize, &bounds).clamp(0.0, 1.0);
        Ok(Self {
            id: String::new(), // assigned by assign_ids after sorting
            bounds,
            area,
            edge_density,
            color_variance: color_variance::chroma_variance_window(&img.pixels, bounds),
        })
    }
}

/// Mean Sobel magnitude over the interior of `bounds` (borders contribute 0
/// by the sobel policy; they are included so density stays comparable
/// across region sizes).
fn mean_edge_in(edges: &[f32], stride: usize, b: &HalfOpenBounds) -> f32 {
    let (x1, y1) = (b.x1 as usize, b.y1 as usize);
    let (x2, y2) = (b.x2 as usize, b.y2 as usize);
    if x1 >= x2 || y1 >= y2 {
        return 0.0;
    }
    let mut sum = 0.0f32;
    for row in y1..y2 {
        let row_start = row * stride;
        let row_end = (row_start + x2).min(edges.len());
        if row_start + x1 >= row_end {
            continue;
        }
        sum += edges[row_start + x1..row_end].iter().sum::<f32>();
    }
    let count = ((y2 - y1) * (x2 - x1)) as f32;
    if count == 0.0 {
        0.0
    } else {
        sum / count / 255.0
    }
}

/// Assigns stable ids `r1..rN` after sorting candidates deterministically:
/// y1 ascending → x1 ascending → area descending → bounds lexicographic.
///
/// Consumes unmeasured candidates so ids can never drift from metrics.
pub fn assign_ids(mut regions: Vec<CandidateRegion>) -> Vec<CandidateRegion> {
    regions.sort_by(|a, b| {
        a.bounds
            .y1
            .cmp(&b.bounds.y1)
            .then(a.bounds.x1.cmp(&b.bounds.x1))
            .then(
                b.bounds
                    .area()
                    .cmp(&a.bounds.area())
                    .then(a.bounds.cmp_key(&b.bounds)),
            )
    });
    for (i, r) in regions.iter_mut().enumerate() {
        r.id = format!("r{}", i + 1);
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Dimensions, ImageMetadata, Pixel, PixelBuffer};

    fn img(w: u32, h: u32, fill: impl Fn(u32, u32) -> Pixel) -> Image {
        let dims = Dimensions::new(w, h).unwrap();
        let pixels: Vec<Pixel> = (0..w * h).map(|i| fill(i % w, i / w)).collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        Image::new(dims, buf, ImageMetadata::default()).unwrap()
    }

    #[test]
    fn contract_metrics_bounded_and_exact_area() {
        // White box on black: region covering the box has known area.
        let im = img(10, 10, |x, y| {
            if (3..7).contains(&x) && (2..6).contains(&y) {
                Pixel::opaque(255, 255, 255)
            } else {
                Pixel::opaque(0, 0, 0)
            }
        });
        let b = HalfOpenBounds::new(3, 2, 7, 6).unwrap();
        let r = CandidateRegion::measure(&im, b).unwrap();
        assert!((r.area - 0.16).abs() < 1e-6, "4x4 of 100 px");
        assert!((0.0..=1.0).contains(&r.edge_density));
        assert!((0.0..=1.0).contains(&(r.color_variance as f32)));
    }

    #[test]
    fn contract_rejects_out_of_image_bounds() {
        let im = img(5, 5, |_, _| Pixel::opaque(1, 2, 3));
        assert!(CandidateRegion::measure(&im, HalfOpenBounds::new(0, 0, 6, 5).unwrap()).is_err());
        assert!(CandidateRegion::measure(&im, HalfOpenBounds::new(3, 3, 3, 3).unwrap()).is_ok());
    }

    #[test]
    fn ids_stable_under_deterministic_sort() {
        let mk = |x1: u32, y1: u32, x2: u32, y2: u32| CandidateRegion {
            id: String::new(),
            bounds: HalfOpenBounds::new(x1, y1, x2, y2).unwrap(),
            area: 0.0,
            edge_density: 0.0,
            color_variance: 0.0,
        };
        // Insertion order scrambled on purpose.
        let regions = vec![mk(5, 5, 9, 9), mk(0, 0, 2, 2), mk(3, 0, 6, 4)];
        let sorted = assign_ids(regions);
        assert_eq!(sorted[0].id, "r1");
        assert_eq!(sorted[0].bounds.to_array(), [0, 0, 2, 2]); // y1=0, x1=0
        assert_eq!(sorted[1].bounds.to_array(), [3, 0, 6, 4]); // y1=0, x1=3
        assert_eq!(sorted[2].bounds.to_array(), [5, 5, 9, 9]); // y1=5
    }

    #[test]
    fn stability_ten_runs_identical() {
        let im = img(24, 24, |x, y| {
            let v = ((x * 13 + y * 7) % 256) as u8;
            Pixel::opaque(v, v.saturating_sub(20), v)
        });
        let b = HalfOpenBounds::new(2, 2, 20, 20).unwrap();
        let first = CandidateRegion::measure(&im, b).unwrap();
        for _ in 0..10 {
            let again = CandidateRegion::measure(&im, b).unwrap();
            assert_eq!(first, again, "stability contract violated");
        }
    }

    #[test]
    fn no_semantic_fields_exist() {
        // Compile-time guard: struct fields are exactly the contract set.
        let im = img(4, 4, |_, _| Pixel::opaque(0, 0, 0));
        let r = CandidateRegion::measure(&im, HalfOpenBounds::covering(4, 4)).unwrap();
        let CandidateRegion {
            id: _,
            bounds: _,
            area: _,
            edge_density: _,
            color_variance: _,
        } = r;
    }
}
