# FEATURE-MATRIX — Reference Repos → agent-eye

> **Status:** Phase 0 deliverable (`agent_eye-w52`)
> **Basis:** Direct source audits of the 6 repositories cloned under `.tmp/`
> **Companion docs:** [`docs/PROVENANCE.md`](docs/PROVENANCE.md) · [`LICENSE-MATRIX.md`](LICENSE-MATRIX.md)

This document maps every function-level capability found in the 6 reference
repositories to its disposition in `ae`. It is the per-function expansion of the
summary matrix in `COMPREHENSIVE_PLAN_FOR_AGENT_EYE.md` §3.

**Legend**

| Symbol | Meaning |
|--------|---------|
| P0 | v1 core — must have |
| P1 | after v1 proven |
| P2 | later if needed |
| ❌ | explicitly out of scope |
| ⛔ | studied and **rejected** (with reason) |
| 💡 | lesson learned (adopted as design rule, not as code) |

Bead IDs refer to the beads tracker (`br show <id>`).

---

## 1. Cross-repo summary (audit-corrected)

| Feature | ASCII-gen (py) | ascii-conv (go) | chafa (c) | jp2a (c) | RASCII (rs) | p2a (rs) | agent-eye |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| Image decode | ✅ cv2 | ✅ imaging | ✅ own loaders | ✅ JPEG only | ✅ image crate | ✅ image crate | P0 (`lsd`) |
| Grayscale analysis | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 internal (`e6s`) |
| Grayscale presentation | ✅ raster | ✅ | ✅ | — | — | — | P0 flag (`e19`) |
| Plain-text color (raster) | ✅ | ✅ | ✅ | — | — | — | n/a (text-first tool) |
| Custom charset | ✅ mode arg | ✅ `--charset` | ✅ symbol map | ✅ `--charset` | ✅ `-C` | ✅ `--charset` | P0 (`6tk`) |
| Charset presets | ✅ simple/complex | ✅ simple/detailed | ✅ many tags | ✅ 1 palette | ✅ 6 sets | ✅ 3 ramps | P0 (`6tk`) |
| Aspect correction | ⚠️ hardcoded 2× | ✅ ×2/÷2 | ✅ `--font-ratio` | ✅ 2.0/0.5 consts | ✅ ×2.0 derive | ✅ `--aspect` cfg | P0 configurable (`a3m`) 💡 |
| Unicode blocks | — | — | ✅ | — | ✅ `BLOCK` | ✅ `"█▓▒░ "` | P0 (`77e`) |
| Braille | — | ✅ threshold+dither | ✅ generated syms | — | — (TODO.md) | — | P1 (`qeh`) |
| Dithering | — | ✅ FloydSteinberg | ✅ ordered/noise/diffusion | — | — | — | P1 (`01a`) |
| ANSI truecolor | — | ✅ level-detect | ✅ +256/16/FGBG | ✅ `--colors` (limited) | ✅ ansi_term | ✅ manual escapes | P1 compat (`d5u`) |
| Background color | ✅ bg_code | ✅ bg flag+alpha | ✅ FGBG modes | ✅ dark/light mnemonic | ✅ `--background` | ⚠️ computed, unused | P1 (`3w9`) |
| Flip X/Y | — | ✅ in-place | — | ✅ `--flipx` only | — | — | P1 |
| Invert | ✅ implicit order | ✅ negative flip | ✅ inverted tags | ✅ default ON | ✅ `--invert` | ✅ `255-lum` | P0 explicit flag (`ok2`) 💡 |
| stdin input | — | ✅ `-` + MIME sniff | ✅ | ✅ | — | — | P0 (`mms`,`lsd`) |
| URL input | — | ✅ http.Get | — | ✅ FEAT_CURL | — | — | ❌ |
| File output | ✅ txt/jpg/video | ✅ txt/png/gif | — | ✅ `--output=` | — | ⚠️ flag unread (bug) | P0 (`jpn`) |
| Save PNG/GIF | ✅ PIL render | ✅ gg+freetype | — | — | — (empty gif_renderer) | — | ❌/❌ |
| Video / animated GIF | ✅ XVID | ✅ goroutine frames | — | — | ⚠️ empty module | ✅ frame loop | ❌ |
| Terminal fit | — | ✅ winsize ioctl | ✅ TIOCGWINSZ | ✅ FEAT_TERMLIB auto | — | — | P0 opt-in (`r4f`) |
| Multilingual charsets | ✅ 12 languages | — | ✅ kana/latin tables | — | ✅ CJK/Cyrillic | — | P1 (`0mx`) |
| Batch processing | — | ✅ multi-arg loop | — | — | — | — | P1 (`4p6`) |
| Library API | — | ✅ `Convert()` | ✅ libchafa | — | ✅ lib+bin | ✅ (parallel impl) | P0 (`jum`+) |
| Region detection | — | — | — | — | — | — | P0 heuristic (`aya`→`ia5`) |
| Spatial relations / provenance / JSON / coordinate mapping | — | — | — | — | — | — | **unique to ae** |

Corrections vs. plan §3 discovered during audit:

1. **chafa licensing is two-part**: libchafa = LGPL-3.0-or-later; the `chafa`
   CLI binary = GPL-3. Plan said "LGPL-3.0" flatly. See LICENSE-MATRIX.
2. **jp2a is GPL-2.0-*only*** (no "or later") — confirmed in `COPYING`, every
   file header, and the runtime string in `options.c:83`.
3. **pixel2ascii declares MIT in Cargo.toml but ships no LICENSE file** —
   usable as MIT per declaration, but we do not copy text regardless.
4. **jp2a defaults `invert = 1`** (dark-background assumption). `ae` inverts
   this philosophy: plain-text agents always get explicit, light-ramp output;
   `--invert` must be deliberate (`ok2`).
5. **aic has no blocks charset** — plan already marked this correctly.
6. **Grayscale coefficients differ across repos** (see §8); `ae` standardizes
   on Rec. 709 internally (plan §7).

---

## 2. ASCII-generator (Python · MIT · `.tmp/ASCII-generator`)

Upstream: vietnh1009/ASCII-generator. Files audited: `img2txt.py`,
`img2img.py`, `img2img_color.py`, `video2video*.py`, `utils.py`,
`alphabets.py`.

| Upstream function / mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| `img2txt.main(opt)` | slice image into `cell_width × cell_height` blocks, mean luminance → char index `int(mean*n/255)` clamped | Core block-sampling shape re-derived in Rust; index formula kept equivalent | `tl7` |
| hardcoded `cell_height = 2*cell_width` (all 5 scripts) | terminal aspect guess baked in | 💡 rejected as hardcode; `ae` exposes `aspect_ratio: f32` (default 0.5) | `a3m` |
| fallback `cell_width=6, cell_height=12` when `num_cols > width` | guard compares wrong quantities (chars vs pixels) | 💡 lesson: validate against `block_w ≥ 1` and reject early with typed error | `l7p`, `jod` |
| `np.mean(block)` float mean over BGR-gray | block averaging | Reimplemented as integer-accurate sum/divide over RGBA with edge clipping | `a3m` |
| `utils.sort_chars(char_list, font, language)` | renders glyphs via PIL, ranks ink coverage, buckets into ≤100 chars | Adopted as *idea* for empirical charset ordering; fix the hardcoded 10 px column-slice bug by measuring real glyph advance | `6tk` |
| `utils.get_data(language,mode)` + `alphabets.py` (12 languages incl. CJK, Cyrillic, diacritics) | multilingual ramps + per-language font metrics | P1 multilingual presets; per-width-class handling instead of single scale | `0mx` |
| `img2img_color.partial_avg_color` | per-cell average RGB drawn into raster | Not needed for text evidence; superseded by truecolor flag (P1) | `d5u` |
| `video2video*.py`, `overlay_ratio`, XVID fourcc | video pipeline + PiP overlay | ❌ out of scope | — |
| bare `try: out except:` lazy-init idiom | fragile init swallowing decoder errors | 💡 anti-pattern noted; `ae` uses typed `AeError`, never swallows | `l7p` |

---

## 3. ascii-image-converter (Go · Apache-2.0 · `.tmp/ascii-image-converter`)

Upstream: TheZoraiz/ascii-image-converter v1.13.1.

| Upstream function / mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| `cmd/root.go` cobra flag set | ~20 flags incl. `--width/--height/--dimensions`, `--color`, `--negative`, `--flipX/Y`, `--braille`, `--dither`, `--threshold`, `--bg-color`, `--font-color`, `--save*, --full, --complex` | UX vocabulary cross-checked; `ae` splits axes renderer/format per plan §6 | `u2d` |
| `cmd/util.checkInputAndFlags` | validation + defaults injection (`saveBgColor=[0,0,0,100]`, `threshold=128`) | Validation style adopted (fail fast, typed errors); defaults live in one place | `l7p`, `ok2` |
| `aic_package.Convert(filePath, flags)` + `DefaultFlags()` | public library entry point | Confirms lib+CLI split is viable; `ae` puts logic in `ae-core`/`ae-render`, CLI orchestrates only | `nbr`, `jum` |
| stdin: arg `-` → `ioutil.ReadAll(os.Stdin)` + `http.DetectContentType` MIME sniff | piped binary input | Adopted: `-` means stdin; format detection by magic bytes, not extension | `lsd`, `mms` |
| URL: `isURL()` → `http.Get` | remote fetch | ❌ out of scope (offline guarantee) | — |
| `image_manipulation.resizeImage()` | target grid math incl. ×2/÷2 terminal compensation, `imaging.Resize(...Lanczos)`; braille ×2/×4 multiplier | 💡 aspect model confirmed; resampling choice deferred (nearest/bilinear decided in `a3m` tests); braille dot-multiplier pattern reused if `qeh` passes benchmark | `a3m`, `qeh` |
| `color.GrayModel.Convert` (≈0.299/0.587/0.114) + `/257` scaling, `charDepth := r1/257` | Rec.601 luma | ⛔ Rec.601 rejected; `ae` uses Rec.709 (0.2126/0.7152/0.0722) per plan §7 | `e6s` |
| `ascii_conversions.go`: `asciiTableDetailed` (70-char), `asciiTableSimple` `" .:-=+*#%@"` | ramp presets | Preset vocabulary adopted (`standard` ≈ simple, `dense` ≈ detailed); exact strings ours | `6tk` |
| `ConvertToBrailleChars` / `getBrailleChar` | `BrailleStruct=[4][2]int{{1,8},{2,16},{4,32},{64,128}}`, base `0x2800`, dot set iff `depth >= threshold` (or `<=` negated) | Reference for P1 braille: bit layout + threshold semantics documented for `qeh` | `qeh` |
| `util.ditherImage()` | `makeworld-the-better-one/dither/v2`, palette {black,white}, `FloydSteinberg`; applied only for braille+dither | P1 dithering: algorithm name + scoping (binary targets only) adopted | `01a` |
| `flipX` per-row swap, `flipY` row reverse | flips | P1; trivial post-transform ops | — |
| `getColoredCharForTerm()` | `gookitColor.TermColorLevel()` → millions/hundreds gating; `.C256()` downgrade; fg/bg/font-color variants | 💡 terminal-capability detection pattern for P1 truecolor; `ae` will use owo-colors/anstyle (ansi_term unmaintained — RASCII confirms why) | `d5u` |
| `convert_root.go` mapping `value/MAX_VAL*len(table)` clamp-at-max, negative flip | char indexing | Equivalent formula verified against `tl7` spec; clamp made explicit | `tl7` |
| `winsize.GetTerminalSize()` unix ioctl `TIOCGWINSZ` on **stdin** when stdout not TTY | terminal fit plumbing | Adopted detail: query stdin as fallback fd | `r4f` |
| `pathIsGif()` goroutine batches capped by `runtime.NumCPU()` | parallel frames | ❌ video out of scope; parallelism policy for `ae` stays "scalar first, profile then rayon" (plan §5 determinism) | — |
| `flattenAscii`, `saveAsciiArt`, `createSaveFileName` | save naming `<name>-ascii-art.txt/png/gif` | File output naming convention noted | `jpn` |
| `clearScreen()` GOOS map + `\x1b[2J\x1b[H` playback | animation loop | ❌ out of scope | — |

---

## 4. chafa (C · libchafa LGPL-3.0+ / tool GPL-3 · `.tmp/chafa`)

Upstream: hpjansson/chafa (working clone). `ae` studies algorithms only —
no linking, no copying (LGPL boundary respected; see LICENSE-MATRIX).

| Upstream function / mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| `internal/chafa-symbols.c` | static 8×8 coverage bitmaps (64-bit words), popcount index, `generate_braille_syms` U+2800–28FF, Block Elements, Legacy Computing U+1CD00–1D000/U+1FB80/U+1FBE6, ascii/kana/latin tables | 💡 canonical proof that glyph = fixed bitmap enables pure-computation rendering without font deps — matches `ae`'s offline/no-FontConfig stance | `77e`, `qeh` |
| `internal/chafa-symbol-renderer.c`: `eval_symbol_error` / `calc_cell_error_plain` (+AVX2/SSE41) | per-cell best-symbol search minimizing bitmap-vs-cell error; wide-cell variant | ⛔ **not adopted for v1**: error-minimization needs 60+ candidate symbols per cell; luminance-mapping (1 symbol/cell) is O(n) and sufficient for token-efficient evidence. Documented as future option if Tier-2 quality gap appears | `mq2` (measure) |
| `ChafaWorkCell` cached sorted pixels / dominant channel | per-cell preprocessing cache | 💡 caching pattern noted for region metrics | `r9o` |
| `CHAFA_SYMBOL_TAG_*` taxonomy | user-selectable symbol classes | Vocabulary informs charset preset naming | `6tk` |
| Canvas modes TRUECOLOR→INDEXED_16→FGBG_BGFG→FGBG; sixel/kitty/iterm2 renderers (`chafa-{sixel,kitty,iterm2}-renderer.c`, `chafa-term-db.c`) | output targets | ❌ terminal-graphics protocols out of scope (text evidence only); color-mode ladder reused conceptually for P1 `d5u` | `d5u` |
| `internal/chafa-dither.c`: ORDERED (Bayer `chafa_gen_bayer_matrix`), NOISE (`chafa-noise.c`), DIFFUSION | dither family | P1: offer diffusion first (matches aic), keep ordered/noise as stretch | `01a` |
| `internal/chafa-pixops.c`: `normalize_rgb` histogram stretch + `boost_saturation_rgb`; config default preprocessing ON | automatic contrast/saturation boost | 💡 strong idea for legibility; deferred behind explicit flag so output stays predictable (determinism contract `hps`) | — (post-v1) |
| `tools/chafa/chicle-options.c parse_font_ratio_arg` + `virt_src_height /= font_ratio` (chafa.c:754) | aspect correction CLI-side, optional ioctl pixel measurement | Confirms ratio belongs at presentation layer; `ae` bakes default 0.5, exposes flag | `a3m` |
| Deterministic batch splitting (`internal/chafa-batch.c`), no RNG in selection | reproducible output despite work partitioning | 💡 validates plan rule: partitioning must not affect ordering | `hps` |
| FreeType loading only via CLI (`chicle-font-loader.c`) feeding `chafa_symbol_map_add_glyph` | optional font import | ⛔ no font dependency in `ae` v1; font rendering stays P2/out | — |

---

## 5. jp2a (C · GPL-2.0-**only** · `.tmp/jp2a`)

Upstream: cslarsen/jp2a 1.0.8. **License wall: zero code may be copied.**
Algorithms/ideas only (ideas are not copyrightable). Clean-room rule: our Rust
implementations are written from the written specification in this repo, not
from reading its sources line-by-line. See LICENSE-MATRIX §2.

| Upstream mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| `src/image.c decompress/process_scanline` (libjpeg, rejects non-8-bit precision via `exit(1)`) | streaming scanline conversion | Streaming decode idea noted; `ae` decodes whole bounded buffer (limit-checked first, `jod`) | `lsd`, `jod` |
| `src/aspect_ratio.c`: `CALC_WIDTH = ROUND(2.0f*h*w/H)`, `CALC_HEIGHT = ROUND(0.5f*w*H/w)` | char-cell 2:1 compensation constants | Third independent confirmation of 0.5 default; `ae` parameterizes it | `a3m` |
| `options.c:69` `ascii_palette = "   ...',;:clodxkO0KXNWM"` (cap 256, `--charset` replaces) | dark→light ramp | Ramp content NOT copied; `ae` defines own presets | `6tk` |
| `redweight=0.2989 greenweight=0.5866 blueweight=0.1145`, `precalc_rgb()` 256-entry LUTs, `--red/--green/--blue` overrides | BT.601 with lookup tables + tunable weights | ⛔ coefficients differ from our Rec.709 standard; LUT micro-opt unnecessary at v1 scale | `e6s` |
| `int pos = ROUND((float)chars * (Y_inv ? : Y))`, `palette[invert ? pos : chars-pos]` (image.c:94,185) | index math + inversion by ramp reversal | Same inversion semantics chosen: reverse index, don't remap pixels | `tl7`, `ok2` |
| `invert = 1` default; `--background=dark/light` mnemonics | assumes dark terminals by default | 💡 inverted philosophy: `ae` default = explicit light ramp for agents; no hidden polarity | `ok2` |
| `--flipx` (`options.c:201`, applied image.c:185) | horizontal flip | P1 parity item | — |
| FEAT_TERMLIB term-fit auto ("default mode is --term-fit --background=dark"), `term.c` | auto-size to terminal | Adopted as opt-in `--full` only; agents must not get surprise sizes | `r4f` |
| FEAT_CURL `curl.c` URL download | remote input | ❌ out of scope | — |
| `html.c` XHTML 1.0 Strict output | markup wrapper | ❌ rejected — JSON covers machine consumption better | `cum` |
| custom `IF_OPTS` parser, `fileout = "-"` stdout default | Unix filter conventions | 💡 `-` conventions adopted for both stdin (`mms`) and `--output -` | `mms`, `jpn` |

---

## 6. RASCII (Rust · MIT · `.tmp/RASCII`)

Upstream: orhnk/RASCII (crate `rascii_art` 0.4.5).

| Upstream mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| lib (`src/lib.rs`: `render`, `render_image`, `render_to`) + bin (`main.rs`) | library+CLI single crate | Shape validated; `ae` goes further: separate `ae-core`/`ae-render`/`ae-cli` crates | `nbr` |
| `RenderOptions` consuming builder (`.width/.height/.colored/.background/.invert/.charset`) | config type | Builder ergonomics noted; fields stay `pub` so callers can construct literally (their own main.rs does this — builder is dead weight) | `ok2` 💡 |
| `DynamicImage::thumbnail_exact(w,h)` | resize to exact grid | Candidate for `a3m` resampling comparison set (vs hand block-mean) | `a3m` |
| `get_grayscale` BT.601 `/255.0`, then **normalize by image max luminance** | relative scaling | ⛔ rejected: max-normalization makes output depend on brightest pixel → same-config images produce different ramps; violates absolute-luminance determinism goal | `e6s`, `hps` |
| `(gray*(len-1)) as usize`, invert via `len-1-idx` | truncating index cast | 💡 lesson: use `.round()` then clamp (truncation biases toward dark end) | `tl7` |
| `charset: &'a [&'a str]` + `UnicodeSegmentation::graphemes` for custom sets | multi-codepoint emoji-safe charsets | Adopted directly as requirement (plan §7 "lessons from repos") | `6tk` |
| `charsets.rs`: BLOCK, CHINESE, DEFAULT, EMOJI, RUSSIAN, SLIGHT; `from_str` lookup | preset registry | Naming/lookup pattern adopted | `6tk`, `0mx` |
| `ansi_term 0.12` styling | color output | Crate itself is unmaintained → `ae` picks owo-colors/anstyle (plan dependency table) | `d5u` |
| aspect ×2.0 factor when deriving missing dimension, `ceil` | terminal compensation | Consistent with other repos; folded into `a3m` | `a3m` |
| Zero tests; empty `gif_renderer.rs`; duplicated `render_to`/`render`; `Option::expect` panics; README/help say default 128 but code forces 80 | hygiene gaps | 💡 each gap maps to an explicit `ae` counter-requirement: golden tests, no stub modules, single render path, typed errors, help-truth CI | `mq2`, `llg`, `l7p` |

---

## 7. pixel2ascii (Rust · MIT declared, no LICENSE file · `.tmp/pixel2ascii`)

Upstream: SameerVers3/pixel2ascii 0.1.1. Primary algorithm baseline for `ae`
(plan §7 credits it as such).

| Upstream mechanism | What it does | agent-eye disposition | Bead |
|---|---|---|---|
| `compute_block_size(img_w, _img_h, ascii_w, aspect)`: `block_w = img_w/ascii_w`, `block_h = block_w/aspect`, `.max(1).round()` | grid sizing | Adopted formula (with `_img_height` smell removed — height participates via row stepping only); explicit overflow/zero guards added | `a3m` |
| `block_color`: nested loop, u32 channel sums, integer-divide average, edge blocks clipped to bounds | block averaging | Adopted with edge-clipping behavior preserved + tested (partial blocks at borders) | `a3m` |
| `lum = 0.2126*r + 0.7152*g + 0.0722*b`; invert = `255.0-lum` | Rec.709 luminance | **Adopted verbatim as the standard** (only repo using Rec.709) | `e6s` |
| `match_char`: `((lum/255)*(len-1)).round()` | index mapping | Adopted; plus explicit clamp for safety | `tl7` |
| ramps `"@%#*+=-:. "` / `"@M#W$9876543210?!abc;:+=-,._ "` / `"█▓▒░ "` | preset strings (duplicated in cli.rs AND convert.rs) | Presets re-declared once in `ae-render`; duplication noted as anti-pattern | `6tk` |
| `sample_image_blocks`: rows `par_iter`, sequential columns, `collect::<Vec<Vec<_>>>` | rayon row parallelism, order-stable | Policy: start scalar; rayon permitted later **iff** golden tests prove byte-identical output (plan determinism contract) | `tl7`, `hps` |
| `font.rs build_charset/compute_intensity` — intensity computed, stored, **never read** (match_char uses only `.ch`) | dead font-intensity pipeline; also drops unknown glyphs with stderr warn | 💡 plan directive confirmed by audit: skip font8x8 entirely | — (skipped) |
| `rusttype` declared, imported nowhere | dead dependency | 💡 counter-example for dependency hygiene; `ae` prunes deps each phase | `kgp` |
| `cli.rs Cli::validate() -> Result<(),String>`: width≠0, width≤5000, aspect>0, custom charset≥2 glyphs, ArgGroup mutual exclusion | upfront validation | Pattern adopted into typed errors; limits raised per plan §14 (10k) | `jod` |
| `-o/--output` parsed but **never read**; `use_background` computed but never passed | advertised features silently missing | 💡 drives `capabilities` command honesty + integration tests asserting every flag has observable effect | `hh3`, `mq2` |
| `unwrap()` panics across decode/IO | crash-style errors | Counter-pattern: `AeError` everywhere, no panics on malformed input (fuzz target) | `l7p`, `g4j` |
| lib API (`convert.image_to_ascii`) exists but binary bypasses it | parallel implementations drift | 💡 CLI must orchestrate through the same public lib path | `nbr`, `u2d` |

---

## 8. Grayscale coefficient survey (drives `e6s` decision)

| Repo | Coefficients | Standard |
|---|---|---|
| ascii-image-converter | 0.299/0.587/0.114 (Go GrayModel) | BT.601 |
| jp2a | 0.2989/0.5866/0.1145 (tunable) | BT.601 |
| RASCII | 0.299/0.587/0.114 (+max-normalize) | BT.601 |
| pixel2ascii | **0.2126/0.7152/0.0722** | **BT.709** |
| ASCII-generator | cv2 `COLOR_BGR2GRAY` | BT.601 |
| chafa | per-channel sort/error, no single luma | n/a |

**Decision:** `ae` computes Rec.709 internally everywhere (`e6s`), matching
the plan and pixel2ascii; presentation-time grayscale (`e19`) is a separate,
explicit mode.

## 9. Coverage checklist (Phase-0 acceptance)

- [x] All 6 repos cloned under `.tmp/` (shallow, HEAD pinned)
- [x] Every repo source-audited; findings above cite file/function names
- [x] Per-function disposition assigned (P0/P1/P2/❌/⛔/💡) with bead refs
- [x] License corrections propagated to LICENSE-MATRIX.md
- [x] Algorithm provenance recorded in docs/PROVENANCE.md
