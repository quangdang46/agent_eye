# LICENSE-MATRIX

> **Status:** Phase 0 deliverable (`agent_eye-w52`)
> **Companion docs:** [`FEATURE-MATRIX.md`](FEATURE-MATRIX.md) · [`docs/PROVENANCE.md`](docs/PROVENANCE.md)
> **Effective date:** 2026-08-21 (SHA-pinned repo clones under `.tmp/`)

## Rule

`ae` redistributes no upstream source verbatim. Algorithm *ideas* remain
portable (ideas are not copyrightable); **code copying requires a compatible
license** or written permission. When in doubt, reimplement independently from
the spec recorded in `docs/PROVENANCE.md`.

---

## Matrix (audit-corrected)

| # | Repo (`.tmp/` dir) | Declared license(s) | Evidence examined | License—accurate, not planned | May copy lines into `ae` (MIT workspace)? | Study algorithms / write from spec? |
|---|---|---|---|---|---|---|
| 1 | `ASCII-generator` | MIT | `LICENSE` (Viet Nguyen, MIT text); `fonts/DejaVu Fonts License.txt` (Bitstream Vera/Arev — font files only) | **MIT** ✅ (code) + DejaVu/Bitstream (fonts) | **Yes** (MIT compatible) — *we still do not* | Yes |
| 2 | `ascii-image-converter` | Apache-2.0 | `LICENSE.txt` (176-line Apache 2.0); `cmd/root.go:176–178` runtime banner "Apache License Version 2.0" | **Apache-2.0** | **Yes** (Apache-2.0 compatible with MIT — keep `NOTICE` if any lines were copied; none are) | Yes |
| 3 | **`chafa`** | LGPL-3.0+ (library) / GPL-3 (tool) | `COPYING` = GPL-3 (2007-06-29); `COPYING.LESSER` = LGPL-3; `README:14` "library LGPLv3+"; every `chafa/*.c` header = "Lesser GPL" while `tools/chafa/*.c` headers carry the same Lesser text in this checkout but the *distributed binary* is under GPL per convention — lib vs tool split | **Split**: `libchafa` = **LGPL-3.0-*or-later***; `chafa` CLI binary = **GPL-3** per README/COPYING | lib: **⚠️ only via dynamic linking** (LGPL boundary); tool sources: **No** outright. We **avoid both** by studying papers/specs only and shipping no chafa code. | Yes |
| 4 | **`jp2a`** | GPL-2.0-**only** | `COPYING` = verbatim "Version 2, June 1991" (no "or later" anywhere); `LICENSES`; every `src/*.{c,h}` + `include/*.h` header "GPL v2" (no "or any later version"); `src/options.c:83` runtime string `"GPL v2"` | **GPL-2.0-only** — **no** "or later" upgrade path | **No** — copyleft wall closes the option entirely (even with relicense bumps) | Ideas only |
| 5 | `RASCII` | MIT | `LICENSE.md` = MIT ("KoBruh"); `Cargo.toml` `license = "MIT"` | **MIT** | **Yes** — still none copied | Yes |
| 6 | `pixel2ascii` | MIT (declared only) | `Cargo.toml:7` `license = "MIT"`; `README.md:148` "MIT © Sameer"; **no `LICENSE*` file** on disk (audit-verified) | **MIT-per-declaration** (no text file to verify) | Treat as MIT but **do not copy text**; no file means no one could rely on more than the metadata string — safe choice is rewrite | Yes |

---

## 1. ASCII-generator — fonts vs code

`fonts/` contains Bitstream Vera / DejaVu derivatives with their own permissive
license file (`fonts/DejaVu Fonts License.txt`). That license governs *the
`.ttf`/`.ttc` binaries*; it does not affect our charset strings or algorithms.
We re-derive ramp contents ourselves.

## 2. ascii-image-converter — Apache-2.0

Compatible with a MIT workspace. If we ever re-use a substantial excerpt
(e.g. a winsize snippet), we would retain the Apache `NOTICE` / header.
No such excerpt is planned; the doc lists provenance entries as specs, not
templates.

## 3. chafa — the split everyone mislabels ⚠️

The plan says "LGPL-3.0". The audit finds a **two-part** picture:

* `libchafa` (anything under `chafa/`, `libnsgif/`, `lodepng/`) — LGPL-3.0
  *or later* (LGPLv3+). LGPL would permit *dynamic linking* without opening
  our sources, but `ae` avoids the dependency entirely.
* `chafa` the *tool* (binary built from `tools/chafa/`) — GPL-3 per `COPYING`
  + packaging intent. Directly copying CLI sources would be GPL-3 infection.

`ae` course: **link against neither**. Behavior specs are extracted from the
feature/option text and docs (as done in FEATURE-MATRIX §4), not from inline
code keeping.

## 4. jp2a — GPL-2.0-only warning 🚧

Strongest copyleft in the set:

* `COPYING` contains exactly "Version 2, June 1991" — the later-clause is
  **absent**, so GPL version bumps do not apply automatically.
* Every translation unit carries `Distributed under the GNU General Public
  License (GPL) v2.` (no "or any later version").
* The executable itself announces `license = "GPL v2"` at `--help` time.

Effect: any line copied into `ae` would impose GPL-2.0-only on the whole
combined work, incompatible with shipping `ae` as MIT without relicensing.
We enforce: **zero jp2a lines enter this repo**, including via cherry-picks;
the 3 rows in `docs/PROVENANCE.md` carry only behavioral contracts (generic
math constants, option names) that are themselves unprotectable.

## 5. RASCII — MIT (clean)

MIT with a copyright line that doesn't match `Cargo.toml` authors (harmless).
Unmaintained dep `ansi_term 0.12` flagged upstream by the audit; `ae` replaces
it with owo-colors/anstyle per the plan dependency table.

## 6. pixel2ascii — "MIT with no file" 🟡

`Cargo.toml` and `README.md` both claim MIT, but no license text ships.
Policy: treat the declaration as intent to license MIT, but **do not** paste
any lines regardless; we re-implement from the written spec (block math, Rec.709,
ramp strings re-declared independently). This eliminates the need to settle
whether `Cargo.toml: license` alone satisfied MIT's "include this copyright
notice" clause.

---

## Contributor obligations — Phase-0 onward

1. **Consult this file + `docs/PROVENANCE.md` *before* opening any `.tmp/`
   source file.** Write against the specs, not the text.
2. **Do not vendor jp2a or chafa code.** Reviewers may reject PRs that import
   any `chafa/` header or any `jp2a/src/*.c` verbatim (+-20 char rule).
3. **Record every studied upstream file** in `docs/PROVENANCE.md` first
   (append-only), then implement.
4. **Keep `.tmp/` untracked.** Already covered by `.gitignore` → `.tmp/*`.
