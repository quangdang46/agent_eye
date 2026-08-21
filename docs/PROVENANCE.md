# PROVENANCE — Algorithm Source Tracking

> **Status:** Phase 0 deliverable (`agent_eye-w52`)
> **Rule (from COMPREHENSIVE_PLAN_FOR_AGENT_EYE.md Appendix A):** algorithm
> ideas are always portable; **code copying requires license compatibility**.
> When in doubt, reimplement independently.

## Policy

1. `ae` contains **zero lines copied** from any of the 6 reference repos.
2. Every algorithm below was studied at the *specification level* (what it
   computes, its math and edge cases) and then written fresh in Rust against
   the plan's contracts (determinism §5, limits §14, error model).
3. Where a reference implementation contained a defect, the defect is recorded
   so the rewrite does not reproduce it.
4. This file is append-only per phase: new studied algorithms get a row before
   the corresponding `ae` module lands.

Column semantics: **Copied code?** is `no` for every row by policy;
**Independently rewritten?** becomes `yes` when the corresponding `ae` bead
lands with its own tests. Rows marked *(planned)* are documented now, ahead of
implementation, because Phase-0 audits are complete.

## Matrix

| Source file | Algorithm studied | License | Copied code? | Independently rewritten? | Notes / defects avoided |
|---|---|---|:-:|:-:|---|
| pixel2ascii/src/image.rs `compute_block_size` | block sizing: `bw = img_w/ascii_w`, `bh = bw/aspect`, round+max(1) | MIT¹ | no | planned → `a3m` | unused `_img_height` param dropped; explicit guards added |
| pixel2ascii/src/image.rs `block_color` | per-block channel averaging, u32 sums, integer divide, edge clip | MIT¹ | no | planned → `a3m` | partial-border blocks tested explicitly |
| pixel2ascii/src/image.rs lum line 36 | Rec.709 luma 0.2126/0.7152/0.0722; invert `255-lum` | MIT¹ | no | planned → `e6s` | adopted as ae standard |
| pixel2ascii/src/ascii.rs `match_char` | index = `(lum/255·(n−1)).round()` | MIT¹ | no | planned → `tl7` | + explicit clamp (RASCII truncation lesson) |
| pixel2ascii/src/font.rs `compute_intensity` | font8x8 glyph ink intensity | MIT¹ | no | **skipped** | dead code upstream (never read); plan directive confirmed |
| ASCII-generator/img2txt.py `main` | grid slice + float mean + `int(mean·n/255)` clamp | MIT | no | planned → `tl7`/`a3m` | hardcoded `cell_height=2·cell_width` replaced by configurable aspect |
| ASCII-generator/utils.py `sort_chars` | empirical charset ordering by rendered glyph ink coverage, bucketed to ≤100 chars | MIT | no | planned → `6tk` | upstream slices brightness in fixed 10 px columns (misaligns vs real advance) — we measure actual glyph metrics |
| ASCII-generator/alphabets.py | multilingual ramp sets (12 langs) | MIT | no | planned → `0mx` | set contents re-derived, not transcribed |
| ascii-image-converter image_manipulation/util.go `resizeImage` | target-grid math incl. ×2/÷2 terminal compensation; braille ×2/×4 dot multiplier | Apache-2.0 | no | planned → `a3m`, ref `qeh` | resampler choice decided by our golden tests |
| ascii-image-converter image_manipulation/ascii_conversions.go `getBrailleChar` | braille bit layout `[4][2]{{1,8},{2,16},{4,32},{64,128}}` base 0x2800, threshold activation | Apache-2.0 | no | planned → `qeh` (P1) | layout is Unicode-standard fact; threshold semantics documented |
| ascii-image-converter util.go `ditherImage` | Floyd–Steinberg error diffusion, binary palette, braille-only scope | Apache-2.0 (idea from makeworld dither/v2, MPL-2.0 upstream lib) | no | planned → `01a` (P1) | we implement FS ourselves; no dither/v2 dependency |
| ascii-image-converter util.go `getColoredCharForTerm` | terminal color-level gating (truecolor→256 fallback) | Apache-2.0 | no | planned → `d5u` (P1) | implemented via owo-colors/anstyle |
| ascii-image-converter aic_package/winsize | TIOCGWINSZ ioctl incl. stdin-fallback fd | Apache-2.0/MIT (consolesize-go) | no | planned → `r4f` | std Rust `terminal_size` crate instead |
| chafa/internal/chafa-symbols.c | static 8×8 coverage-bitmap symbol tables; generated braille/block/legacy ranges | LGPL-3.0+ | no | n/a (concept only) | proves fontless rendering; ae charsets are own tables |
| chafa/internal/chafa-symbol-renderer.c `eval_symbol_error` | per-cell min-error symbol selection (+SIMD) | LGPL-3.0+ | no | ⛔ not adopted v1 | O(candidates·cells) unjustified for token-efficient evidence; revisit only if Tier-2 gap measured (`mq2`) |
| chafa/internal/chafa-dither.c | ordered Bayer / noise / diffusion dither modes | LGPL-3.0+ | no | planned → `01a` (P1, diffusion first) | matrix generation ours |
| chafa/internal/chafa-pixops.c `normalize_rgb` | histogram contrast stretch preprocessing | LGPL-3.0+ | no | deferred post-v1 | must stay behind explicit flag (predictability) |
| jp2a/src/aspect_ratio.c | 2.0/0.5 reciprocal constants for char-cell aspect | GPL-2.0-only | no | planned → `a3m` | constants are generic math; parameterized anyway |
| jp2a/src/options.c `precalc_rgb` | BT.601 weights as 256-entry LUTs | GPL-2.0-only | no | ⛔ coefficients rejected | ae standardizes Rec.709; LUT unneeded |
| jp2a/src/image.c pos/palette inversion | invert = reverse ramp index | GPL-2.0-only | no | planned → `ok2` | semantic idea only |
| jp2a term-fit / curl features | terminal auto-fit; URL fetch | GPL-2.0-only | no | `r4f` opt-in / ❌ URLs | ideas only (GPL wall) |
| RASCII/src/image_renderer.rs | thumbnail_exact resize; grapheme charsets; ×2 aspect derive | MIT | no | planned → `a3m`, `6tk` | max-luminance normalization rejected (config-dependent output) |
| RASCII/src/renderer.rs builder | consuming-builder config type | MIT | no | pattern noted → `ok2` | fields remain constructible directly |

¹ pixel2ascii: `license = "MIT"` in Cargo.toml; **no LICENSE file present**
(see LICENSE-MATRIX §4). We treat as MIT-per-declaration and do not copy text.

## Clean-room attestation

- Audits were performed by reading sources to extract *behavioral specs*,
  recorded in FEATURE-MATRIX.md.
- Implementations will be written against those specs plus the plan's formal
  contracts, with golden tests (`mq2`) defining behavior independently.
- For GPL-2.0 jp2a specifically: no source file was kept open during future
  implementation work; entries above contain everything any contributor needs.
