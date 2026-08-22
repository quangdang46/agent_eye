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
//!   `area` = w·h / total_pixels ∈ \[0,1\], `edge_density` = mean Sobel
//!   strength /255 ∈ \[0,1\], `color_variance` = chroma variance ∈ \[0,1\].
//! * Stability requirement: the same input bytes and config MUST produce
//!   byte-identical regions across runs (asserted 10× in tests).
//! * Forbidden forever: labels, classes, confidence scores, importance,
//!   descriptions.

use crate::analysis::{color_variance, sobel_edges};
use crate::geometry::HalfOpenBounds;
use crate::image::Image;
use serde::{Deserialize, Serialize};

/// Cap on candidates returned by one detection pass (plan §14).
pub const MAX_REGION_COUNT: usize = 500;

/// Detection configuration.
#[derive(Clone, Copy, Debug)]
pub struct DetectConfig {
    /// Luminance delta above which a pixel counts as an "active" (non-flat)
    /// cell relative to its row's median tone. Deterministic threshold.
    pub edge_threshold: f32,
    /// Merge two candidate rectangles when their gap is at most this many
    /// pixels on both axes (whitespace bridging).
    pub merge_gap: u32,
}

impl Default for DetectConfig {
    fn default() -> Self {
        Self {
            edge_threshold: 12.0,
            merge_gap: 2,
        }
    }
}

/// First v1 heuristic (plan §8 pipeline, simplified to be fully
/// deterministic and allocation-bounded):
///
/// 1. Sobel edge map (`analysis::sobel_edges`).
/// 2. Column/row projection profiles of edge strength — "active" bands are
///    runs where the profile exceeds `edge_threshold`.
/// 3. Candidate rectangles = cartesian products of active x-bands and
///    y-bands that contain at least one strong pixel (connected-component
///    stand-in with identical output for band-structured content).
/// 4. Iteratively merge rectangles whose whitespace gap ≤ `merge_gap` on
///    both axes.
/// 5. Keep at most [`MAX_REGION_COUNT`] (largest-area first), then assign
///    stable ids via [`assign_ids`].
///
/// Determinism: no randomness; ties broken by explicit orderings.
pub fn detect_regions(
    img: &Image,
    cfg: &DetectConfig,
) -> crate::error::Result<Vec<CandidateRegion>> {
    let w = img.dimensions.width;
    let h = img.dimensions.height;
    if w == 0 || h == 0 {
        return Ok(Vec::new());
    }
    let edges = sobel_edges(&img.pixels);
    let stride = w as usize;

    // Projection profiles: mean edge strength per column / per row.
    let mut col = vec![0.0f32; stride];
    let mut row = vec![0.0f32; h as usize];
    for y in 0..h as usize {
        for x in 0..stride {
            let e = edges[y * stride + x];
            col[x] += e;
            row[y] += e;
        }
    }
    for c in &mut col {
        *c /= h as f32;
    }
    for r in &mut row {
        *r /= w as f32;
    }

    let bands = |prof: &[f32]| -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut start: Option<usize> = None;
        for (i, v) in prof.iter().enumerate() {
            match start {
                None => {
                    if *v > cfg.edge_threshold {
                        start = Some(i);
                    }
                }
                Some(_) => {
                    if *v <= cfg.edge_threshold {
                        out.push((start.unwrap(), i));
                        start = None;
                    }
                }
            }
        }
        if let Some(s) = start {
            out.push((s, prof.len()));
        }
        out
    };

    // Cartesian candidates from active bands, keeping only those whose area
    // actually contains edge energy above threshold (sparsity filter).
    let xb = bands(&col);
    let yb = bands(&row);
    let mut rects: Vec<HalfOpenBounds> = Vec::new();
    for (y0, y1) in &yb {
        for (x0, x1) in &xb {
            let has_energy = (*y0..*y1)
                .any(|y| (*x0..*x1).any(|x| edges[y * stride + x] > cfg.edge_threshold * 4.0));
            if has_energy {
                rects.push(HalfOpenBounds::new(
                    *x0 as u32, *y0 as u32, *x1 as u32, *y1 as u32,
                )?);
            }
        }
    }

    // Merge passes until fixpoint: union rectangles separated by ≤ merge_gap
    // on BOTH axes (i.e., overlapping or nearly touching).
    loop {
        let mut merged_any = false;
        'outer: for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                if let Some(u) = try_merge(rects[i], rects[j], cfg.merge_gap) {
                    rects[i] = u;
                    rects.swap_remove(j);
                    merged_any = true;
                    break 'outer;
                }
            }
        }
        if !merged_any {
            break;
        }
    }

    // Bound the count deterministically: largest area first.
    if rects.len() > MAX_REGION_COUNT {
        rects.sort_by(|a, b| b.area().cmp(&a.area()).then(a.cmp_key(b)));
        rects.truncate(MAX_REGION_COUNT);
    }

    let mut regions = Vec::with_capacity(rects.len());
    for r in rects {
        regions.push(CandidateRegion::measure(img, r)?);
    }
    Ok(assign_ids(regions))
}

fn try_merge(a: HalfOpenBounds, b: HalfOpenBounds, gap: u32) -> Option<HalfOpenBounds> {
    // Gap on an axis: 0 when intervals intersect, else the whitespace
    // between them (order-independent).
    let gap_x = if a.overlaps_x(&b) {
        0
    } else {
        a.x2.abs_diff(b.x1).min(b.x2.abs_diff(a.x1))
    };
    let gap_y = if a.overlaps_y(&b) {
        0
    } else {
        a.y2.abs_diff(b.y1).min(b.y2.abs_diff(a.y1))
    };
    if gap_x <= gap && gap_y <= gap {
        HalfOpenBounds::new(
            a.x1.min(b.x1),
            a.y1.min(b.y1),
            a.x2.max(b.x2),
            a.y2.max(b.y2),
        )
        .ok()
    } else {
        None
    }
}

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

    #[test]
    fn detect_finds_two_separated_boxes() {
        // Two white boxes on black background, well separated.
        let im = img(40, 20, |x, y| {
            let in_a = (2..12).contains(&x) && (3..9).contains(&y);
            let in_b = (28..38).contains(&x) && (10..17).contains(&y);
            if in_a || in_b {
                Pixel::opaque(255, 255, 255)
            } else {
                Pixel::opaque(0, 0, 0)
            }
        });
        let regions = detect_regions(&im, &DetectConfig::default()).unwrap();
        assert!(!regions.is_empty(), "boxes produce candidates");
        assert!(regions.len() <= MAX_REGION_COUNT);
        // Every candidate must lie inside the image.
        for r in &regions {
            assert!(r.bounds.x2 <= 40 && r.bounds.y2 <= 20);
            assert!((0.0..=1.0).contains(&r.area));
        }
        // Ids are r1..rN with no gaps.
        for (i, r) in regions.iter().enumerate() {
            assert_eq!(r.id, format!("r{}", i + 1));
        }
    }

    #[test]
    fn detect_flat_image_yields_no_candidates() {
        let im = img(16, 16, |_, _| Pixel::opaque(77, 77, 77));
        assert!(detect_regions(&im, &DetectConfig::default())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn detect_deterministic_across_runs_and_configs() {
        let im = img(30, 30, |x, y| {
            let v = ((x * x + y * 5) % 251) as u8;
            Pixel::opaque(v, v / 2, v)
        });
        let cfg = DetectConfig::default();
        let a = detect_regions(&im, &cfg).unwrap();
        for _ in 0..5 {
            assert_eq!(detect_regions(&im, &cfg).unwrap(), a);
        }
    }

    #[test]
    fn merge_gap_bridges_nearby_rects() {
        let a = HalfOpenBounds::new(0, 0, 10, 10).unwrap();
        let b = HalfOpenBounds::new(11, 0, 20, 10).unwrap(); // gap 1 on x
        assert!(try_merge(a, b, 1).is_some());
        assert!(try_merge(a, b, 0).is_none());
        let c = HalfOpenBounds::new(0, 0, 10, 10).unwrap();
        let d = HalfOpenBounds::new(15, 0, 20, 10).unwrap(); // gap 5 on x
        assert!(try_merge(c, d, 4).is_none());
        assert!(try_merge(c, d, 5).is_some());
    }
}
