//! Decode performance baseline (stable `cargo bench`, no nightly harness).
//!
//! Prints elapsed nanoseconds per decode for a few synthetic sizes so CI can
//! track regressions without external benchmark frameworks.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ae_core::{decode_bytes, Limits};

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        Rgba([
            ((x * 7) % 256) as u8,
            ((y * 11) % 256) as u8,
            (((x + y) * 3) % 256) as u8,
            255,
        ])
    });
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode fixture");
    buf
}

fn bench_decode(name: &str, bytes: &[u8], iters: u32) -> Duration {
    let limits = Limits::default();
    // warmup
    let _ = decode_bytes(bytes, &limits).expect("decode fixture");
    let started = Instant::now();
    for _ in 0..iters {
        black_box(decode_bytes(black_box(bytes), &limits).expect("decode fixture"));
    }
    let per_call = started.elapsed() / iters;
    println!("{name}: {per_call:?}/decode over {iters} iterations");
    per_call
}

fn main() {
    println!("ae-core decode bench");
    bench_decode("png_320x240", &png_bytes(320, 240), 50);
    bench_decode("png_1280x720", &png_bytes(1280, 720), 20);
    bench_decode("png_1920x1080", &png_bytes(1920, 1080), 10);
}
