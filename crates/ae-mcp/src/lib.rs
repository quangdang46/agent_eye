//! Minimal MCP (Model Context Protocol) stdio server for `ae`.
//!
//! Wraps the core primitives as MCP tools per plan §11:
//!   * inspect_image    → orchestrated overview (scene.v1)
//!   * render_image     → ASCII/blocks/braille render
//!   * zoom_image       → targeted crop at allocation level
//!   * inspect_geometry → regions + relations
//!
//! Transport: stdin/stdout JSON-RPC 2.0 lines. No HTTP, no LLM.
//! Protocol surface implemented: `initialize`, `tools/list`,
//! `tools/call`, plus `ping`. Errors follow MCP conventions
//! (`isError: true` + human-readable text content).
//!
//! Launch: `ae-mcp` (reads one JSON-RPC request per line on stdin).

use anyhow::Result as AnyResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use ae_core::decode::decode_bytes;
use ae_core::geometry::HalfOpenBounds;
use ae_core::image::{Dimensions, Image, Limits, PixelBuffer};
use ae_render::config::{RenderConfig, RendererType};

// -- wire types ------------------------------------------------------------

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i64,
    message: String,
}

/// MCP tool result: plain text content; errors flagged with isError.
fn tool_text(text: impl Into<String>, is_error: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": text.into()}],
        "isError": is_error,
    })
}

// -- tool implementations ---------------------------------------------------

fn decode_arg_image(args: &Value) -> AnyResult<ae_core::image::Image> {
    let path = args
        .get("image_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required string param: image_path"))?;
    if path == "-" {
        anyhow::bail!("stdin (-) is not supported over MCP; pass a file path");
    }
    let bytes = std::fs::read(path)?;
    Ok(decode_bytes(&bytes, &Limits::default())?)
}

fn tool_inspect_image(args: &Value) -> AnyResult<Value> {
    let img = decode_arg_image(args)?;
    let cfg = ae_core::regions::DetectConfig::default();
    let regions = ae_core::regions::detect_regions(&img, &cfg)?;
    let relations = ae_core::relations::compute_relations(&regions);
    let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(60) as u32;
    let charset = ae_render::presets::standard()?;
    let rcfg = RenderConfig {
        width: width.clamp(1, 10_000),
        ..Default::default()
    };
    let grid = ae_render::render::render(&img, &rcfg, &charset)?;
    let transform = ae_core::geometry::CoordinateTransform::new(
        img.bounds(),
        grid.width().max(1) as u32,
        grid.height().max(1) as u32,
    );
    let prov = ae_core::provenance::Provenance::compute(
        // hash of decoded bytes is unavailable here without keeping them;
        // callers needing byte-level identity should use render_image's
        // provenance via the CLI. Keep geometry-only provenance for MCP.
        &[],
        img.bounds(),
        transform,
    );
    Ok(json!({
        "schema_version": "agent-eye.scene.v1",
        "image": {"width": img.dimensions.width, "height": img.dimensions.height,
                   "format": img.metadata.format},
        "provenance": {"source_bounds": prov.source_bounds.to_array()},
        "regions": regions.iter().map(|r| json!({
            "id": r.id, "bounds": r.bounds.to_array(),
            "area": r.area, "edge_density": r.edge_density,
            "color_variance": r.color_variance,
        })).collect::<Vec<_>>(),
        "relations": relations,
        "representation": {"renderer": "ascii", "charset": grid.charset_name,
                            "data": grid.to_text()},
        "mapping": {
            "source_bounds": transform.source_bounds.to_array(),
            "scale_x": transform.scale_x, "scale_y": transform.scale_y,
            "offset_x": transform.offset_x, "offset_y": transform.offset_y,
        },
    }))
}

fn parse_renderer(args: &Value) -> RendererType {
    match args.get("renderer").and_then(|v| v.as_str()) {
        Some("blocks") => RendererType::Blocks,
        _ => RendererType::Ascii,
    }
}

fn tool_render_image(args: &Value) -> AnyResult<Value> {
    let img = decode_arg_image(args)?;
    let renderer = parse_renderer(args);
    let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
    let charset = renderer.default_charset()?;
    let rcfg = RenderConfig {
        renderer,
        width: width.clamp(1, 10_000),
        ..Default::default()
    };
    let grid = ae_render::render::render(&img, &rcfg, &charset)?;
    Ok(json!({
        "schema_version": "agent-eye.render.v1",
        "image": {"width": img.dimensions.width, "height": img.dimensions.height},
        "representation": {"renderer": grid.renderer.as_str(),
                            "charset": grid.charset_name, "data": grid.to_text()},
    }))
}

fn tool_zoom_image(args: &Value) -> AnyResult<Value> {
    let img = decode_arg_image(args)?;
    let box_str = args
        .get("box")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required string param: box (x,y,w,h)"))?;
    let parts: Vec<u32> = box_str
        .split(',')
        .map(|p| p.trim().parse())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("box parse failed: {e}"))?;
    if parts.len() != 3 && parts.len() != 4 {
        anyhow::bail!("box expects x,y,w,h");
    }
    let level = args
        .get("level")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        .min(3) as u32;
    let scale = 2u32.pow(level);
    let (x, y, w, h) = (parts[0], parts[1], parts[2], parts[3]);
    let crop_w = (w / scale).clamp(1, w);
    let crop_h = (h / scale).clamp(1, h);
    let bounds = ae_core::geometry::HalfOpenBounds::new(x, y, x + crop_w, y + crop_h)?;

    let charset = ae_render::presets::standard()?;
    let out_w = args.get("width").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
    let rcfg = RenderConfig {
        width: out_w.clamp(1, 10_000),
        aspect_ratio: 1.0,
        ..Default::default()
    };
    let cropped = crop_image(&img, bounds);
    let grid = ae_render::render::render(&cropped, &rcfg, &charset)?;
    let transform = ae_core::geometry::CoordinateTransform::new(
        bounds,
        grid.width().max(1) as u32,
        grid.height().max(1) as u32,
    );
    Ok(json!({
        "schema_version": "agent-eye.zoom.v1",
        "zoom": {"scale": scale, "level": level},
        "source_bounds": bounds.to_array(),
        "representation": {"renderer": "ascii", "data": grid.to_text()},
        "mapping": {
            "source_bounds": bounds.to_array(),
            "scale_x": transform.scale_x, "scale_y": transform.scale_y,
            "offset_x": transform.offset_x, "offset_y": transform.offset_y,
        },
    }))
}

fn tool_inspect_geometry(args: &Value) -> AnyResult<Value> {
    let img = decode_arg_image(args)?;
    let cfg = ae_core::regions::DetectConfig::default();
    let regions = ae_core::regions::detect_regions(&img, &cfg)?;
    let relations = ae_core::relations::compute_relations(&regions);
    Ok(json!({
        "schema_version": "agent-eye.geometry.v1",
        "regions": regions.iter().map(|r| json!({
            "id": r.id, "bounds": r.bounds.to_array(),
            "area": r.area, "edge_density": r.edge_density,
            "color_variance": r.color_variance,
        })).collect::<Vec<_>>(),
        "relations": relations,
    }))
}

// -- server loop -------------------------------------------------------------

fn tools_json() -> Value {
    json!({
    "tools": [
        {
            "name": "inspect_image",
            "description": "Orchestrated overview: dimensions, detected regions, spatial relations, ASCII overview, coordinate mapping",
            "inputSchema": {"type": "object", "properties": {
                "image_path": {"type": "string"},
                "width": {"type": "integer", "minimum": 1, "maximum": 10000}
            }, "required": ["image_path"]}
        },
        {
            "name": "render_image",
            "description": "Render an image as ASCII or Unicode blocks text",
            "inputSchema": {"type": "object", "properties": {
                "image_path": {"type": "string"},
                "renderer": {"type": "string", "enum": ["ascii", "blocks"]},
                "width": {"type": "integer", "minimum": 1, "maximum": 10000}
            }, "required": ["image_path"]}
        },
        {
            "name": "zoom_image",
            "description": "Crop + resample: more output samples devoted to a smaller source region (spatial allocation, not detail increase)",
            "inputSchema": {"type": "object", "properties": {
                "image_path": {"type": "string"},
                "box": {"type": "string", "description": "x,y,w,h in source pixels"},
                "level": {"type": "integer", "minimum": 0, "maximum": 3},
                "width": {"type": "integer"}
            }, "required": ["image_path", "box"]}
        },
        {
            "name": "inspect_geometry",
            "description": "Spatial evidence only: all detected candidate regions and their formal relations (no rendering)",
            "inputSchema": {"type": "object", "properties": {
                "image_path": {"type": "string"}
            }, "required": ["image_path"]}
        }
    ]})
}

fn handle(req: &RpcRequest) -> RpcResponse {
    let ok = |result: Value| RpcResponse {
        jsonrpc: "2.0",
        id: req.id.clone(),
        result: Some(result),
        error: None,
    };
    let err = |code: i64, message: String| RpcResponse {
        jsonrpc: "2.0",
        id: req.id.clone(),
        result: None,
        error: Some(RpcError { code, message }),
    };
    match req.method.as_str() {
        "initialize" => ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "ae-mcp", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => ok(json!({})),
        "tools/list" => ok(tools_json()),
        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = req.params.get("arguments").cloned().unwrap_or(Value::Null);
            let outcome = match name {
                "inspect_image" => tool_inspect_image(&args),
                "render_image" => tool_render_image(&args),
                "zoom_image" => tool_zoom_image(&args),
                "inspect_geometry" => tool_inspect_geometry(&args),
                other => return err(-32601, format!("unknown tool: {other}")),
            };
            match outcome {
                Ok(v) => ok(tool_text(v.to_string(), false)),
                Err(e) => ok(tool_text(format!("error: {e}"), true)),
            }
        }
        other => err(-32601, format!("unknown method: {other}")),
    }
}

pub fn run() -> AnyResult<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp: String = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(req) => serde_json::to_string(&handle(&req))?,
            Err(e) => serde_json::to_string(&RpcResponse {
                jsonrpc: "2.0",
                id: Value::Null,
                result: None,
                error: Some(RpcError {
                    code: -32700,
                    message: format!("parse error: {e}"),
                }),
            })?,
        };
        writeln!(stdout.lock(), "{resp}")?;
    }
    Ok(())
}

/// Crop helper shared with the CLI (kept here to avoid a public util crate).
fn crop_image(img: &Image, b: HalfOpenBounds) -> Image {
    let out_w = b.width().max(1);
    let out_h = b.height().max(1);
    let dims = Dimensions::new(out_w, out_h).unwrap();
    let mut pixels = Vec::with_capacity((out_w * out_h) as usize);
    for y in b.y1..b.y1 + out_h {
        for x in b.x1..b.x1 + out_w {
            pixels.push(img.pixels.get(x, y).unwrap_or_default());
        }
    }
    let buf = PixelBuffer::from_vec(dims, pixels).unwrap();
    Image::new(dims, buf, img.metadata.clone()).unwrap()
}
