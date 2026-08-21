# ae — Agent-Eye

<div align="center">
  <img src="ae_illustration.webp" alt="ae — Give text-only AI agents eyes">
</div>

<div align="center">

![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue.svg)
![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)

</div>

**Give text-only AI agents eyes — without a vision model.**

`ae` is a Rust-native CLI that converts pixels into deterministic visual evidence: regions, geometry, spatial relations, and structured representations. No LLM, no API keys, no cloud. The agent interprets; `ae` provides evidence.

```text
                    IMAGE
                      │
                      ▼
               ┌─────────────┐
               │      ae     │
               │   Rust CLI  │
               └──────┬──────┘
                      │
            compact evidence
                      │
                      ▼
               TEXT-ONLY AGENT
```

---

## Quick Start

```bash
# Install (from source)
git clone https://github.com/quangdang46/agent_eye
cd agent_eye
cargo build --release
./target/release/ae --help
```

### Agent workflow — progressive inspection

```bash
# Step 1: overview
ae inspect screenshot.png --format json

# Step 2: agent decides to look at region r3
ae zoom screenshot.png --region r3 --format json

# Step 3: spatial evidence
ae geometry screenshot.png --format json

# Step 4: targeted crop
ae region screenshot.png r3 --format json
```

Agent drives the perception loop. `ae` provides primitives.

---

## The Problem

Text-only LLMs can't see images. When a coding agent encounters a screenshot, a UI mockup, or a diagram, it's blind. The common workaround — shoving raw ASCII into context — wastes tokens on flat regions while losing spatial structure.

**What's actually needed:**

- **Where** things are (geometry, not just luminance)
- **How regions relate** (left_of, above, inside — formal math, not guesses)
- **Progressive inspection** (overview → zoom into uncertain regions)
- **Provenance** (every representation maps back to original pixel coordinates)

## The Solution

`ae` is a **visual evidence layer** — not another ASCII converter.

```bash
ae inspect screenshot.png
# → 3 regions detected, spatial relations, geometry, ASCII overview

ae zoom screenshot.png --region r3
# → targeted crop + resample at 4x scale

ae geometry screenshot.png --format json
# → formal spatial relations with coordinate mapping
```

Every output includes **provenance**: source hash, original bounds, affine coordinate transform. Agent never loses connection to the source image.

---

## Why `ae`?

| Feature | ASCII-only tools | `ae` |
|---------|:----------------:|:----:|
| Grayscale rendering | ✅ | ✅ |
| Unicode blocks | ❌ | ✅ |
| Aspect correction | ⚠️ hardcoded | ✅ configurable |
| Region detection | ❌ | ✅ heuristic geometric |
| Spatial relations | ❌ | ✅ 7 formal relations |
| Coordinate mapping | ❌ | ✅ affine transform |
| Provenance tracking | ❌ | ✅ every output |
| Progressive inspection | ❌ | ✅ agent-driven |
| JSON output | ❌ | ✅ versioned schemas |
| Deterministic | ⚠️ varies | ✅ contract |
| No LLM required | ✅ | ✅ |

---

## Comparison

| Tool | Language | Regions | Relations | Provenance | JSON | Agent-native |
|------|----------|:-------:|:---------:|:----------:|:----:|:------------:|
| `ae` | Rust | ✅ | ✅ | ✅ | ✅ | ✅ |
| ASCII-generator | Python | ❌ | ❌ | ❌ | ❌ | ❌ |
| ascii-image-converter | Go | ❌ | ❌ | ❌ | ❌ | ❌ |
| pixel2ascii | Rust | ❌ | ❌ | ❌ | ❌ | ❌ |
| chafa | C | ❌ | ❌ | ❌ | ❌ | ❌ |

`ae` is not a replacement for these tools. It's a different category: **visual evidence for agents**, not terminal art for humans.

---

## Commands

```bash
ae <command> <image> [options]
```

| Command | Purpose | Example |
|---------|---------|---------|
| `inspect` | Orchestrated overview | `ae inspect photo.png` |
| `render` | ASCII/blocks rendering | `ae render photo.png --renderer ascii --width 100` |
| `region` | Extract specific area | `ae region photo.png r3` |
| `zoom` | Targeted crop + resample | `ae zoom photo.png --region r3 --level 2` |
| `geometry` | Spatial evidence | `ae geometry photo.png --format json` |
| `capabilities` | Feature discovery | `ae capabilities --format json` |

### Options

| Flag | Values | Default | Description |
|------|--------|---------|-------------|
| `--format` | `text`, `json` | `text` | Output serialization |
| `--renderer` | `ascii`, `blocks` | `ascii` | Rendering engine |
| `--width` | `N` | `100` | Output width in characters |
| `--region` | `id` | — | Target specific region |
| `--box` | `x,y,w,h` | — | Target bounding box |
| `--level` | `0-3` | `0` | Zoom scale (1×, 2×, 4×, 8×) |
| `--invert` | — | off | Invert luminance |
| `--charset` | `string` | — | Custom character ramp |
| `--output` | `file` | stdout | Write to file |
| `--quiet` | — | off | Suppress diagnostics |

---

## Design Principles

| Principle | Detail |
|-----------|--------|
| **Evidence, not interpretation** | `ae` provides geometric candidates, not semantic labels. The agent reasons. |
| **Deterministic** | Same input + same config = identical output, every run. No randomness. |
| **Offline** | No LLM, no API keys, no cloud inference. `cargo install agent-eye` works offline. |
| **Provenance** | Every output maps back to original pixels via affine transform. |
| **Progressive** | Overview → region → zoom → answer. Agent drives the inspection loop. |
| **Bounded** | Resource limits on input, output, regions, and relations prevent abuse. |

---

## Architecture

```text
ae-cli (orchestration)
  │
  ├── ae-core
  │     ├── image (decode, pixel buffer)
  │     ├── geometry (spatial relations)
  │     ├── sampling (block averaging)
  │     ├── transforms (resize, crop)
  │     ├── analysis (luminance, edges)
  │     ├── regions (candidate segmentation)
  │     ├── provenance (source tracking)
  │     └── mapping (affine coordinate transform)
  │
  └── ae-render
        ├── ASCII renderer
        ├── blocks renderer
        └── charset system
```

Dependency flows one way. Core does not know about CLI. No LLM dependency anywhere.

---

## JSON Output

All commands support `--format json`. Every output includes `schema_version` for forward compatibility.

```json
{
  "schema_version": "agent-eye.scene.v1",
  "image": { "width": 1440, "height": 900 },
  "regions": [
    { "id": "r1", "bounds": [0, 0, 1440, 96], "area": 0.07, "edge_density": 0.42 }
  ],
  "relations": [
    { "type": "left_of", "a": "r2", "b": "r3" }
  ],
  "representation": { "renderer": "ascii", "data": "..." },
  "mapping": {
    "source_bounds": [0, 0, 1440, 900],
    "scale_x": 18.0, "scale_y": 16.67,
    "offset_x": 0.0, "offset_y": 0.0
  }
}
```

No semantic labels. No `importance`, `confidence`, `description`. Pure geometric evidence.

---

## Benchmark Philosophy

> **Don't benchmark to prove `ae` is good.** Design benchmarks that can also prove `ae` is NOT useful.

If `ae + agent ≈ ASCII + agent`, cut agent features, keep rendering only.

The key metric: **task accuracy per unit of context + interaction cost**.

```text
LLM alone:              baseline
LLM + fixed ASCII:      X% accuracy / N tokens
LLM + ae inspect:       Y% accuracy / N tokens
LLM + ae progressive:   Z% accuracy / N tokens
Vision model:           upper bound
```

---

## Limitations

- **Region detection is heuristic.** Regions are geometric candidates, not semantic objects. The same algorithm may produce different valid segmentations for different images.
- **No OCR in core.** Text extraction requires an external adapter (e.g., Tesseract). `ae` provides geometry; the agent decides what to read.
- **No semantic labels.** `ae` will never say "this is a button." It provides edge density, bounding boxes, and coordinates. The agent interprets.
- **v1 = ASCII + Blocks.** Braille, dithering, ANSI color are P1 — added only after benchmark proves value.
- **No video/GIF in v1.** Image-only for now.
- **No URL input in v1.** Local files and stdin only.

---

## FAQ

**Q: How is this different from `ascii-image-converter`?**

A: `ascii-image-converter` renders images as ASCII art for humans. `ae` produces structured evidence (regions, geometry, relations, provenance) for agents. ASCII rendering is one output mode, not the product.

**Q: Does `ae` use an LLM?**

A: No. `ae` is 100% deterministic image processing. No API keys, no cloud, no vision model. The consuming agent handles interpretation.

**Q: Can I use `ae` as a regular ASCII converter?**

A: Yes. `ae render image.png --renderer ascii --width 100` works like any ASCII tool. But the real value is in `ae inspect` and `ae geometry`.

**Q: Why no OCR?**

A: OCR requires a heavy dependency (Tesseract) and introduces non-determinism. `ae` focuses on geometric evidence. If you need OCR, use an external tool or adapter.

**Q: Is `ae` deterministic?**

A: Yes. Given identical input bytes, configuration, and library version, `ae` produces identical output. Region IDs, relations, and rendering are all order-stable.

**Q: What images are supported?**

A: PNG, JPEG, WebP via the `image` crate. stdin piping supported. GIF, video, and URL input are explicitly out of scope for v1.

---

## Security

`ae` enforces resource limits at decode time:

- Max file size: 100 MB
- Max decoded pixels: 25,000,000
- Max render width/height: 10,000
- Max regions: 500
- Max charset length: 128

Decompression bombs are rejected before memory allocation. Malformed images produce graceful errors, never panics.

---

## License

MIT
