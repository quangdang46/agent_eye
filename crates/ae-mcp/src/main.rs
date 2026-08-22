//! ae-mcp binary: runs the stdio MCP server loop (one JSON-RPC line per request).
fn main() -> anyhow::Result<()> {
    ae_mcp::run()
}
