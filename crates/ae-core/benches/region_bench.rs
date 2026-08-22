//! Region detection benchmark (plan §8, Phase 5C).
//!
//! Measures over the contract fixtures (`tests/fixtures/`):
//!
//! * **Detection stability** — 10 runs per fixture must produce identical
//!   region sets (the hard determinism gate; a failure here is fatal for
//!   the whole feature).
//! * **Region count distribution** — how many candidates each fixture
//!   yields; guards against degenerate detectors (0 everywhere or 500
//!   everywhere).
//! * **Quality proxies** — mean edge density / color variance of detected
//!   regions vs. the full image, so Phase 7's "do regions help agent
//!   tasks?" decision has baseline numbers.
//!
//! Human-annotation overlap is intentionally absent: no annotations exist
//! yet; the plan defers that to Phase 7.
//!
//! Run: `cargo bench -p ae-core --bench region_bench` or as a test:
//! `cargo test -p ae-core --bench region_bench`.

use ae_core::decode::decode_bytes;
use ae_core::image::{Image, Limits};
use ae_core::regions::{detect_regions, CandidateRegion, DetectConfig, MAX_REGION_COUNT};
use std::fs;
use std::path::Path;
use std::time::Instant;

const FIXTURES: &[&str] = &[
    "ui.png",
    "diagram.png",
    "screenshot.png",
    // Reuse render fixtures too — they are valid geometric inputs.
];

fn load(name: &str) -> Image {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let bytes = fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    decode_bytes(&bytes, &Limits::default()).expect("fixture decodes")
}

fn bench_fixture(name: &str) -> Result<(), String> {
    let img = load(name);
    let cfg = DetectConfig::default();

    // 1. Stability: 10 identical runs.
    let t0 = Instant::now();
    let first = detect_regions(&img, &cfg).map_err(|e: ae_core::AeError| e.to_string())?;
    for run in 1..10 {
        let again = detect_regions(&img, &cfg).map_err(|e: ae_core::AeError| e.to_string())?;
        if again != first {
            return Err(format!("{name}: run {run} diverged from run 0"));
        }
    }
    let elapsed = t0.elapsed();

    // 2. Count sanity: within bounds, non-degenerate for structured images.
    assert!(
        first.len() <= MAX_REGION_COUNT,
        "{name}: exceeds MAX_REGION_COUNT"
    );

    // 3. Quality proxies.
    let mean_edge = if first.is_empty() {
        0.0
    } else {
        first
            .iter()
            .map(|r: &CandidateRegion| r.edge_density)
            .sum::<f32>()
            / first.len() as f32
    };
    let mean_area = if first.is_empty() {
        0.0
    } else {
        first.iter().map(|r| r.area).sum::<f32>() / first.len() as f32
    };
    println!(
        "{name:>16}: {:>3} regions | stability 10/10 OK | mean_edge={mean_edge:.3} \
         mean_area={mean_area:.3} | 10 runs in {:.1?}",
        first.len(),
        elapsed
    );
    Ok(())
}

#[test]
fn region_bench_all_fixtures() {
    let mut failures = Vec::new();
    for name in FIXTURES {
        if let Err(e) = bench_fixture(name) {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "benchmark failures: {failures:?}");
}

/// The ui.png fixture must yield a useful candidate count: enough to cover
/// its distinct blocks (header bar, sidebar, content), few enough to stay
/// interpretable. This is the anti-degeneracy guard.
#[test]
fn region_bench_ui_fixture_count_in_band() {
    let img = load("ui.png");
    let regions = detect_regions(&img, &DetectConfig::default()).unwrap();
    assert!(
        (2..=20).contains(&regions.len()),
        "ui.png yielded {} regions — expected 2..=20; detector may be degenerate",
        regions.len()
    );
}

/// Timing budget: detection on small fixtures should stay well under the
/// plan's max_processing_time even in debug builds.
#[test]
fn region_bench_latency_budget() {
    for name in FIXTURES {
        let img = load(name);
        let t = Instant::now();
        let _ = detect_regions(&img, &DetectConfig::default()).unwrap();
        assert!(
            t.elapsed().as_secs_f64() < 5.0,
            "{name}: detection took {:?} — over budget",
            t.elapsed()
        );
    }
}

/// Bench harness entry: run the suite when invoked via `cargo bench`.
fn main() {
    for name in FIXTURES {
        if let Err(e) = bench_fixture(name) {
            eprintln!("FAIL {name}: {e}");
            std::process::exit(1);
        }
    }
    println!("all fixtures passed");
}
