# Real-Image Validation Report

> Validated against actual test images from all 6 reference repos in `.tmp/`.
> Binary: `target/release/ae` 0.1.0, Apple M4, 2026-08-22.

## Decode matrix — 17/17 pass

| Source | Images | Result |
|--------|--------|--------|
| ASCII-generator | demo_image_complex.png (1600×900), demo_image_simple.png, input.jpg ×2 | ✅ decode + inspect |
| jp2a | 4 JPG grayscale tests (incl. 80×50, 320×240) | ✅ decode + inspect |
| chafa good/ | PNG alpha/no-alpha/indexed, CMYK JPEG, 1×1 pixel | ✅ decode + inspect |

## Render quality

* ASCII on real portrait (input.jpg, 976×538): facial structure and hair
  clearly resolved at 80 cols.
* Braille + dither on complex photo: high-detail dot texture.
* vs jp2a golden (`grind-2grayscale-fill.txt`, same charset, invert):
  row-mean luminance correlation **+0.813** — same bright/dark structure.
  Polarity difference is by design: jp2a assumes dark terminal background,
  `ae` uses a dark→light ramp.

## Edge cases (chafa bad suite)

| File | Expected | Actual |
|------|----------|--------|
| lodepng-zlib-big-alloc.png | rejected (bomb) | ✅ rejected, no crash |
| lodepng-zero-length-literal.png | rejected (malformed) | ✅ rejected, no crash |
| lodepng-adam7-mystery-over-read.png | decodes (valid 1×1) | ✅ decoded |

## Performance (release)

| Workload | Time |
|----------|------|
| render 1600×900 complex @80 cols | 10.3 ms ± 0.6 ms |
| render portrait 976×538 @80 cols | 5.6 ms ± 1.6 ms |
| render 320×240 → 663 cols dense | 5.6 ms ± 0.6 ms |
| inspect full pipeline end-to-end | ~16 ms |

## Region detection on real photos

demo_image_complex.png (1600×900): 9 candidate regions, 51 relations
(above ×33, left_of ×11, right_of ×7) — deterministic across runs.

Reproduce with:
```bash
cargo build --release -p ae-cli
for img in .tmp/*/tests/*.jpg .tmp/*/demo/*.png; do
  target/release/ae inspect "$img" --no-render --format json > /dev/null && echo "OK $img"
done
```
