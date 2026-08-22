//! ae CLI entry: subcommand dispatch + I/O plumbing.

/// Piped-binary stdin on Windows defaults to text mode; switch to binary so
/// CRLF translation cannot corrupt PNG/JPEG/WebP bytes (no-op elsewhere).
#[cfg(windows)]
fn set_stdin_binary() {
    use std::os::windows::io::AsRawStdin;
    unsafe {
        // winapi-style: _setmode(0, _O_BINARY) == 0x8000
        #[link(name = "ucrt")]
        extern "C" {
            fn _setmode(fd: i32, mode: i32) -> i32;
        }
        let fd = std::io::stdin().as_raw_stdin() as i32;
        let _ = _setmode(fd, 0x8000);
    }
}

#[cfg(not(windows))]
fn set_stdin_binary() {}

use clap::{Parser, Subcommand, ValueEnum};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ae_core::decode::decode_bytes;
use ae_core::image::Limits;
use ae_render::config::{ColorMode, RenderConfig};
use ae_render::render::RenderedGrid;
use ae_render::{render, Charset};

/// ae — Agent-Eye: converts pixels into deterministic visual evidence.
#[derive(Parser, Debug)]
#[command(name = "ae", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Feature discovery for agents: what this build can do.
    Capabilities {
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
    },
    /// Batch mode: inspect every image in a directory, JSONL output,
    /// parallel via rayon. Deterministic ordering (sorted by path).
    Batch {
        /// Directory containing images (png/jpeg/webp).
        path: PathBuf,
        /// Output width for embedded renders.
        #[arg(long, default_value_t = 60)]
        width: u32,
        /// Skip ASCII overview in each record.
        #[arg(long, default_value_t = false)]
        no_render: bool,
    },
    /// Orchestrated overview: dimensions, regions, relations, rendering,
    /// mapping — the agent's first look at an image.
    Inspect {
        /// Image path, or `-` for stdin.
        image: PathBuf,
        /// Output width in characters for the embedded render.
        #[arg(long, default_value_t = 60)]
        width: u32,
        /// Skip the ASCII overview (geometry only).
        #[arg(long, default_value_t = false)]
        no_render: bool,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
        /// Write result to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Overwrite `--output` file if it exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Spatial evidence: detected regions and their formal relations.
    Geometry {
        /// Image path, or `-` for stdin.
        image: PathBuf,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
        /// Write result to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Overwrite `--output` file if it exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Crop + resample: more output samples devoted to a smaller source
    /// region (spatial allocation, NOT detail increase).
    Zoom {
        /// Image path, or `-` for stdin.
        image: PathBuf,
        /// Region bounds as x,y,w,h in source pixels.
        #[arg(long)]
        box_: String,
        /// Zoom level 0-3 → 1x, 2x, 4x, 8x spatial allocation scale.
        #[arg(long, value_parser = clap::value_parser!(u32).range(0..=3), default_value_t = 0)]
        level: u32,
        /// Output width in characters.
        #[arg(long, default_value_t = 80)]
        width: u32,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
        /// Write result to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Overwrite `--output` file if it exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Render an image as ASCII or Unicode blocks.
    /// Extract a specific area as a rendered region with full provenance.
    Region {
        /// Image path, or `-` for stdin.
        image: PathBuf,
        /// Region bounds as x,y,w,h in source pixels (half-open window).
        #[arg(long, group = "target")]
        box_: Option<String>,
        /// Detected region id from `ae geometry` (e.g. r3).
        #[arg(long, group = "target")]
        region: Option<String>,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
        /// Write result to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Overwrite `--output` file if it exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Render an image as ASCII or Unicode blocks.
    Render {
        /// Image path, or `-` for stdin.
        image: PathBuf,
        /// Rendering engine.
        #[arg(long, value_enum, default_value_t = RendererArg::Ascii)]
        renderer: RendererArg,
        /// Output width in characters (capped at image resolution).
        #[arg(long, default_value_t = 100)]
        width: u32,
        /// Output height in rows (default: derived from width + aspect).
        #[arg(long)]
        height: Option<u32>,
        /// Terminal-cell aspect correction; 0.5 default, 1.0 = square.
        #[arg(long, default_value_t = 0.5)]
        aspect: f32,
        /// Flip luminance mapping: dark source → light glyphs.
        #[arg(long, default_value_t = false)]
        invert: bool,
        /// Color presentation: `none` (agent default), `grayscale`,
        /// `true-color` (per-cell RGB escapes; human display).
        #[arg(long, default_value = "none")]
        color: ColorMode,
        /// ANSI background as R,G,B (0-255 each); requires a --color mode
        /// other than none.
        #[arg(long)]
        background: Option<String>,
        /// ANSI 256 grayscale presentation shorthand (= --color grayscale).
        #[arg(long, default_value_t = false, conflicts_with = "color")]
        grayscale: bool,
        /// Charset: preset name (`standard|dense|blocks`) or custom ramp string.
        #[arg(long)]
        charset: Option<String>,
        /// Fit output to terminal width, preserving aspect (overrides
        /// --width; agents should prefer explicit --width).
        #[arg(long, default_value_t = false)]
        full: bool,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
        /// Write result to file instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Overwrite `--output` file if it exists.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum RendererArg {
    Ascii,
    Blocks,
}

impl From<RendererArg> for ae_render::config::RendererType {
    fn from(r: RendererArg) -> Self {
        match r {
            RendererArg::Ascii => Self::Ascii,
            RendererArg::Blocks => Self::Blocks,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum FormatArg {
    Text,
    Json,
}

fn main() {
    set_stdin_binary();
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Command::Capabilities { format } => cmd_capabilities(format),
        Command::Batch {
            path,
            width,
            no_render,
        } => cmd_batch(&path, width, no_render),
        Command::Inspect {
            image,
            width,
            no_render,
            format,
            output,
            force,
        } => cmd_inspect(InspectSpec {
            image,
            width,
            no_render,
            format,
            output,
            force,
        }),
        Command::Geometry {
            image,
            format,
            output,
            force,
        } => cmd_geometry(GeometrySpec {
            image,
            format,
            output,
            force,
        }),
        Command::Zoom {
            image,
            box_,
            level,
            width,
            format,
            output,
            force,
        } => cmd_zoom(ZoomSpec {
            image,
            box_,
            level,
            width,
            format,
            output,
            force,
        }),
        Command::Region {
            image,
            box_,
            region,
            format,
            output,
            force,
        } => cmd_region(RegionSpec {
            image,
            box_,
            region,
            format,
            output,
            force,
        }),
        Command::Render {
            image,
            renderer,
            width,
            height,
            aspect,
            invert,
            color,
            background,
            grayscale,
            charset,
            full,
            format,
            output,
            force,
        } => {
            let mut width = width;
            let mut height = height;
            if full {
                // --full overrides explicit dims with the terminal size.
                if let Some((cols, rows)) = terminal_size::terminal_size() {
                    width = cols.0 as u32;
                    height = Some(rows.0 as u32);
                }
            }
            // --grayscale is shorthand for --color grayscale.
            let color = if grayscale {
                ColorMode::Grayscale
            } else {
                color
            };
            cmd_render(RenderSpec {
                image,
                renderer: renderer.into(),
                width,
                height,
                aspect,
                invert,
                color,
                background,
                charset,
                format,
                output,
                force,
            })
        }
    };
    std::process::exit(exit_code);
}

/// `agent-eye.capabilities.v1` payload (plan §11).
#[derive(serde::Serialize)]
struct CapabilitiesV1 {
    schema_version: &'static str,
    version: &'static str,
    input: &'static [&'static str],
    renderers: &'static [&'static str],
    commands: &'static [&'static str],
    output_formats: &'static [&'static str],
    limits: LimitsInfo,
}

#[derive(serde::Serialize)]
struct LimitsInfo {
    max_pixels: u64,
    max_file_size_bytes: u64,
    max_region_count: u32,
    max_render_width: u32,
    max_render_height: u32,
    max_charset_length: usize,
}

fn cmd_capabilities(format: FormatArg) -> i32 {
    let caps = CapabilitiesV1 {
        schema_version: "agent-eye.capabilities.v1",
        version: env!("CARGO_PKG_VERSION"),
        input: &["png", "jpeg", "webp", "stdin"],
        renderers: &["ascii", "blocks"],
        // Commands shipped so far; grows as phases land.
        commands: &["render", "capabilities"],
        output_formats: &["text", "json"],
        limits: LimitsInfo {
            max_pixels: 25_000_000,
            max_file_size_bytes: 104_857_600,
            max_region_count: 500,
            max_render_width: 10_000,
            max_render_height: 10_000,
            max_charset_length: ae_render::MAX_CHARSET_LEN,
        },
    };
    match format {
        FormatArg::Text => {
            println!("ae {} — agent-eye capabilities", caps.version);
            println!("input:         {}", caps.input.join(", "));
            println!("renderers:     {}", caps.renderers.join(", "));
            println!("commands:      {}", caps.commands.join(", "));
            println!("output:        {}", caps.output_formats.join(", "));
            println!("limits:");
            println!(
                "  max_pixels={} max_file_size_bytes={}",
                caps.limits.max_pixels, caps.limits.max_file_size_bytes
            );
            println!(
                "  max_region_count={} max_render_width={} max_render_height={}",
                caps.limits.max_region_count,
                caps.limits.max_render_width,
                caps.limits.max_render_height
            );
            println!("  max_charset_length={}", caps.limits.max_charset_length);
        }
        FormatArg::Json => match serde_json::to_string_pretty(&caps) {
            Ok(s) => println!("{s}"),
            Err(e) => return fail(&e.to_string()),
        },
    }
    0
}

/// One JSONL record for batch output (agent-eye.scene.v1 + file name).
fn batch_record(path: &Path, width: u32, no_render: bool) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let img = decode_bytes(&bytes, &Limits::default()).ok()?;
    let cfg = ae_core::regions::DetectConfig::default();
    let regions = ae_core::regions::detect_regions(&img, &cfg).ok()?;
    let relations = ae_core::relations::compute_relations(&regions);

    let mut representation: Option<RepresentationOwned> = None;
    if !no_render {
        let charset = ae_render::presets::standard().ok()?;
        let rcfg = ae_render::RenderConfig {
            width: width.clamp(1, 10_000),
            ..Default::default()
        };
        let grid = ae_render::render::render(&img, &rcfg, &charset).ok()?;
        representation = Some(RepresentationOwned {
            renderer: "ascii",
            charset: grid.charset_name.clone(),
            data: grid.to_text(),
        });
    }

    let transform = ae_core::geometry::CoordinateTransform::new(
        img.bounds(),
        img.dimensions.width,
        img.dimensions.height,
    );
    let prov = ae_core::provenance::Provenance::compute(&bytes, img.bounds(), transform);

    let payload = SceneJsonV1 {
        schema_version: "agent-eye.scene.v1",
        image: ImageInfoOwned {
            width: img.dimensions.width,
            height: img.dimensions.height,
            format: img.metadata.format.clone(),
        },
        provenance: ProvenanceJson {
            source_hash: prov.source_hash,
            source_bounds: prov.source_bounds.to_array(),
        },
        regions: regions
            .iter()
            .map(|r| RegionInfo {
                id: r.id.clone(),
                bounds: r.bounds.to_array(),
                area: r.area,
                edge_density: r.edge_density,
                color_variance: r.color_variance,
            })
            .collect(),
        relations,
        representation,
        mapping: MappingInfo::from_transform(&transform),
    };
    // Prepend the source file so JSONL consumers can join results.
    let mut v = serde_json::to_value(&payload).ok()?;
    v["file"] = serde_json::Value::String(path.display().to_string());
    serde_json::to_string(&v).ok()
}

/// `ae batch <dir>`: parallel inspect of every decodable image in a
/// directory. Output is JSONL on stdout, ordered deterministically by
/// sorted path — rayon parallelism never reorders lines.
fn cmd_batch(dir: &Path, width: u32, no_render: bool) -> i32 {
    if !dir.is_dir() {
        return fail(&format!("{} is not a directory", dir.display()));
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return fail(&format!("{}: {e}", dir.display())),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("png") | Some("jpg") | Some("jpeg") | Some("webp")
            )
        })
        .collect();
    if files.is_empty() {
        return fail(&format!(
            "no png/jpeg/webp images found in {}",
            dir.display()
        ));
    }
    files.sort(); // deterministic ordering independent of FS enumeration

    use rayon::prelude::*;
    let records: Vec<Option<String>> = files
        .par_iter()
        .map(|p| batch_record(p, width, no_render))
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    use std::io::Write;
    for line in records.into_iter().flatten() {
        let _ = writeln!(out, "{line}");
    }
    0
}

struct InspectSpec {
    image: PathBuf,
    width: u32,
    no_render: bool,
    format: FormatArg,
    output: Option<PathBuf>,
    force: bool,
}

/// `agent-eye.scene.v1` payload — the orchestrated overview (plan §11).
#[derive(serde::Serialize)]
struct SceneJsonV1 {
    schema_version: &'static str,
    image: ImageInfoOwned,
    provenance: ProvenanceJson,
    regions: Vec<RegionInfo>,
    relations: Vec<ae_core::relations::Relation>,
    representation: Option<RepresentationOwned>,
    mapping: MappingInfo,
}

/// Orchestrates decode → detect → relate → render → provenance. The CLI is
/// orchestration only; every primitive lives in ae-core / ae-render.
fn cmd_inspect(spec: InspectSpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };
    let cfg = ae_core::regions::DetectConfig::default();
    let regions = match ae_core::regions::detect_regions(&img, &cfg) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };
    let relations = ae_core::relations::compute_relations(&regions);

    // Optional ASCII overview at the requested width.
    let mut representation: Option<RepresentationOwned> = None;
    let mut render_cfg: Option<(ae_render::RenderConfig, ae_render::Charset)> = None;
    if !spec.no_render {
        let charset = match ae_render::presets::standard() {
            Ok(c) => c,
            Err(e) => return fail(&e.to_string()),
        };
        let rcfg = ae_render::RenderConfig {
            width: spec.width.clamp(1, 10_000),
            ..Default::default()
        };
        match ae_render::render::render(&img, &rcfg, &charset) {
            Ok(grid) => {
                representation = Some(RepresentationOwned {
                    renderer: "ascii",
                    charset: grid.charset_name.clone(),
                    data: grid.to_text(),
                });
                render_cfg = Some((rcfg, charset));
            }
            Err(e) => return fail(&e.to_string()),
        }
    }
    let _ = &render_cfg; // kept for symmetry with region/zoom paths

    let transform = ae_core::geometry::CoordinateTransform::new(
        img.bounds(),
        img.dimensions.width,
        img.dimensions.height,
    );
    let prov = ae_core::provenance::Provenance::compute(&bytes, img.bounds(), transform);

    let body = match spec.format {
        FormatArg::Text => {
            let mut s = String::new();
            s.push_str(&format!(
                "image: {}x{} ({})\n",
                img.dimensions.width,
                img.dimensions.height,
                img.metadata.format.as_deref().unwrap_or("unknown")
            ));
            if let Some(repr) = &representation {
                s.push_str(repr.data.trim_end());
                s.push('\n');
            }
            s.push_str(&format!("regions: {}\n", regions.len()));
            for r in &regions {
                s.push_str(&format!(
                    "  {} bounds={:?} area={:.3} edge_density={:.3}\n",
                    r.id,
                    r.bounds.to_array(),
                    r.area,
                    r.edge_density
                ));
            }
            s.push_str(&format!("relations: {}\n", relations.len()));
            for rel in &relations {
                s.push_str(&format!("  {} {} {}\n", rel.kind, rel.a, rel.b));
            }
            s
        }
        FormatArg::Json => {
            let payload = SceneJsonV1 {
                schema_version: "agent-eye.scene.v1",
                image: ImageInfoOwned {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: img.metadata.format.clone(),
                },
                provenance: ProvenanceJson {
                    source_hash: prov.source_hash,
                    source_bounds: prov.source_bounds.to_array(),
                },
                regions: regions
                    .iter()
                    .map(|r| RegionInfo {
                        id: r.id.clone(),
                        bounds: r.bounds.to_array(),
                        area: r.area,
                        edge_density: r.edge_density,
                        color_variance: r.color_variance,
                    })
                    .collect(),
                relations,
                representation,
                mapping: MappingInfo::from_transform(&transform),
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => s,
                Err(e) => return fail(&e.to_string()),
            }
        }
    };
    emit(&body, spec.output.as_deref(), spec.force)
}

struct GeometrySpec {
    image: PathBuf,
    format: FormatArg,
    output: Option<PathBuf>,
    force: bool,
}

/// `agent-eye.geometry.v1` payload.
#[derive(serde::Serialize)]
struct GeometryJsonV1 {
    schema_version: &'static str,
    image: ImageInfoOwned,
    provenance: ProvenanceJson,
    regions: Vec<RegionInfo>,
    relations: Vec<ae_core::relations::Relation>,
    mapping: MappingInfo,
}

fn cmd_geometry(spec: GeometrySpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };
    let cfg = ae_core::regions::DetectConfig::default();
    let regions = match ae_core::regions::detect_regions(&img, &cfg) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };
    let relations = ae_core::relations::compute_relations(&regions);
    let transform = ae_core::geometry::CoordinateTransform::new(
        img.bounds(),
        img.dimensions.width,
        img.dimensions.height,
    );

    let body = match spec.format {
        FormatArg::Text => {
            let mut s = String::new();
            s.push_str(&format!(
                "image: {}x{} ({})\n",
                img.dimensions.width,
                img.dimensions.height,
                img.metadata.format.as_deref().unwrap_or("unknown")
            ));
            s.push_str(&format!("regions: {}\n", regions.len()));
            for r in &regions {
                s.push_str(&format!(
                    "  {} bounds={:?} area={:.3} edge_density={:.3} color_variance={:.3}\n",
                    r.id,
                    r.bounds.to_array(),
                    r.area,
                    r.edge_density,
                    r.color_variance
                ));
            }
            s.push_str(&format!("relations: {}\n", relations.len()));
            for rel in &relations {
                s.push_str(&format!("  {} {} {}\n", rel.kind, rel.a, rel.b));
            }
            s
        }
        FormatArg::Json => {
            let prov = ae_core::provenance::Provenance::compute(&bytes, img.bounds(), transform);
            let payload = GeometryJsonV1 {
                schema_version: "agent-eye.geometry.v1",
                image: ImageInfoOwned {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: img.metadata.format.clone(),
                },
                provenance: ProvenanceJson {
                    source_hash: prov.source_hash,
                    source_bounds: prov.source_bounds.to_array(),
                },
                regions: regions
                    .iter()
                    .map(|r| RegionInfo {
                        id: r.id.clone(),
                        bounds: r.bounds.to_array(),
                        area: r.area,
                        edge_density: r.edge_density,
                        color_variance: r.color_variance,
                    })
                    .collect(),
                relations,
                mapping: MappingInfo::from_transform(&transform),
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => s,
                Err(e) => return fail(&e.to_string()),
            }
        }
    };
    emit(&body, spec.output.as_deref(), spec.force)
}

struct ZoomSpec {
    image: PathBuf,
    box_: String,
    level: u32,
    width: u32,
    format: FormatArg,
    output: Option<PathBuf>,
    force: bool,
}

/// `agent-eye.zoom.v1` payload.
#[derive(serde::Serialize)]
struct ZoomJsonV1 {
    schema_version: &'static str,
    image: ImageInfoOwned,
    provenance: ProvenanceJson,
    zoom: ZoomInfo,
    representation: RepresentationOwned,
    mapping: MappingInfo,
}

#[derive(serde::Serialize)]
struct ZoomInfo {
    /// Spatial allocation scale: 1x, 2x, 4x, 8x.
    scale: f64,
    level: u32,
}

fn cmd_zoom(spec: ZoomSpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };
    let bounds = match parse_box(&spec.box_) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    if bounds.x2 > img.dimensions.width || bounds.y2 > img.dimensions.height {
        return fail(&format!(
            "box {:?} exceeds image {}x{}",
            bounds.to_array(),
            img.dimensions.width,
            img.dimensions.height
        ));
    }
    // Level → spatial allocation scale. The OUTPUT grid is fixed at
    // --width; a higher level renders the SAME grid from a SMALLER source
    // window, i.e. output samples per source pixel grow 2^level×.
    let level_scale = 2u32.pow(spec.level);
    let crop_w = (bounds.width() / level_scale).clamp(1, bounds.width());
    let crop_h = (bounds.height() / level_scale).clamp(1, bounds.height());
    let crop_bounds = match ae_core::geometry::HalfOpenBounds::new(
        bounds.x1,
        bounds.y1,
        bounds.x1 + crop_w,
        bounds.y1 + crop_h,
    ) {
        Ok(b) => b,
        Err(e) => return fail(&e.to_string()),
    };
    let cropped = crop_image(&img, crop_bounds);
    let charset = match ae_render::presets::standard() {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string()),
    };
    let cfg = ae_render::RenderConfig {
        renderer: Default::default(),
        width: spec.width.clamp(1, 10_000),
        height: None,      // derive from aspect
        aspect_ratio: 1.0, // zoom is pixel-true spatial allocation
        invert: false,
        charset_override: None,
        color: Default::default(),
        background: None,
    };
    let grid = match ae_render::render::render(&cropped, &cfg, &charset) {
        Ok(g) => g,
        Err(e) => return fail(&e.to_string()),
    };
    let out_w = grid.width().max(1) as u32;
    let out_h = grid.height().max(1) as u32;
    let transform = ae_core::geometry::CoordinateTransform::new(crop_bounds, out_w, out_h);

    let body = match spec.format {
        FormatArg::Text => grid.to_text(),
        FormatArg::Json => {
            let prov = ae_core::provenance::Provenance::compute(&bytes, crop_bounds, transform);
            let payload = ZoomJsonV1 {
                schema_version: "agent-eye.zoom.v1",
                image: ImageInfoOwned {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: img.metadata.format.clone(),
                },
                provenance: ProvenanceJson {
                    source_hash: prov.source_hash,
                    source_bounds: prov.source_bounds.to_array(),
                },
                zoom: ZoomInfo {
                    scale: f64::from(level_scale),
                    level: spec.level,
                },
                representation: RepresentationOwned {
                    renderer: "ascii",
                    charset: grid.charset_name.clone(),
                    data: grid.to_text(),
                },
                mapping: MappingInfo::from_transform(&transform),
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => s,
                Err(e) => return fail(&e.to_string()),
            }
        }
    };
    emit(&body, spec.output.as_deref(), spec.force)
}

struct RegionSpec {
    image: PathBuf,
    box_: Option<String>,
    region: Option<String>,
    format: FormatArg,
    output: Option<PathBuf>,
    force: bool,
}

/// `agent-eye.region.v1` payload.
#[derive(serde::Serialize)]
struct RegionJsonV1 {
    schema_version: &'static str,
    image: ImageInfoOwned,
    provenance: ProvenanceJson,
    region: RegionInfo,
    representation: RepresentationOwned,
    mapping: MappingInfo,
}

#[derive(serde::Serialize)]
struct ImageInfoOwned {
    width: u32,
    height: u32,
    format: Option<String>,
}

#[derive(serde::Serialize)]
struct RepresentationOwned {
    renderer: &'static str,
    charset: String,
    data: String,
}

#[derive(serde::Serialize)]
struct RegionInfo {
    id: String,
    bounds: [u32; 4],
    area: f32,
    edge_density: f32,
    color_variance: f64,
}

fn parse_box(s: &str) -> Result<ae_core::geometry::HalfOpenBounds, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("--box expects x,y,w,h".into());
    }
    let nums: Result<Vec<u32>, _> = parts.iter().map(|p| p.trim().parse()).collect();
    let n = nums.map_err(|e| format!("--box parse failed: {e}"))?;
    let (x, y, w, h) = (n[0], n[1], n[2], n[3]);
    if w == 0 || h == 0 {
        return Err("--box width/height must be > 0".into());
    }
    ae_core::geometry::HalfOpenBounds::new(x, y, x + w, y + h)
        .map_err(|e| format!("--box invalid: {e}"))
}

fn cmd_region(spec: RegionSpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };

    // Resolve target bounds: explicit --box wins; --region looks up a
    // detected candidate by id.
    let bounds = if let Some(bx) = &spec.box_ {
        match parse_box(bx) {
            Ok(b) => b,
            Err(e) => return fail(&e),
        }
    } else if let Some(rid) = &spec.region {
        let cfg = ae_core::regions::DetectConfig::default();
        let regions = match ae_core::regions::detect_regions(&img, &cfg) {
            Ok(r) => r,
            Err(e) => return fail(&e.to_string()),
        };
        match regions.iter().find(|r| r.id == *rid) {
            Some(r) => r.bounds,
            None => {
                return fail(&format!(
                    "region '{rid}' not found; run 'ae geometry {}' for ids",
                    spec.image.display()
                ))
            }
        }
    } else {
        return fail("one of --box or --region is required");
    };

    let region = match ae_core::regions::CandidateRegion::measure(&img, bounds) {
        Ok(r) => r,
        Err(e) => return fail(&e.to_string()),
    };
    // Render just the crop at its natural resolution capped to 80 cols.
    let out_w = bounds.width().clamp(1, 80);
    let out_h = bounds.height().clamp(1, 60);
    let transform = ae_core::geometry::CoordinateTransform::new(bounds, out_w.max(1), out_h);
    let charset = match ae_render::presets::standard() {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string()),
    };
    let cfg = ae_render::RenderConfig {
        renderer: Default::default(),
        width: out_w.max(1),
        height: Some(out_h),
        aspect_ratio: 1.0, // crop is pixel-true; no terminal correction
        invert: false,
        charset_override: None,
        color: Default::default(),
        background: None,
    };
    // Crop the source pixels so rendering covers exactly the window.
    let cropped = crop_image(&img, bounds);
    let grid = match ae_render::render::render(&cropped, &cfg, &charset) {
        Ok(g) => g,
        Err(e) => return fail(&e.to_string()),
    };

    let body = match spec.format {
        FormatArg::Text => grid.to_text(),
        FormatArg::Json => {
            let prov = ae_core::provenance::Provenance::compute(&bytes, bounds, transform);
            let payload = RegionJsonV1 {
                schema_version: "agent-eye.region.v1",
                image: ImageInfoOwned {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: img.metadata.format.clone(),
                },
                provenance: ProvenanceJson {
                    source_hash: prov.source_hash,
                    source_bounds: prov.source_bounds.to_array(),
                },
                region: RegionInfo {
                    id: if let Some(rid) = &spec.region {
                        rid.clone()
                    } else if region.id.is_empty() {
                        "custom".to_owned()
                    } else {
                        region.id.clone()
                    },
                    bounds: region.bounds.to_array(),
                    area: region.area,
                    edge_density: region.edge_density,
                    color_variance: region.color_variance,
                },
                representation: RepresentationOwned {
                    renderer: "ascii",
                    charset: grid.charset_name.clone(),
                    data: grid.to_text(),
                },
                mapping: MappingInfo::from_transform(&transform),
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => s,
                Err(e) => return fail(&e.to_string()),
            }
        }
    };
    emit(&body, spec.output.as_deref(), spec.force)
}

/// Extracts `bounds` into a new image (zero-copy semantics preserved by
/// cloning only the requested pixels).
fn crop_image(
    img: &ae_core::image::Image,
    b: ae_core::geometry::HalfOpenBounds,
) -> ae_core::image::Image {
    let out_w = b.width().max(1);
    let out_h = b.height().max(1);
    let dims = ae_core::image::Dimensions::new(out_w, out_h).unwrap();
    let mut pixels = Vec::with_capacity((out_w * out_h) as usize);
    for y in b.y1..b.y1 + out_h {
        for x in b.x1..b.x1 + out_w {
            pixels.push(img.pixels.get(x, y).unwrap_or_default());
        }
    }
    let buf = ae_core::image::PixelBuffer::from_vec(dims, pixels).unwrap();
    ae_core::image::Image::new(dims, buf, img.metadata.clone()).unwrap()
}

struct RenderSpec {
    image: PathBuf,
    renderer: ae_render::config::RendererType,
    width: u32,
    height: Option<u32>,
    aspect: f32,
    invert: bool,
    color: ColorMode,
    /// Raw `--background` value "R,G,B", parsed in cmd_render.
    background: Option<String>,
    charset: Option<String>,
    format: FormatArg,
    /// Write result here instead of stdout.
    output: Option<PathBuf>,
    /// Overwrite an existing `output` file.
    force: bool,
}

/// `agent-eye.render.v1` JSON payload — geometry + provenance only.
#[derive(serde::Serialize)]
struct RenderJsonV1<'a> {
    schema_version: &'static str,
    image: ImageInfo<'a>,
    /// SHA-256 of original bytes + bounds + affine map-back.
    provenance: ProvenanceJson,
    representation: Representation<'a>,
    mapping: MappingInfo,
}

#[derive(serde::Serialize)]
struct ProvenanceJson {
    source_hash: String,
    source_bounds: [u32; 4],
}

#[derive(serde::Serialize)]
struct ImageInfo<'a> {
    width: u32,
    height: u32,
    format: &'a Option<String>,
}

#[derive(serde::Serialize)]
struct Representation<'a> {
    renderer: &'a str,
    charset: &'a str,
    data: &'a str,
}
#[derive(serde::Serialize)]
struct MappingInfo {
    source_bounds: [u32; 4],
    output_width: usize,
    output_height: usize,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
}

impl MappingInfo {
    /// Builds the mapping from `ae-core`'s canonical affine transform so
    /// every JSON output shares one map-back implementation.
    fn from_transform(t: &ae_core::geometry::CoordinateTransform) -> Self {
        Self {
            source_bounds: [
                t.source_bounds.x1,
                t.source_bounds.y1,
                t.source_bounds.x2,
                t.source_bounds.y2,
            ],
            output_width: t.output_width as usize,
            output_height: t.output_height as usize,
            scale_x: t.scale_x,
            scale_y: t.scale_y,
            offset_x: t.offset_x,
            offset_y: t.offset_y,
        }
    }
}

fn cmd_render(spec: RenderSpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };
    // Parse --background "R,G,B" (each 0-255). Only meaningful with a
    // non-none color mode.
    let background = match &spec.background {
        Some(s) => {
            let parts: Result<Vec<u8>, _> = s.split(',').map(|p| p.trim().parse()).collect();
            let rgb = match parts.map_err(|e| format!("--background parse failed: {e}")) {
                Ok(v) => v,
                Err(m) => return fail(&m),
            };
            if rgb.len() != 3 {
                return fail("--background expects R,G,B (three 0-255 values)");
            }
            if spec.color == ColorMode::None {
                return fail("--background requires --color grayscale or true-color");
            }
            Some((rgb[0], rgb[1], rgb[2]))
        }
        None => None,
    };

    let cfg = RenderConfig {
        renderer: spec.renderer,
        width: spec.width,
        height: spec.height,
        aspect_ratio: spec.aspect,
        invert: spec.invert,
        charset_override: spec.charset.clone(),
        color: spec.color,
        background,
    };
    if let Err(e) = cfg.validate() {
        return fail(&e.to_string());
    }
    let charset: Charset = match cfg.resolve_charset() {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string()),
    };
    // render() revalidates and derives grid dims from the same config.
    let grid: RenderedGrid = match render::render(&img, &cfg, &charset) {
        Ok(g) => g,
        Err(e) => return fail(&e.to_string()),
    };
    let body = match spec.format {
        FormatArg::Text => grid.to_text(),
        FormatArg::Json => {
            let out_w = grid.width().max(1);
            let out_h = grid.height().max(1);
            let prov = ae_core::provenance::Provenance::compute(
                &bytes,
                img.bounds(),
                ae_core::geometry::CoordinateTransform::new(
                    img.bounds(),
                    out_w as u32,
                    out_h as u32,
                ),
            );
            let payload = RenderJsonV1 {
                schema_version: "agent-eye.render.v1",
                image: ImageInfo {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: &img.metadata.format,
                },
                provenance: ProvenanceJson {
                    source_hash: prov.source_hash,
                    source_bounds: prov.source_bounds.to_array(),
                },
                representation: Representation {
                    renderer: spec.renderer.as_str(),
                    charset: &grid.charset_name,
                    data: &grid.to_text(),
                },
                // Single source of truth for the affine map-back math.
                mapping: MappingInfo::from_transform(&ae_core::geometry::CoordinateTransform::new(
                    img.bounds(),
                    out_w as u32,
                    out_h as u32,
                )),
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => s,
                Err(e) => return fail(&e.to_string()),
            }
        }
    };
    emit(&body, spec.output.as_deref(), spec.force)
}

/// Writes `body` to `path` (refusing to clobber without `force`) or stdout.
fn emit(body: &str, path: Option<&Path>, force: bool) -> i32 {
    match path {
        None => {
            println!("{body}");
            0
        }
        Some(p) => {
            if p.exists() && !force {
                return fail(&format!(
                    "{} already exists (use --force to overwrite)",
                    p.display()
                ));
            }
            let opened = std::fs::File::create(p).map_err(|e| e.to_string());
            match opened.and_then(|f| {
                let mut w = std::io::BufWriter::new(f);
                w.write_all(body.as_bytes())
                    .and_then(|()| w.write_all(b"\n"))
                    .and_then(|()| w.flush())
                    .map_err(|e| e.to_string())
            }) {
                Ok(()) => 0,
                Err(e) => fail(&format!("{}: {e}", p.display())),
            }
        }
    }
}

fn read_input(path: &PathBuf) -> Result<Vec<u8>, String> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("stdin read failed: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

fn fail(msg: &str) -> i32 {
    eprintln!("ae: error: {msg}");
    1
}
