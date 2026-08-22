//! Golden test suite: fixture PNGs × renderers × configs compared against
//! stored golden files (plan §13 "Golden test fixtures").
//!
//! Goldens live in `tests/golden/<fixture>.<renderer>.<config>.txt`. A
//! renderer change shows up as an exact diff against these files. Regenerate
//! intentionally with `REGENERATE_GOLDEN=1 cargo test -p ae-render --test
//! golden_suite` and review the diff before committing.
//!
//! Determinism contract is also asserted here: every case runs 10× and must
//! be byte-identical.

use ae_render::config::RenderConfig;
use ae_render::Charset;

use ae_core::decode::decode_bytes;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURES: &[&str] = &["simple-box.png", "checkerboard.png", "gradient.png"];

struct GoldenCase {
    /// File suffix, e.g. `ascii.default`.
    name: &'static str,
    config: RenderConfig,
}

fn cases() -> Vec<GoldenCase> {
    vec![
        GoldenCase {
            name: "ascii.default",
            config: RenderConfig::ascii(40),
        },
        GoldenCase {
            name: "blocks.default",
            config: RenderConfig::blocks(40),
        },
        GoldenCase {
            name: "ascii.square",
            config: RenderConfig::ascii(32).without_aspect(),
        },
        GoldenCase {
            name: "ascii.inverted",
            config: RenderConfig::ascii(24).with_invert(),
        },
        GoldenCase {
            name: "ascii.dense",
            config: RenderConfig {
                charset_override: Some("dense".into()),
                ..RenderConfig::ascii(48)
            },
        },
    ]
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn golden_path(fixture_stem: &str, case_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{fixture_stem}.{case_name}.txt"))
}

fn load_fixture(name: &str) -> ae_core::image::Image {
    let bytes =
        fs::read(fixture_path(name)).unwrap_or_else(|e| panic!("fixture {name} missing: {e}"));
    decode_bytes(&bytes, &ae_core::image::Limits::default()).expect("fixture decodes")
}

fn render_to_text(img: &ae_core::image::Image, cfg: &RenderConfig) -> String {
    let charset: Charset = cfg.resolve_charset().expect("charset resolves");
    let grid = ae_render::render::render(img, cfg, &charset).expect("render succeeds");
    grid.to_text()
}

#[test]
fn golden_outputs_match() {
    let regenerate = std::env::var("REGENERATE_GOLDEN").is_ok();
    for fixture in FIXTURES {
        let img = load_fixture(fixture);
        let stem = fixture.trim_end_matches(".png");
        for case in cases() {
            let text = render_to_text(&img, &case.config);
            let path = golden_path(stem, case.name);
            if regenerate {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, &text).unwrap();
                continue;
            }
            // Read as bytes and normalize CRLF: git may check the goldens
            // out with Windows line endings on windows-latest runners.
            let golden = {
                let raw = fs::read(&path).unwrap_or_else(|e| {
                    panic!(
                        "golden {} missing ({e}); run REGENERATE_GOLDEN=1 and review",
                        path.display()
                    )
                });
                String::from_utf8(raw)
                    .unwrap_or_else(|e| panic!("golden {} not UTF-8: {e}", path.display()))
                    .replace("\r\n", "\n")
            };
                panic!(
                    "golden {} missing ({e}); run REGENERATE_GOLDEN=1 and review",
                    path.display()
                )
            });
            assert_eq!(
                text, golden,
                "golden drift for {stem}/{}. If intentional, regenerate + review.",
                case.name
            );
        }
    }
}

#[test]
fn deterministic_across_ten_runs() {
    for fixture in FIXTURES {
        let img = load_fixture(fixture);
        for case in cases() {
            let first = render_to_text(&img, &case.config);
            for run in 1..10 {
                let again = render_to_text(&img, &case.config);
                assert_eq!(
                    first, again,
                    "{fixture}/{}: run {run} diverged — determinism contract broken",
                    case.name
                );
            }
        }
    }
}

#[test]
fn fixtures_exist_and_decode() {
    for fixture in FIXTURES {
        let img = load_fixture(fixture);
        assert!(img.dimensions.width > 0 && img.dimensions.height > 0);
    }
}
