# COMPREHENSIVE PLAN FOR AGENT-EYE

> **Date:** 2026-08-21
> **Repository:** `quangdang46/agent_eye`
> **Binary:** `ae`
> **Language:** Rust

> **Tagline:** Agent-Eye converts pixels into deterministic visual evidence.

> **Design principle:** `ae` should maximize task-relevant visual evidence per unit of agent context, while remaining deterministic, inspectable, offline, and model-agnostic.

---

## Table of Contents

1. [Product Definition](#1-product-definition)
2. [Research Corpus](#2-research-corpus)
3. [Feature Matrix — 6 Repos vs agent-eye](#3-feature-matrix--6-repos-vs-agent-eye)
4. [Core Philosophy & Boundaries](#4-core-philosophy--boundaries)
5. [Crate Architecture](#5-crate-architecture)
6. [CLI Specification](#6-cli-specification)
7. [Rendering Engine](#7-rendering-engine)
8. [Perception Engine](#8-perception-engine)
9. [Grounding & Spatial Relations](#9-grounding--spatial-relations)
10. [Coordinate Mapping](#10-coordinate-mapping)
11. [Agent Interface — JSON/MCP](#11-agent-interface--jsonmcp)
12. [Development Phases](#12-development-phases)
13. [Benchmark Specification](#13-benchmark-specification)
14. [Security & Robustness](#14-security--robustness)
15. [Definition of Done](#15-definition-of-done)

---

## 1. Product Definition

### What is `agent-eye`?

> **`ae` is a Rust-native CLI that converts pixels into deterministic visual evidence — without an LLM, API keys, or cloud inference.**

```text
                   ┌──────────────┐
                   │     ae       │
                   └──────┬───────┘
                          │
             deterministic evidence
                          │
          ┌───────────────┼────────────────┐
          │               │                │
       pixels          geometry       representation
          │               │                │
          └───────────────┼────────────────┘
                          │
                       provenance
                          │
                          ▼
                     TEXT AGENT
                          │
                       meaning
```

### What `ae` is NOT

```text
❌ LLM / VLM / API keys in core
❌ image editor
❌ general computer vision framework
❌ object detection zoo
❌ image generation
❌ cloud service
❌ visual filesystem (ls/cat/stat)
❌ semantic image captioning
❌ autonomous reasoning
❌ OCR core dependency (optional external adapter only)
❌ diff tool (use a separate tool)
❌ HTTP service
❌ video/GIF processing in v1
❌ URL input in v1
❌ Python/Node/WASM bindings in v1
```

### The boundary

```text
ae  = eyes + preprocessing (deterministic)
agent = brain (reasoning/interpretation)
```

Agent decides what to see. `ae` provides the ability to look.

### Why it exists

Standard image-to-ASCII tools optimize: *"How can I make this image look good in a terminal?"*

`ae` optimizes: *"How can I encode enough visual information into text so a model without vision can reason about it?"*

---

## 2. Research Corpus

### 6 reference repositories cloned to `.tmp/`

| # | Repo | Language | Why cloned |
|---|------|----------|------------|
| 1 | [vietnh1009/ASCII-generator](https://github.com/vietnh1009/ASCII-generator) | Python | Rendering algorithms, multilingual charsets |
| 2 | [TheZoraiz/ascii-image-converter](https://github.com/TheZoraiz/ascii-image-converter) | Go | CLI UX, Braille, dither, stdin, save modes |
| 3 | [hpjansson/chafa](https://github.com/hpjansson/chafa) | C | Advanced rendering: ANSI/Unicode, terminal graphics |
| 4 | [cslarsen/jp2a](https://github.com/cslarsen/jp2a) | C | Unix CLI philosophy, stdin/stdout, terminal behavior |
| 5 | [orhnk/RASCII](https://github.com/orhnk/RASCII) | Rust | Rust-native: library+CLI, charsets, builder pattern |
| 6 | [SameerVers3/pixel2ascii](https://github.com/SameerVers3/pixel2ascii) | Rust | Block sampling, rayon, aspect correction baseline |

### Research papers referenced

- [Rethinking Token Reduction for Large Vision-Language Models](https://arxiv.org/abs/2603.21701) — multi-turn VQA, adaptive compression
- [Can Visual Input Be Compressed?](https://arxiv.org/abs/2511.02650) — UniPruneBench, OCR sensitivity
- [Visual token compression via run-length pruning](https://www.sciencedirect.com/science/article/pii/S0167865526002308) — 80% reduction, 99.3% accuracy
- [LLaVA-PruMerge (ICCV 2025)](https://openaccess.thecvf.com/content/ICCV2025/html/Shang_LLaVA-PruMerge_Adaptive_Token_Reduction_for_Efficient_Large_Multimodal_Models_ICCV_2025_paper.html) — adaptive token reduction
- [ASCIIEval 2026](https://proceedings.iclr.cc/paper_files/paper/2026/hash/63f5c95b1e6364c42075f913d84ccb73-Abstract-Conference.html) — LLM visual perception in ASCII text
- [Visual Perception Token](https://arxiv.org/abs/2502.17425) — selective region re-reading
- [VGR: Visual Grounded Reasoning](https://proceedings.iclr.cc/paper_files/paper/2026/hash/90aeee6dfe75ab6b5d6958bce40d3e16-Abstract-Conference.html) — dynamic visual memory replay
- [CropVLM](https://arxiv.org/abs/2511.19820) — fine-grained perception via cropping
- [SpatialVLM](https://arxiv.org/pdf/2401.12168) — "friend who can see" decomposition

### Key research insight

> **Visual token quantity does not matter as much as information quality/relevance. Fixed compression does not fit all image/task types.**

---

## 3. Feature Matrix — 6 Repos vs agent-eye

### Compatibility features (rendering capabilities useful for agents)

| Feature | ASCII-gen | ascii-conv | chafa | jp2a | RASCII | p2a | agent-eye |
|---------|:---------:|:----------:|:-----:|:----:|:------:|:---:|:---------:|
| Image decode | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| Grayscale | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| Color (plain text) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| Custom charset | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| Aspect correction | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| Unicode blocks | — | — | ✅ | — | ✅ | ✅ | P0 |
| Invert | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | P0 |
| stdin | — | ✅ | ✅ | ✅ | — | — | P0 |
| File output (stdout) | ✅ | ✅ | — | ✅ | — | ✅ | P0 |
| Full terminal fit | — | ✅ | ✅ | ✅ | — | — | P0 |
| Multiple languages | ✅ | — | — | — | ✅ | — | P0 |
| Flip X/Y | — | ✅ | ✅ | ✅ | — | — | P1 |
| Braille | — | ✅ | ✅ | — | — | — | P1 |
| ANSI truecolor | — | ✅ | ✅ | — | ✅ | ✅ | P1 (compat) |
| Dithering | — | ✅ | ✅ | — | — | — | P1 |
| Background color | ✅ | ✅ | ✅ | — | ✅ | — | P1 |
| Batch processing | — | ✅ | ✅ | — | — | — | P1 |
| Save PNG | ✅ | ✅ | — | — | — | — | P2 |
| Font rendering | ✅ | ✅ | — | — | — | — | P2 |
| Save GIF | — | ✅ | ✅ | — | — | — | P2 |
| Library API | — | ✅ | ✅ | — | ✅ | ✅ | P0 |
| Video | ✅ | — | ✅ | — | — | ✅ | ❌ out of scope |
| URL input | — | ✅ | ✅ | ✅ | — | — | ❌ out of scope |

### Agent-native features (the differentiator)

| Feature | Priority | Description |
|---------|----------|-------------|
| `inspect` | P0 | Orchestrated overview: dimensions, regions, geometry, rendering |
| Candidate region segmentation | P0 | Deterministic geometric candidates from image analysis |
| `zoom` | P0 | Targeted crop + resampling at specified scale |
| `region` | P0 | Extract specific area with full provenance |
| Geometry | P0 | Spatial evidence: regions, bounds, formal relations |
| Spatial relations | P0 | left_of, right_of, above, below, inside, contains, overlaps |
| Token-efficient output | P0 | JSON machine-readable with versioned schemas |
| Coordinate mapping | P0 | Affine transform: output → original pixels |
| Progressive inspection | P0 | Agent-driven: overview → uncertainty → region → zoom → answer |
| Deterministic ordering | P0 | Region IDs + relations sorted by y → x → area → stable tie-break |
| Adaptive budget | P1 | `--budget N` → optimize representation (after benchmark proves value) |
| MCP server | P1 | Transport/interface, no LLM |
| Advanced region analysis | P1 | Hierarchy, importance (after proven) |

---

## 4. Core Philosophy & Boundaries

### Design principles

1. **`ae` MUST NOT contain an AI brain.**
2. **`ae` MUST expose as much useful visual evidence as possible in machine-readable and token-efficient forms.**
3. **The consuming agent performs interpretation, reasoning, and decides when more visual evidence is needed.**
4. **Every feature must either improve visual information preservation or directly improve agent perception/reasoning.**
5. **No semantic fields in core schema:** `label`, `type`, `confidence`, `semantic_class`, `importance`, `object`, `description` are all prohibited.

### What agents will use `ae` for

```text
inspect    → orchestrated overview (dimensions, regions, geometry, rendering)
render     → visual → text representation (ASCII/Braille/blocks)
region     → extract only the part they need
zoom       → get more detail at a specific scale
geometry   → spatial evidence (regions, formal relations, coordinates)
```

### What agents will NOT use `ae` for

```text
reasoning about what the image "means"
classifying objects
captioning
detecting faces
understanding semantics
reading text (OCR = external adapter, not core)
comparing two images (diff = separate tool)
```

### Agent default output: plain text, not ANSI

Agent output is always:

```text
plain text
JSON
```

Never ANSI escape sequences by default. ANSI `--color` is a **compatibility feature** for human terminal display, not for agent consumption.

JSONL is P1 (only needed with batch processing).

### Killer feature: progressive visual inspection

NOT `--budget`. The real differentiator is:

```text
image
 ↓
overview (ae inspect)
 ↓
agent sees uncertainty
 ↓
region (ae region)
 ↓
zoom (ae zoom)
 ↓
answer
```

Agent drives the perception loop. `ae` provides primitives.

---

## 5. Crate Architecture

### Workspace layout

```text
agent-eye/
├── Cargo.toml          (workspace)
├── Cargo.lock
├── rust-toolchain.toml
├── crates/
│   ├── ae-core/        (image, geometry, analysis, regions, transforms, sampling, provenance, mapping)
│   ├── ae-render/      (ASCII, blocks, charset, mapping; braille/dither in P1)
│   └── ae-cli/         (clap CLI, all commands — orchestration only)
├── tests/
│   ├── golden/
│   ├── integration/
│   └── fixtures/
├── benches/
├── bench/              (agent evaluation harness)
├── docs/
│   └── PROVENANCE.md   (algorithm provenance tracking)
└── FEATURE-MATRIX.md   (detailed per-function matrix)
```

**No `ae-ocr` crate.** OCR is an optional external adapter, not a core dependency.

### Dependency graph

```text
ae-cli (orchestration only — no business logic)
  │
  ├── ae-core
  │     ├── image (decode, pixel buffer)
  │     ├── geometry
  │     ├── sampling
  │     ├── transforms (resize, crop)
  │     ├── analysis (luminance, edges, contrast)
  │     ├── regions (candidate segmentation)
  │     ├── provenance (source tracking)
  │     └── mapping (affine coordinate transform)
  │
  └── ae-render
        ├── ASCII renderer
        ├── blocks renderer
        ├── charset system
        └── dithering [P1]
```

**Rules:**
- Dependency flows one way: `cli → core, render`
- Core does not know about CLI, render, or MCP
- No LLM/API dependency in any crate
- No Python subprocess, no ImageMagick
- CLI is **orchestration only** — business logic lives in core/render
- Provenance is a **first-class module** in ae-core, not sprinkled throughout

### Canonical internal image representation

```rust
pub struct Image {
    pub dimensions: Dimensions,
    pub pixels: PixelBuffer,
    pub metadata: ImageMetadata,
}

pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
```

**Do not expose `image` crate types throughout `ae-core`.** This gives freedom to change decoding/storage without infecting the API.

### Key Rust dependencies

| Crate | Purpose |
|-------|---------|
| `clap` 4.x (derive) | CLI parsing |
| `image` 0.25.x | Image decode (PNG, JPEG, WebP) — internal only |
| `rayon` 1.x | Parallel processing (only after profiling proves need) |
| `serde` + `serde_json` | JSON output |
| `thiserror` | Error types |
| `unicode-segmentation` | Grapheme cluster handling for charsets |
| `owo-colors` or `anstyle` | ANSI color (replacing unmaintained `ansi_term`) |
| `proptest` | Property-based testing |

### Error model

```rust
#[derive(thiserror::Error, Debug)]
enum AeError {
    #[error("decode failed: {0}")]
    Decode(String),
    #[error("invalid dimensions: {0}")]
    InvalidDimensions(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
    #[error("rendering failed: {0}")]
    Rendering(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
}
```

### Determinism contract

> Given identical input bytes, library version, configuration, and platform-independent algorithm parameters, `ae` produces identical machine-readable output.

Specifically:
- Region IDs sorted deterministically: `y → x → area → stable tie-break`
- Relations sorted the same way
- Parallelism (rayon) must not alter output ordering
- `rayon` only parallelizes where profiling proves value; start scalar, optimize later

---

## 6. CLI Specification

### Command tree (v1)

```text
ae
├── inspect <image>              # orchestrated overview
├── render <image>               # ASCII/blocks
├── region <image> <id|box>      # extract specific area
├── zoom <image> <region>        # targeted crop + resample
├── geometry <image>             # spatial evidence
└── capabilities                 # machine-readable feature list
```

Later (P1+):
```text
├── benchmark                    # run evaluation suite
└── mcp                          # MCP server
```

**No `ae ocr`** (external adapter). **No `ae diff`** (separate tool). **No `ae serve`** (no HTTP).

### Global options

```bash
--format text|json              # serialization (default: text)
--renderer ascii|blocks         # rendering engine (default: ascii)
--output <file>                 # write to file (default: stdout)
--budget <N>                    # token/byte budget (P1, after benchmark)
--region <id>                   # target specific region
--box <x,y,w,h>                # target specific bounding box
--level <0-3>                   # zoom spatial allocation scale
--width <N>                     # output width in chars
--invert                        # invert luminance
--color                         # ANSI truecolor (compatibility, not agent default)
--charset <string>              # custom character ramp
--quiet                         # suppress non-error output
```

**Two separate axes:**

```text
--renderer    = WHAT to render with (ascii/braille/blocks)
--format      = HOW to present output (text/json)
```

Never collide. `ae render img.png --renderer blocks --format json` is valid.

### `inspect` — orchestrated convenience operation (not a primitive)

`inspect` is composition, not a monolithic function:

```rust
// Internal orchestration
let image = Image::load(...)?;
let regions = detector.detect(&image)?;
let relations = geometry.relations(&regions)?;
let representation = renderer.render(&image, &regions)?;
let output = SceneInspector {
    image_analyzer: ImageAnalyzer,
    region_detector: RegionDetector,
    relation_engine: RelationEngine,
    renderer: Renderer,
};
```

The CLI doesn't become the architecture.

### Example usage — progressive inspection

```bash
# Step 1: overview
ae inspect screenshot.png

# Step 2: agent decides to look at r3
ae zoom screenshot.png --region r3

# Step 3: agent needs more detail
ae zoom screenshot.png --region r3 --level 2

# Step 4: agent needs spatial evidence
ae geometry screenshot.png --format json

# Rendering (compatibility)
ae render image.png --renderer ascii --width 100
ae render image.png --renderer blocks --invert

# Machine-readable
ae inspect image.png --format json
ae geometry image.png --format json
```

---

## 7. Rendering Engine

### ASCII renderer

Core algorithm (from pixel2ascii + ASCII-generator research):

```rust
// Block sampling
block_w = img_width / ascii_width
block_h = block_w / aspect_ratio    // aspect correction

// Per block
r_avg = avg(R pixels in block)
g_avg = avg(G pixels in block)
b_avg = avg(B pixels in block)

// Luminance (Rec. 709) — always computed internally for analysis
lum = 0.2126 * r_avg + 0.7152 * g_avg + 0.0722 * b_avg

// Character mapping
idx = round(lum / 255.0 * (charset_len - 1))
char = charset[idx]
```

### Render config

```rust
pub struct RenderConfig {
    pub renderer: RendererType,  // ASCII, Blocks
    pub width: u32,              // output width in chars
    pub aspect_ratio: f32,       // default 0.5 (2:1 terminal chars)
    pub invert: bool,
    pub charset: Option<String>, // custom charset override
    pub color: ColorMode,        // None (agent default), Grayscale, TrueColor
}
```

### Luminance is analysis, not presentation

- `--grayscale` is only a **presentation/output mode** if explicitly requested
- Analysis luminance (`0.2126R + 0.7152G + 0.0722B`) is always computed internally
- No duplicate pipelines

### Charset system

```rust
pub struct Charset {
    pub name: &'static str,
    pub glyphs: Vec<char>,
}

pub const PRESETS: &[Charset] = &[
    Charset { name: "standard", glyphs: vec!['@','%','#','*','+','-','.',':',' '] },
    Charset { name: "dense",    glyphs: vec!['@','M','#','W','$','9','8',...' '] },
    Charset { name: "blocks",   glyphs: vec!['█','▓','▒','░',' '] },
];

// Custom charset from CLI
pub fn from_custom(s: &str) -> Charset {
    Charset { name: "custom", glyphs: s.chars().collect() }
}
```

**Lessons from repos:**
- RASCII: use `&[&str]` not `&[char]` for multi-codepoint emoji — `unicode-segmentation`
- pixel2ascii: font8x8 intensity computation is dead code — skip it
- ASCII-generator: font-based brightness sorting for charsets — implement properly

### Aspect ratio correction

```rust
// Terminal chars are ~2x tall as wide
// block_h = block_w / aspect_ratio
// Default aspect = 0.5 (block_h = 2 * block_w)
// --no-aspect → aspect = 1.0 (square blocks)
```

**Known bug in ASCII-generator:** hardcoded `cell_height = 2 * cell_width` causes distortion for CJK/emoji double-width chars. `ae` must handle this correctly per charset width class.

### Color pipeline

```rust
pub enum ColorMode {
    None,        // plain text (agent default)
    Grayscale,
    TrueColor,   // \x1b[38;2;R;G;Bm (compatibility)
}
```

**Agent default is always `None` (plain text).** ANSI color is a human-display convenience.

---

## 8. Perception Engine

### Candidate region segmentation

**⚠️ Critical:** Region detection produces **deterministic geometric candidates**, not semantic interpretations. The same algorithm can produce different valid segmentations depending on the image. **Do not promise that regions correspond to human-perceived layout regions.**

Rename internally:

```rust
pub struct CandidateRegion { ... }   // not Region
```

Or:

```rust
pub struct VisualRegion { ... }
```

Document:

> A region is a deterministic geometric candidate produced by image analysis. It is not guaranteed to correspond to a semantic object or human-perceived layout region. The agent interprets meaning.

### Region detection pipeline

```text
V1 — not P0. Define contract FIRST, then implement.

Phase 5A: define contract + fixtures + stability measurement
Phase 5B: implement first heuristic
Phase 5C: benchmark it
Phase 5D: expose through inspect/geometry
```

V1 heuristic:

```text
1. Edge map
2. Whitespace segmentation
3. Connected component analysis
4. Merge overlapping rectangles
5. Assign IDs + compute metrics
6. Deterministic sort: y → x → area → stable tie-break
```

```rust
pub struct CandidateRegion {
    pub id: String,           // "r1", "r2", "r3.1"
    pub bounds: HalfOpenBounds, // [x1, y1, x2, y2) — half-open
    pub area: f32,
    pub edge_density: f32,
    pub color_variance: f32,
}
```

**What NOT to do in v1:**
- ❌ Semantic labels ("button", "card", "dashboard")
- ❌ Layout detection ("header/sidebar/main/footer")
- ❌ Object classification
- ❌ Scene understanding
- ❌ Guarantee that regions match human intuition

### Half-open coordinate convention

Internally:

```rust
pub struct HalfOpenBounds {
    pub x1: u32,  // inclusive
    pub y1: u32,  // inclusive
    pub x2: u32,  // exclusive
    pub y2: u32,  // exclusive
}
```

Expose as `[x, y, width, height]` at JSON boundary if friendlier.

### Resource limits for regions

```text
max_region_count: 500
max_relation_count: bounded by O(n²) × max_region_count
```

Spatial relations are O(n²) for n regions. Define limits to prevent CPU exhaustion.

### VisualComplexity (renamed from InformationScore)

```rust
pub struct VisualComplexity {
    pub edge: f32,
    pub gradient: f32,
    pub variance: f32,
    pub redundancy: f32,
    pub score: f32,
}
```

**"Information" implies task relevance.** "Complexity" is purely geometric/statistical.

**Not exposed in CLI v1.** Internal implementation detail for future `--budget`.

---

## 9. Grounding & Spatial Relations

### Spatial relations (v1 — 7 core relations, formal definitions)

```rust
pub enum Relation {
    LeftOf,
    RightOf,
    Above,
    Below,
    Inside,
    Contains,
    Overlaps,
}
```

**Not in v1:** `Near`, `AlignedWith`, `AdjacentTo` — too much ambiguity.

### Formal relation semantics (half-open bounds)

```text
left_of(A, B):    A.x2 <= B.x1
right_of(A, B):   A.x1 >= B.x2
above(A, B):      A.y2 <= B.y1
below(A, B):      A.y1 >= B.y2
inside(A, B):     A.x1 >= B.x1 AND A.x2 <= B.x2
                  A.y1 >= B.y1 AND A.y2 <= B.y2
contains(A, B):   inside(B, A)
overlaps(A, B):   A.x1 < B.x2 AND A.x2 > B.x1
                  A.y1 < B.y2 AND A.y2 > B.y1
```

**Touching boxes:** `A.x2 == B.x1` means A is `left_of` B (touching). This is the mathematical answer — document it explicitly in the contract.

### Deterministic relation ordering

Relations sorted by: `relation_type → a.id → b.id`. Same output every run.

---

## 10. Coordinate Mapping

### Affine transform (formal)

```rust
pub struct CoordinateTransform {
    pub source: HalfOpenBounds,
    pub output_width: u32,
    pub output_height: u32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}
```

Mapping:

```text
source_x = output_x * scale_x + offset_x
source_y = output_y * scale_y + offset_y
```

**Why affine?** After crop, resize, zoom, or region transform, `scale_x != scale_y` and `offset_x != 0`. Simple ratios are insufficient.

### Usage across operations

```text
crop       → offset changes, scale unchanged
resize     → scale changes
zoom       → scale + offset change (crop + resample)
region     → offset changes
render     → scale changes
```

Every output includes the transform so agent can map back:

```json
{
  "mapping": {
    "source_bounds": [300, 80, 1440, 900],
    "output_width": 80,
    "output_height": 45,
    "scale_x": 14.25,
    "scale_y": 18.44,
    "offset_x": 300.0,
    "offset_y": 80.0
  }
}
```

Agent can reason on ASCII but map back to original coordinates.

### Provenance (first-class module)

```rust
pub struct Provenance {
    pub source_hash: String,       // sha256 of original image
    pub source_bounds: HalfOpenBounds,
    pub transform: CoordinateTransform,
}
```

Every output includes provenance. Agent never loses connection to original image.

---

## 11. Agent Interface — JSON/MCP

### JSON output — versioned schemas

Every JSON output includes `schema_version`.

```text
agent-eye.scene.v1       — inspect output
agent-eye.region.v1      — region output
agent-eye.geometry.v1    — geometry output
agent-eye.render.v1      — render output
agent-eye.capabilities.v1 — capabilities output
```

#### `inspect` output (`agent-eye.scene.v1`)

```json
{
  "schema_version": "agent-eye.scene.v1",
  "image": {
    "width": 1440,
    "height": 900,
    "format": "png"
  },
  "regions": [
    {
      "id": "r1",
      "bounds": [0, 0, 1440, 96],
      "area": 0.07,
      "edge_density": 0.42,
      "color_variance": 0.18
    }
  ],
  "relations": [
    {"type": "left_of", "a": "r2", "b": "r3"},
    {"type": "above", "a": "r1", "b": "r3"}
  ],
  "representation": {
    "renderer": "ascii",
    "data": "..."
  },
  "mapping": {
    "source_bounds": [0, 0, 1440, 900],
    "output_width": 80,
    "output_height": 54,
    "scale_x": 18.0,
    "scale_y": 16.67,
    "offset_x": 0.0,
    "offset_y": 0.0
  }
}
```

**No semantic labels** in JSON — regions have geometry and metrics only.

**No prohibited fields:** `label`, `type` (semantic), `confidence`, `importance`, `object`, `description`.

#### `capabilities` output (`agent-eye.capabilities.v1`)

```bash
ae capabilities --format json
```

```json
{
  "schema_version": "agent-eye.capabilities.v1",
  "version": "1.0.0",
  "input": ["png", "jpeg", "webp", "stdin"],
  "renderers": ["ascii", "blocks"],
  "commands": ["inspect", "render", "region", "zoom", "geometry"],
  "output_formats": ["text", "json"],
  "limits": {
    "max_pixels": 25000000,
    "max_file_size_bytes": 104857600,
    "max_region_count": 500,
    "max_render_width": 10000,
    "max_render_height": 10000,
    "max_charset_length": 128
  }
}
```

Agent discovers `ae` capabilities dynamically instead of assuming features exist.

### MCP server (P1, later)

```bash
ae mcp
```

Tools:

```text
inspect_image    → orchestrated overview
render_image     → ASCII/blocks
zoom_image       → targeted crop
inspect_geometry → spatial evidence
```

MCP exposes CLI/core primitives only. No HTTP. No LLM.

---

## 12. Development Phases

### Phase 0 — Specification ✅

```text
[x] Clone 6 repos
[x] Audit all 6 codebases
[x] Read conversation corpus (~14,500 lines)
[x] Read benchmark spec (~711 lines)
[x] Write this plan
[ ] FEATURE-MATRIX.md (detailed per-function matrix)
[ ] docs/PROVENANCE.md (algorithm source tracking)
[ ] LICENSE-MATRIX.md (GPL-2.0 warning for jp2a)
```

### Phase 1 — Workspace + Secure Image Abstraction

```text
[ ] Workspace setup
[ ] ae-core: canonical Image + PixelBuffer (not image crate types)
[ ] ae-core: Dimensions, Bounds, HalfOpenBounds
[ ] ae-core: error types (AeError)
[ ] ae-core: decode PNG/JPEG/WebP
[ ] ae-core: security limits (max_pixels, max_file_size)
[ ] ae-core: stdin input
[ ] ae-core: CoordinateTransform
[ ] ae-core: Provenance (first-class module)
[ ] CI: fmt + clippy + test
```

**Definition of done:** Decode any PNG/JPEG/WebP, return Image with dimensions + pixel data + provenance.

### Phase 2 — ASCII + Blocks Renderer

```text
[ ] Luminance calculation (Rec. 709 — always internal)
[ ] Block sampling with aspect correction
[ ] Charset system (preset + custom)
[ ] ASCII renderer
[ ] Blocks renderer
[ ] --renderer, --width, --height, --aspect, --invert, --charset
[ ] Golden tests (scalar implementation first, parallelize only after profiling)
```

**Definition of done:** `ae render image.png --renderer ascii --width 100` matches/exceeds pixel2ascii quality.

### Phase 3 — Renderer Parity + CLI

```text
[ ] Grayscale presentation mode (--grayscale)
[ ] Color mode (plain text)
[ ] Full terminal fit
[ ] stdin piping
[ ] Multiple charset presets
[ ] Multilingual charsets (CJK, Cyrillic)
[ ] Custom charset
[ ] File output
[ ] ae inspect command (orchestrated)
[ ] ae render command
[ ] ae capabilities command
[ ] JSON output for all commands
```

**NOT in this phase:** video, GIF, save PNG, font rendering, URL, JSONL, batch, flip.

**Definition of done:** All v1 rendering capabilities useful for agents have Rust equivalents with tests.

### Phase 4 — Core Analysis Primitives

```text
[ ] Edge detection (Sobel or similar)
[ ] Contrast analysis
[ ] Color variance computation
[ ] VisualComplexity computation (internal)
```

### Phase 5 — Candidate Region Segmentation

```text
Phase 5A: define region contract + fixtures + stability measurement
Phase 5B: implement first heuristic (edges, whitespace, connected components)
Phase 5C: benchmark region quality
Phase 5D: expose through inspect/geometry
[ ] CandidateRegion struct
[ ] Deterministic sort (y → x → area → tie-break)
[ ] Resource limits (max_region_count)
```

**Critical gate:** If regions don't help agent tasks in Phase 7, simplify or remove.

### Phase 6 — Geometry + Provenance + Zoom

```text
[ ] Spatial relations (7 core, formal half-open math)
[ ] Relation ordering (deterministic)
[ ] Coordinate mapping (affine transform)
[ ] Provenance tracking (every output)
[ ] ae zoom command (crop + resample at scale)
[ ] ae region command (with provenance)
[ ] ae geometry command
```

### Phase 7 — Agent Benchmark (PROJECT GATE)

```text
[ ] Agent evaluation harness
[ ] cases.jsonl with test cases
[ ] 4-tier benchmark implementation
[ ] Fixed baselines: ASCII, Blocks
[ ] Progressive inspection baseline
[ ] Measure: accuracy / tokens / tool calls / latency / wall-clock
[ ] Negative capability cases (uncertainty, insufficient evidence)
[ ] Prove or disprove: progressive inspection > fixed rendering
```

```text
                ┌───────────────┐
                │ Phase 7       │
                │ benchmark     │
                └───────┬───────┘
                        │
              ┌─────────┴─────────┐
              │                   │
           improves            doesn't
              │                   │
              ▼                   ▼
         continue             simplify
```

**If benchmark shows `ae + agent ≈ ASCII + agent`, stop. Re-evaluate.**

### Phase 8 — Only Proven Features Continue

```text
[ ] Braille renderer [P1, only if benchmark shows value]
[ ] Dithering [P1]
[ ] ANSI truecolor (--color compatibility) [P1]
[ ] Adaptive budget [P1, only if benchmark proves value]
[ ] Batch processing [P1]
[ ] Background color [P1]
```

### Phase 9 — MCP Integration (P1)

```text
[ ] ae-mcp crate
[ ] MCP server wrapping core primitives
[ ] Tool definitions
```

### Phase 10 — Hardening & Release

```text
[ ] Fuzzing (image decoder, CLI args, JSON schema)
[ ] Property-based testing (determinism, bounds, no-panic)
[ ] Cross-platform CI (Linux, macOS, Windows)
[ ] Performance benchmarks
[ ] Documentation
[ ] Release pipeline (crates.io, GitHub releases, binaries)
```

### Explicitly out of scope

```text
❌ OCR core, diff, HTTP, video, URL,
   Python/Node/WASM, LLM, semantic detection,
   object detection, captioning, cloud
```

---

## 13. Benchmark Specification

### Overview

> **Don't benchmark to prove `ae` is good.** Design benchmarks that can also prove `ae` is NOT useful.

### 4-tier benchmark system

#### Tier 1 — Correctness

```text
Dataset: bench/fixtures/
├── portrait/
├── landscape/
├── diagram/
├── chart/
├── screenshot/
├── dense/
└── text-heavy/

Tests:
[x] Image decoding (PNG, JPEG, WebP)
[x] Dimensions preserved
[x] Crop correctness
[x] Region validity
[x] Zoom at specified scale
[x] Renderer determinism (same input → same output, every run)
[x] Deterministic region ordering (rayon doesn't alter order)
```

#### Tier 2 — Representation Quality

```text
Measure:
- Spatial preservation (position error, edge displacement)
- Edge preservation (horizontal, vertical, corners)
- Detail preservation (low/medium/high complexity)

Compare:
ASCII vs Blocks

Metrics:
- information coverage
- structure preservation
- spatial distortion
```

#### Tier 3 — Context Efficiency

```text
For the same image:
- Fixed ASCII (width=100)
- Fixed Blocks (width=100)
- ae inspect (no budget)
- ae inspect --budget N (P1)

Measure:
- output bytes
- estimated tokens
- visual information retained (spatial accuracy)

Target:
╔═══════════════════════════════════════════════╗
║  maximize useful visual information / token   ║
╚═══════════════════════════════════════════════╝
```

#### Tier 4 — Agent Task Benchmark (MOST IMPORTANT)

```text
Dataset: 100 screenshots + 100 diagrams + 100 charts + 100 UI + 100 natural

Task classes:

Spatial:    "Where is A relative to B?"
Geometry:   "Which region occupies the largest area?"
Counting:   "How many repeated visual elements?"
Detail:     "What shape/pattern appears in region X?"
Structure:  "How many visually distinct regions?"
Progressive: "Can the agent answer after overview?
             If not, does targeted zoom resolve uncertainty?"

Negative:   "What exact text appears in tiny button?"
            → Correct: "I cannot determine from available representation."
            → Tests calibrated uncertainty, not hallucinated answers.
```

Compare:

```text
╔═══════════════════════════════════════════════════════════╗
║  LLM alone:              baseline                        ║
║  LLM + fixed ASCII:      X% accuracy / N tokens / T calls║
║  LLM + fixed Blocks:     X% accuracy / N tokens / T calls║
║  LLM + ae inspect:       Y% accuracy / N tokens / T calls║
║  LLM + ae progressive:   Z% accuracy / N tokens / T calls║
║  Vision model:           upper bound                     ║
╚═══════════════════════════════════════════════════════════╝
```

**Key:** Measure **task accuracy per unit of context + interaction cost.**

Progressive inspection can "win" by spending unlimited context. Must measure:

```text
tool calls
bytes transferred
tokens transferred
wall-clock latency
reasoning turns
```

### Agent Evaluation Harness

```text
bench/
├── fixtures/
│   ├── ui/
│   ├── diagrams/
│   ├── charts/
│   ├── screenshots/
│   └── natural/
├── cases.jsonl
├── agent-eval.sh
├── run-case.sh
├── evaluate.*
└── results/
    ├── baseline.jsonl
    ├── ascii.jsonl
    ├── blocks.jsonl
    ├── ae-inspect.jsonl
    ├── ae-progressive.jsonl
    ├── ae-negative.jsonl
    └── summary.json
```

### `cases.jsonl` format

```json
{"id":"001","image":"001-ui.png","task":"What is located in the top-right corner?","type":"spatial"}
{"id":"002","image":"002-dashboard.png","task":"How many visually distinct regions?","type":"structure"}
{"id":"003","image":"003-button.png","task":"What exact text appears in the tiny button?","type":"negative","expected_insufficient":true}
```

### Capture ae usage

```json
{
  "case": "001",
  "mode": "ae-progressive",
  "answer": "...",
  "ae_calls": ["inspect", "zoom", "geometry"],
  "ae_tokens_used": 1820,
  "tool_calls": 3,
  "bytes_transferred": 4200,
  "duration_ms": 1820
}
```

### Visual Information Density metric

```text
VID = useful visual information / (context cost + interaction cost)

ASCII        0.42
Blocks       0.65
ae inspect   0.78
ae progress  0.89  (if progressive proves valuable)
```

### Performance benchmarks

```text
Decode time      (1MP, 5MP, 10MP, 20MP)
Render time      (per renderer, scalar first)
Region detection (per image complexity)
Zoom             (per crop size)
Memory peak      (RSS)
Startup time
Binary size
Dependency count
```

### CI regression guards

```text
binary size +20% → warning
startup time +30% → warning
```

### Golden test fixtures

```text
tests/fixtures/
├── simple-box.png          # basic geometry
├── checkerboard.png        # pattern detection
├── gradient.png            # luminance mapping
├── ui.png                  # layout detection
├── diagram.png             # edge detection
├── dense.png               # complexity
└── text-heavy.png          # high-detail regions
```

---

## 14. Security & Robustness

### Input limits

```text
max_file_size: 100MB
max_decoded_pixels: 25,000,000
max_width: 100,000
max_height: 100,000
max_processing_time: 30s
```

### Output limits

```text
max_output_bytes: 50MB
max_render_width: 10,000
max_render_height: 10,000
max_region_count: 500
max_relation_count: bounded by O(n²) × max_region_count
max_charset_length: 128
```

**Why output limits?** An attacker doesn't need a huge image. Tiny input + pathological region generation = huge output / CPU.

### Malformed input handling

```text
Corrupted PNG → graceful error, no panic
Truncated JPEG → graceful error
Invalid dimensions → error message
Empty image → error message
Random bytes → no crash (fuzz testing)
```

### Decompression bomb protection

```text
Check decoded dimensions BEFORE allocating memory
Reject images where width × height > max_pixels
```

---

## 15. Definition of Done

### Non-negotiable requirements

1. **Rust-native** — no Python, no subprocess
2. **Deterministic** — same input + same config = identical output, every run
3. **Secure** — survives malformed/crafted images
4. **Fast** — benchmarks prove Rust advantage
5. **Agent-proven** — benchmark shows improved agent task accuracy per context unit
6. **Focused** — no feature exists just because "it's cool"

### The ultimate success metric

> **Agent + ae achieves better visual task accuracy per unit of context + interaction cost than agent + fixed ASCII/Blocks.**

If benchmark shows:

```text
ae + agent ≈ ASCII + agent
```

→ cut agent features, keep rendering only.

If benchmark shows:

```text
ae progressive + agent >> fixed ASCII + agent
```

→ `ae` has proven its value.

### Definition of Done for v1

```text
                    agent-eye 1.0
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
      Rendering     Performance    Agent-native
          │              │              │
        ASCII           Rust-native    inspect
        Blocks          benchmark      region
        charset         secure         zoom
        aspect                         geometry
        stdin                          JSON
        provenance                     progressive
                                       coordinate mapping
```

**Braille is P1, not in v1.** v1 rendering = ASCII + Blocks.

### Core thesis, final word

```text
             ┌─────────────┐
IMAGE ──────►│      AE     │
             │             │
             │  inspect    │
             │  region     │
             │  zoom       │
             │  render     │
             │  geometry   │
             └──────┬──────┘
                    │
            compact evidence
                    │
                    ▼
              TEXT-ONLY AGENT
```

Agent doesn't need `ae` to say:

> "This is a dashboard with a sidebar."

Agent needs `ae` to say:

> "3 regions detected. r2 is left, occupying 21% area. r3 has high edge density. Here is the representation of r3."

**Agent figures out it's a dashboard.**

That's the beautiful boundary of this project.

> **`ae` does not tell the agent what the image means. It gives the agent better evidence to decide what it means.**

And we won't implement features just because the plan says so. If the benchmark at Phase 7 shows `region + zoom` doesn't help the agent, we **don't continue** — we re-evaluate. Plan is a guide, not a contract.

---

## Appendix A: License/Provenance Matrix

| Source | License | Can copy code? | Can use algorithms? |
|--------|---------|:--------------:|:-------------------:|
| ASCII-generator | MIT | ✅ | ✅ |
| ascii-image-converter | Apache-2.0 | ✅ | ✅ |
| chafa | LGPL-3.0 | ⚠️ lib only | ✅ |
| jp2a | GPL-2.0 | ❌ (copyleft) | ✅ (ideas only) |
| RASCII | MIT | ✅ | ✅ |
| pixel2ascii | MIT | ✅ | ✅ |

**Rule:** Algorithm ideas are always portable. Code copying requires license compatibility. When in doubt, reimplement independently.

### docs/PROVENANCE.md format

```text
| Source file | Algorithm studied | License | Copied code? | Independently rewritten? |
|-------------|-------------------|---------|:------------:|:------------------------:|
| pixel2ascii/src/image.rs | Block sampling avg | MIT | no | yes |
| ASCII-generator/utils.py | Brightness sorting | MIT | no | yes |
| jp2a/src/image.c | Aspect ratio calc | GPL-2.0 | no | yes |
```

## Appendix B: P0/P1/P2 Summary

```text
P0 — v1 core (must have)
  decode PNG/JPEG/WebP, stdin, resource limits
  ASCII, blocks, charset, aspect correction
  grayscale (analysis always internal), invert, terminal fit
  inspect (orchestrated), candidate region segmentation
  zoom (scale-based crop), geometry, spatial relations (7)
  coordinate mapping (affine), provenance (first-class)
  JSON, deterministic output, progressive inspection
  library API, capabilities

P1 — after v1 proven (should have)
  Braille, dithering, flip X/Y
  adaptive budget (--budget N, only if benchmark proves value)
  ANSI truecolor (compatibility flag)
  batch processing, background color, JSONL
  MCP server
  advanced region analysis (hierarchy)

P2 — later if needed (nice to have)
  font rendering, save PNG, GIF export

Explicitly out of scope:
  ❌ OCR core, diff, HTTP, video, URL,
     Python/Node/WASM, LLM, semantic detection,
     object detection, captioning, cloud
```
