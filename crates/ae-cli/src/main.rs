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
use std::io::Read;
use std::path::PathBuf;

use ae_core::decode::decode_bytes;
use ae_core::image::Limits;
use ae_render::config::RenderConfig;
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
        /// Charset: preset name (`standard|dense|blocks`) or custom ramp string.
        #[arg(long)]
        charset: Option<String>,
        /// Output serialization.
        #[arg(long, value_enum, default_value_t = FormatArg::Text)]
        format: FormatArg,
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
        Command::Render {
            image,
            renderer,
            width,
            height,
            aspect,
            invert,
            charset,
            format,
        } => cmd_render(RenderSpec {
            image,
            renderer: renderer.into(),
            width,
            height,
            aspect,
            invert,
            charset,
            format,
        }),
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

struct RenderSpec {
    image: PathBuf,
    renderer: ae_render::config::RendererType,
    width: u32,
    height: Option<u32>,
    aspect: f32,
    invert: bool,
    charset: Option<String>,
    format: FormatArg,
}

/// `agent-eye.render.v1` JSON payload — geometry + provenance only.
#[derive(serde::Serialize)]
struct RenderJsonV1<'a> {
    schema_version: &'static str,
    image: ImageInfo<'a>,
    representation: Representation<'a>,
    mapping: MappingInfo,
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

fn cmd_render(spec: RenderSpec) -> i32 {
    let bytes = match read_input(&spec.image) {
        Ok(b) => b,
        Err(e) => return fail(&e),
    };
    let img = match decode_bytes(&bytes, &Limits::default()) {
        Ok(i) => i,
        Err(e) => return fail(&e.to_string()),
    };

    let cfg = RenderConfig {
        renderer: spec.renderer,
        width: spec.width,
        height: spec.height,
        aspect_ratio: spec.aspect,
        invert: spec.invert,
        charset_override: spec.charset.clone(),
        color: Default::default(),
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

    match spec.format {
        FormatArg::Text => println!("{}", grid.to_text()),
        FormatArg::Json => {
            let out_w = grid.width().max(1);
            let out_h = grid.height().max(1);
            let payload = RenderJsonV1 {
                schema_version: "agent-eye.render.v1",
                image: ImageInfo {
                    width: img.dimensions.width,
                    height: img.dimensions.height,
                    format: &img.metadata.format,
                },
                representation: Representation {
                    renderer: spec.renderer.as_str(),
                    charset: &grid.charset_name,
                    data: &grid.to_text(),
                },
                mapping: MappingInfo {
                    source_bounds: [0, 0, img.dimensions.width, img.dimensions.height],
                    output_width: out_w,
                    output_height: out_h,
                    scale_x: f64::from(img.dimensions.width) / f64::from(out_w as u32),
                    scale_y: f64::from(img.dimensions.height) / f64::from(out_h as u32),
                    offset_x: 0.0,
                    offset_y: 0.0,
                },
            };
            match serde_json::to_string_pretty(&payload) {
                Ok(s) => println!("{s}"),
                Err(e) => return fail(&e.to_string()),
            }
        }
    }
    0
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
