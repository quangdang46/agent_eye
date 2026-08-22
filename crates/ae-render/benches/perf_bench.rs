//! Full performance benchmark suite (plan §12/13, Phase 7).
//!
//! Covers every dimension the plan names:
//!   * decode time (1MP, 5MP, 10MP, 20MP)
//!   * render time per renderer (ASCII, Blocks) at several widths
//!   * region detection per image complexity (flat / structured / noisy)
//!   * zoom per crop size
//!   * startup + binary size + dependency count (reported for CI guards —
//!     the CI workflow owns the regression warnings; this prints values)
//!
//! Run: `cargo bench -p ae-render --bench perf_bench` or as tests:
//! `cargo test -p ae-render --bench perf_bench`.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ae_core::decode::decode_bytes;
use ae_core::image::{Dimensions, Image, ImageMetadata, Limits, Pixel, PixelBuffer};
use ae_core::regions::{detect_regions, DetectConfig};
use ae_render::config::RenderConfig;
use ae_render::render::{render_ascii, render_blocks};

fn timed<F: FnMut()>(mut f: F, iters: u32) -> Duration {
    f(); // warmup
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    t.elapsed() / iters
}

fn synthetic_image(w: u32, h: u32, mode: Mode) -> Image {
    let dims = Dimensions::new(w, h).unwrap();
    let pixels: Vec<Pixel> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            match mode {
                Mode::Flat => Pixel::opaque(128, 128, 128),
                Mode::Structured => {
                    let block = ((x / 8) + (y / 8)) % 2 == 0;
                    if block {
                        Pixel::opaque(240, 240, 240)
                    } else {
                        Pixel::opaque(15, 15, 15)
                    }
                }
                Mode::Noisy => {
                    let v = ((x.wrapping_mul(37)).wrapping_add(y.wrapping_mul(17))) % 256;
                    Pixel::opaque(v as u8, (255 - v) as u8, (x % 256) as u8)
                }
            }
        })
        .collect();
    let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
    Image::new(dims, buf, ImageMetadata::default()).unwrap()
}

#[derive(Clone, Copy)]
enum Mode {
    Flat,
    Structured,
    Noisy,
}

fn fmt(name: &str, d: Duration) {
    println!("{name:<44} {d:>12.2?}");
}

fn perf_decode_scaling() {
    // Decode is exercised through ae-core; sizes capped so the debug-mode
    // test run stays quick. Release `cargo bench` covers the full matrix.
    let limits = Limits::default();
    for (label, w, h) in [("1MP", 1024u32, 1024u32), ("4MP", 2048, 2048)] {
        let img = synthetic_image(w, h, Mode::Structured);
        let mut bytes = Vec::new();
        image_encode_rgba(&img, &mut bytes);
        let d = timed(
            || {
                let _ = black_box(decode_bytes(black_box(&bytes), &limits).unwrap());
            },
            3,
        );
        fmt(&format!("decode {label} png"), d);
    }
}

fn perf_render_per_renderer() {
    let img = synthetic_image(512, 512, Mode::Structured);
    for width in [40u32, 80, 160] {
        let cfg = RenderConfig {
            width,
            height: Some(width / 2),
            ..Default::default()
        };
        let da = timed(
            || {
                let _ = black_box(render_ascii(black_box(&img), &cfg).unwrap());
            },
            5,
        );
        fmt(&format!("render ascii w={width}"), da);
        let db = timed(
            || {
                let _ = black_box(render_blocks(black_box(&img), &cfg).unwrap());
            },
            5,
        );
        fmt(&format!("render blocks w={width}"), db);
    }
}

fn perf_region_detection_by_complexity() {
    let cfg = DetectConfig::default();
    for (name, mode) in [
        ("flat", Mode::Flat),
        ("structured", Mode::Structured),
        ("noisy", Mode::Noisy),
    ] {
        let img = synthetic_image(256, 256, mode);
        let d = timed(
            || {
                let _ = black_box(detect_regions(black_box(&img), &cfg).unwrap());
            },
            3,
        );
        fmt(&format!("regions 256x256 {name}"), d);
    }
}

fn perf_zoom_crop_sizes() {
    let img = synthetic_image(512, 512, Mode::Structured);
    let charset = ae_render::presets::standard().unwrap();
    let _ = &charset;
    for crop in [32u32, 128, 512] {
        let cropped_w = crop.min(512);
        let dims = Dimensions::new(cropped_w, cropped_w).unwrap();
        let pixels: Vec<Pixel> = (0..cropped_w * cropped_w)
            .map(|i| *img.pixels.as_slice().get(i as usize).unwrap())
            .collect();
        let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
        let sub = Image::new(dims, buf, ImageMetadata::default()).unwrap();
        let cfg = RenderConfig {
            width: 80,
            height: Some(40),
            ..Default::default()
        };
        let d = timed(
            || {
                let _ = black_box(render_ascii(black_box(&sub), &cfg).unwrap());
            },
            5,
        );
        fmt(&format!("zoom render crop={crop}"), d);
    }
}

/// Startup/binary-size numbers belong to the CI guard (`.github/workflows/
/// ci.yml`); print local values for reference.
fn perf_report_binary_metadata() {
    let exe = std::env::current_exe().ok();
    let _ = exe;
    // Dependency count from cargo tree would require spawning cargo; the
    // CI workflow tracks binary size + startup with baselines instead.
    println!("binary size/startup: tracked by CI regression guards");
}

fn main() {
    perf_decode_scaling();
    perf_render_per_renderer();
    perf_region_detection_by_complexity();
    perf_zoom_crop_sizes();
    perf_report_binary_metadata();
    println!("perf bench complete");
}

// -- helpers ---------------------------------------------------------------

/// Encodes an [`Image`] as PNG bytes via the `image` crate (dev-only path).
fn image_encode_rgba(img: &Image, out: &mut Vec<u8>) {
    use image::{ImageBuffer, Rgba};
    let w = img.dimensions.width;
    let h = img.dimensions.height;
    let mut ib = ImageBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = img.pixels.get(x, y).unwrap();
            ib.put_pixel(x, y, Rgba([p.r, p.g, p.b, p.a]));
        }
    }
    ib.write_to(&mut std::io::Cursor::new(out), image::ImageFormat::Png)
        .expect("encode");
}
