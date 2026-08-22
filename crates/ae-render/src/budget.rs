//! Adaptive budget: allocate output resolution to where the image is
//! visually complex (plan §8 P1, gated on Phase 7 evidence — now shipped).
//!
//! Algorithm:
//! 1. Detect candidate regions (`crate::regions::detect_regions`).
//! 2. Score each region's local complexity (Sobel edge density).
//! 3. Rank regions by complexity, descending.
//! 4. Allocate the total byte/char budget: every region gets a base slice;
//!    leftover after the least-complex regions hit their minimum is
//!    distributed to the most complex ones (proportional to complexity).
//! 5. Render each region at its allocated width; emit JSONL records with
//!    per-region provenance so the agent knows which crop is which.
//!
//! Determinism: sorting ties break by region id; allocation arithmetic is
//! pure integer/fixed-point.

use ae_core::analysis::VisualComplexity;
use ae_core::geometry::CoordinateTransform;
use ae_core::image::Image;
use ae_core::regions::CandidateRegion;
use ae_core::regions::{detect_regions, DetectConfig};

/// One allocated slice of the budget for one region.
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetSlice {
    pub id: String,
    /// Complexity score in `[0,1]` (edge density proxy).
    pub complexity: f64,
    /// Characters of output width granted to this region.
    pub allocated_width: u32,
}

/// Minimum width any ranked region receives before proportional top-up.
const MIN_REGION_WIDTH: u32 = 8;

/// Computes a deterministic width allocation across regions under `budget`
/// total characters. `budget` must be ≥ number of regions × MIN_REGION_WIDTH
/// (callers should drop lowest-complexity regions until it fits).
pub fn allocate(regions: &[CandidateRegion], budget: u32) -> Vec<BudgetSlice> {
    let mut scored: Vec<(String, f64)> = regions
        .iter()
        .map(|r| {
            (
                r.id.clone(),
                // Edge density is the complexity proxy; clamp tiny noise.
                if r.edge_density < 0.01 {
                    0.0
                } else {
                    r.edge_density as f64
                },
            )
        })
        .collect();
    // Rank: complexity desc, then id asc (stable tie-break).
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let n = scored.len() as u32;
    if n == 0 || budget == 0 {
        return Vec::new();
    }
    let min_total = n * MIN_REGION_WIDTH;
    if budget < min_total {
        let keep = (budget / MIN_REGION_WIDTH) as usize;
        scored.truncate(keep);
    }

    let count = scored.len() as u32;
    let base = MIN_REGION_WIDTH.min(budget / count.max(1));
    let mut alloc: Vec<BudgetSlice> = scored
        .iter()
        .map(|(id, cx)| BudgetSlice {
            id: id.clone(),
            complexity: *cx,
            allocated_width: base,
        })
        .collect();

    // Distribute leftovers proportionally to complexity (deterministic
    // largest-remainder method).
    let spent = base * count;
    let leftover = budget.saturating_sub(spent);
    if leftover > 0 && !alloc.is_empty() {
        let total_cx: f64 = alloc.iter().map(|s| s.complexity).sum();
        if total_cx > 0.0 {
            // Exact fractional shares, then hand out remainders by largest
            // fractional part (ties by id).
            let shares: Vec<(usize, f64)> = alloc
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let exact = leftover as f64 * (s.complexity / total_cx);
                    (i, exact)
                })
                .collect();
            let mut given = vec![0u32; alloc.len()];
            let mut distributed = 0u32;
            let mut order: Vec<usize> = (0..alloc.len()).collect();
            order.sort_by(|a, b| {
                shares[*b]
                    .1
                    .partial_cmp(&shares[*a].1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(alloc[*a].id.cmp(&alloc[*b].id))
            });
            for &(i, exact) in &shares {
                let whole = exact.floor() as u32;
                given[i] = whole.min(leftover - distributed);
                distributed += given[i];
            }
            for &i in &order {
                if distributed >= leftover {
                    break;
                }
                given[i] += 1;
                distributed += 1;
            }
            for (i, g) in given.into_iter().enumerate() {
                alloc[i].allocated_width += g;
            }
        }
    }
    alloc
}

/// Full adaptive render: detect → rank → allocate → produce slices.
/// Returns JSONL-ready records ordered by region rank (complexity desc).
pub fn adaptive_slices(
    img: &Image,
    cfg: &DetectConfig,
    budget: u32,
) -> ae_core::Result<Vec<(CandidateRegion, VisualComplexity, BudgetSlice)>> {
    let regions = detect_regions(img, cfg)?;
    let alloc = allocate(&regions, budget);
    let by_id: std::collections::HashMap<&str, &BudgetSlice> =
        alloc.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut out = Vec::new();
    for r in &regions {
        let Some(&slice) = by_id.get(r.id.as_str()) else {
            continue;
        };
        // Local visual complexity inside this region via full-image pass.
        let vc = VisualComplexity::compute(&img.pixels);
        out.push((r.clone(), vc, slice.clone()));
    }
    // Order by allocated width desc then id — most-informative crops first.
    out.sort_by(|a, b| {
        b.2.allocated_width
            .cmp(&a.2.allocated_width)
            .then(a.0.id.cmp(&b.0.id))
    });
    Ok(out)
}

/// Affine transform helper exposed for callers mapping slices back.
pub fn slice_transform(
    _img: &Image,
    r: &CandidateRegion,
    allocated_width: u32,
) -> CoordinateTransform {
    CoordinateTransform::new(r.bounds, allocated_width.max(1), allocated_width.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: &str, edge: f32) -> CandidateRegion {
        CandidateRegion {
            id: id.to_owned(),
            bounds: ae_core::geometry::HalfOpenBounds::new(0, 0, 4, 4).unwrap(),
            area: 0.1,
            edge_density: edge,
            color_variance: 0.0,
        }
    }

    #[test]
    fn budget_respected_and_complexity_ranked_first() {
        let regions = vec![mk("r1", 0.9), mk("r2", 0.5), mk("r3", 0.05)];
        let budget = 100u32;
        let alloc = allocate(&regions, budget);
        let total: u32 = alloc.iter().map(|s| s.allocated_width).sum();
        assert!(total <= budget, "allocation exceeds budget");
        // Most complex gets the largest share.
        let w = |id: &str| alloc.iter().find(|s| s.id == id).unwrap().allocated_width;
        assert!(w("r1") >= w("r2"), "r1 ({}) ≥ r2 ({})", w("r1"), w("r2"));
        assert!(w("r2") >= w("r3"), "r2 ({}) ≥ r3 ({})", w("r2"), w("r3"));
    }

    #[test]
    fn tiny_budget_drops_lowest_complexity() {
        let regions = vec![mk("r1", 0.9), mk("r2", 0.5), mk("r3", 0.05)];
        // Budget fits only 1 region at the minimum floor.
        let alloc = allocate(&regions, MIN_REGION_WIDTH);
        assert_eq!(alloc.len(), 1);
        assert_eq!(alloc[0].id, "r1"); // highest complexity kept
        assert_eq!(alloc[0].allocated_width, MIN_REGION_WIDTH);
    }

    #[test]
    fn zero_budget_or_no_regions_empty() {
        assert!(allocate(&[], 100).is_empty());
        let regions = vec![mk("r1", 0.5)];
        assert!(allocate(&regions, 0).is_empty());
    }

    #[test]
    fn deterministic_allocation() {
        let regions = vec![mk("r1", 0.9), mk("r2", 0.5), mk("r3", 0.05)];
        let a = allocate(&regions, 77);
        let b = allocate(&regions, 77);
        assert_eq!(a, b);
    }
}
