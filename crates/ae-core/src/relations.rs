//! Spatial relations (plan §8): 7 formal relations over half-open bounds.
//!
//! Formal definitions — no guessing, pure math:
//!
//! | relation   | definition                                  |
//! |------------|---------------------------------------------|
//! | `LeftOf`   | A.x2 <= B.x1 (touching counts: A ends where B starts) |
//! | `RightOf`  | B.x2 <= A.x1                                |
//! | `Above`    | A.y2 <= B.y1                                |
//! | `Below`    | B.y2 <= A.y1                                |
//! | `Inside`   | A within B (A.x1>=B.x1 ∧ A.x2<=B.x2 ∧ same for y) |
//! | `Contains` | B inside A                                  |
//! | `Overlaps` | x-intervals intersect AND y-intervals intersect |
//!
//! Deterministic output ordering: relation type enum order → a.id → b.id.

use serde::{Deserialize, Serialize};

/// The 7 core relations. Order matters: it defines the deterministic
/// emission order before id tie-breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RelationType {
    LeftOf,
    RightOf,
    Above,
    Below,
    Inside,
    Contains,
    Overlaps,
}

impl RelationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LeftOf => "left_of",
            Self::RightOf => "right_of",
            Self::Above => "above",
            Self::Below => "below",
            Self::Inside => "inside",
            Self::Contains => "contains",
            Self::Overlaps => "overlaps",
        }
    }
}

/// One detected relation between two regions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    #[serde(rename = "type")]
    pub kind: String,
    /// Id of the first region (subject).
    pub a: String,
    /// Id of the second region (object).
    pub b: String,
}

use crate::geometry::HalfOpenBounds;

/// Computes all relations holding between two bounds.
fn relations_between(a_bounds: &HalfOpenBounds, b_bounds: &HalfOpenBounds) -> Vec<RelationType> {
    let mut out = Vec::new();
    let x_touch_or_gap = a_bounds.x2 <= b_bounds.x1;
    let x_rev_touch_or_gap = b_bounds.x2 <= a_bounds.x1;
    let y_touch_or_gap = a_bounds.y2 <= b_bounds.y1;
    let y_rev_touch_or_gap = b_bounds.y2 <= a_bounds.y1;

    if x_touch_or_gap {
        out.push(RelationType::LeftOf);
    }
    if x_rev_touch_or_gap {
        out.push(RelationType::RightOf);
    }
    if y_touch_or_gap {
        out.push(RelationType::Above);
    }
    if y_rev_touch_or_gap {
        out.push(RelationType::Below);
    }

    let inside = a_bounds.x1 >= b_bounds.x1
        && a_bounds.x2 <= b_bounds.x2
        && a_bounds.y1 >= b_bounds.y1
        && a_bounds.y2 <= b_bounds.y2;
    if inside && a_bounds != b_bounds {
        out.push(RelationType::Inside);
    }
    // Contains is the mirror of Inside; identical bounds report neither so
    // duplicates never appear.
    let contains = b_bounds.x1 >= a_bounds.x1
        && b_bounds.x2 <= a_bounds.x2
        && b_bounds.y1 >= a_bounds.y1
        && b_bounds.y2 <= a_bounds.y2;
    if contains && a_bounds != b_bounds {
        out.push(RelationType::Contains);
    }

    let overlap_x = a_bounds.overlaps_x(b_bounds);
    let overlap_y = a_bounds.overlaps_y(b_bounds);
    if overlap_x && overlap_y {
        out.push(RelationType::Overlaps);
    }
    out
}

/// Pairwise relation computation over measured regions with ids already
/// assigned. Emission is fully ordered: `(RelationType, a.id, b.id)` with
/// `a` always the lexicographically-smaller id in each pair.
pub fn compute_relations(regions: &[crate::regions::CandidateRegion]) -> Vec<Relation> {
    let mut out = Vec::new();
    for i in 0..regions.len() {
        for j in (i + 1)..regions.len() {
            // Canonical direction: smaller id is `a`.
            let (lo, hi) = if regions[i].id <= regions[j].id {
                (&regions[i], &regions[j])
            } else {
                (&regions[j], &regions[i])
            };
            for kind in relations_between(&lo.bounds, &hi.bounds) {
                out.push(Relation {
                    kind: kind.as_str().to_owned(),
                    a: lo.id.clone(),
                    b: hi.id.clone(),
                });
            }
        }
    }
    out.sort_by(|x, y| {
        type_key(&x.kind)
            .cmp(&type_key(&y.kind))
            .then(x.a.cmp(&y.a))
            .then(x.b.cmp(&y.b))
    });
    out
}

fn type_key(k: &str) -> u8 {
    match k {
        "left_of" => 0,
        "right_of" => 1,
        "above" => 2,
        "below" => 3,
        "inside" => 4,
        "contains" => 5,
        _ => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x1: u32, y1: u32, x2: u32, y2: u32) -> HalfOpenBounds {
        HalfOpenBounds::new(x1, y1, x2, y2).unwrap()
    }

    #[test]
    fn left_of_includes_touching() {
        // A ends exactly where B begins — documented as left_of.
        assert!(
            relations_between(&b(0, 0, 10, 10), &b(10, 0, 20, 10)).contains(&RelationType::LeftOf)
        );
        assert!(
            relations_between(&b(0, 0, 9, 10), &b(10, 0, 20, 10)).contains(&RelationType::LeftOf)
        );
    }

    #[test]
    fn right_of_mirrors_left_of() {
        let rels = relations_between(&b(0, 0, 10, 10), &b(10, 0, 20, 10));
        assert!(rels.contains(&RelationType::LeftOf));
        assert!(!rels.contains(&RelationType::RightOf));
        let rels_rev = relations_between(&b(10, 0, 20, 10), &b(0, 0, 10, 10));
        assert!(rels_rev.contains(&RelationType::RightOf));
        assert!(!rels_rev.contains(&RelationType::LeftOf));
    }

    #[test]
    fn above_below_vertical() {
        let rels = relations_between(&b(0, 0, 10, 5), &b(0, 8, 10, 12));
        assert!(rels.contains(&RelationType::Above));
        assert!(!rels.contains(&RelationType::Below));
        let rels_rev = relations_between(&b(0, 8, 10, 12), &b(0, 0, 10, 5));
        assert!(rels_rev.contains(&RelationType::Below));
    }

    #[test]
    fn inside_contains_mutually_exclusive() {
        let outer = b(0, 0, 100, 100);
        let inner = b(10, 10, 20, 20);
        let rels = relations_between(&inner, &outer);
        assert!(rels.contains(&RelationType::Inside));
        assert!(!rels.contains(&RelationType::Contains));
        let rels_rev = relations_between(&outer, &inner);
        assert!(rels_rev.contains(&RelationType::Contains));
        assert!(!rels_rev.contains(&RelationType::Inside));
    }

    #[test]
    fn identical_bounds_report_no_inside_no_contains() {
        let r = b(5, 5, 15, 15);
        let rels = relations_between(&r, &r);
        assert!(!rels.contains(&RelationType::Inside));
        assert!(!rels.contains(&RelationType::Contains));
    }

    #[test]
    fn overlaps_requires_both_axes() {
        let rels = relations_between(&b(0, 0, 10, 10), &b(5, 5, 15, 15));
        assert!(rels.contains(&RelationType::Overlaps));
        assert!(!rels.contains(&RelationType::LeftOf));
        // Same column but vertically disjoint: no overlap.
        let rels2 = relations_between(&b(0, 0, 10, 10), &b(0, 15, 10, 25));
        assert!(!rels2.contains(&RelationType::Overlaps));
    }

    #[test]
    fn compute_relations_orders_deterministically() {
        use crate::regions::{assign_ids, CandidateRegion};
        let mk = |x1: u32, y1: u32, x2: u32, y2: u32| CandidateRegion {
            id: String::new(),
            bounds: b(x1, y1, x2, y2),
            area: 0.0,
            edge_density: 0.0,
            color_variance: 0.0,
        };
        let regions = assign_ids(vec![mk(50, 50, 60, 60), mk(0, 0, 10, 10), mk(30, 0, 40, 4)]);
        let rels = compute_relations(&regions);
        // Sorted by (type_order, a, b); verify monotone non-decreasing keys.
        for w in rels.windows(2) {
            let (ka, kb) = (type_key(&w[0].kind), type_key(&w[1].kind));
            assert!(ka <= kb);
            if ka == kb {
                assert!((w[0].a.clone(), w[0].b.clone()) <= (w[1].a.clone(), w[1].b.clone()));
            }
        }
        // r2=(30..40,0..4) and r3=(50..60,50..60): r2 above r3.
        assert!(rels
            .iter()
            .any(|r| r.kind == "above" && r.a == "r2" && r.b == "r3"));
        // r1=(0..10) left of both others.
        assert!(rels.iter().any(|r| r.kind == "left_of" && r.a == "r1"));
    }
}
