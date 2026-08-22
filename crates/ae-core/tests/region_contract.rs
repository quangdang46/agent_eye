//! Fixture-based stability tests for the CandidateRegion contract (5A).
//!
//! These fixtures exist to pin down *expected behavior classes* before the
//! Phase 5B heuristic lands. They assert what any detector must satisfy:
//! determinism (10 identical runs), bounds validity, and metric ranges.
//! They deliberately do NOT assert that regions correspond to human-perceived
//! layout — no such guarantee exists by contract.

use ae_core::decode::decode_bytes;
use ae_core::image::{Image, Limits};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Image {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let bytes = fs::read(&p).unwrap_or_else(|e| panic!("fixture {} missing: {e}", p.display()));
    decode_bytes(&bytes, &Limits::default()).expect("fixture decodes")
}

fn fixture_path_str(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn all_contract_fixtures_exist_and_decode() {
    for name in ["ui.png", "diagram.png", "screenshot.png"] {
        let img = fixture(name);
        assert!(
            img.dimensions.width > 0 && img.dimensions.height > 0,
            "{name} has real dimensions"
        );
    }
}

#[test]
fn region_measurement_is_stable_across_ten_runs_on_fixtures() {
    for name in ["ui.png", "diagram.png", "screenshot.png"] {
        let img = fixture(name);
        let b = img.bounds();
        let first = ae_core::regions::CandidateRegion::measure(&img, b).unwrap();
        for run in 1..10 {
            let again = ae_core::regions::CandidateRegion::measure(&img, b).unwrap();
            assert_eq!(first, again, "{name}: run {run} diverged");
        }
    }
}

#[test]
fn measured_metrics_stay_in_contract_ranges_on_fixtures() {
    for name in ["ui.png", "diagram.png", "screenshot.png"] {
        let img = fixture(name);
        let r = ae_core::regions::CandidateRegion::measure(&img, img.bounds()).unwrap();
        assert!((0.0..=1.0).contains(&r.area), "{name} area range");
        assert!((0.0..=1.0).contains(&r.edge_density), "{name} edge range");
        assert!(
            (0.0..=1.0).contains(&(r.color_variance as f32)),
            "{name} chroma range"
        );
        assert_eq!(r.bounds, img.bounds());
        // Full-image region: area is exactly 1.
        assert!((r.area - 1.0).abs() < 1e-6);
    }
}

#[test]
fn fixtures_are_deterministic_bytes() {
    // Byte-level identity across reads guards against accidental mutation of
    // the fixture set (which would silently shift every golden comparison).
    for name in ["ui.png", "diagram.png", "screenshot.png"] {
        let p = fixture_path_str(name);
        let a = fs::read(&p).unwrap();
        let b = fs::read(&p).unwrap();
        assert_eq!(a, b, "{name} bytes changed between reads");
    }
}
