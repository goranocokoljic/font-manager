//! `fm-mcp-stdio` — the Font Manager MCP server for clients that start a
//! process (Claude Desktop). It forwards stdin/stdout to the running app's
//! `/mcp` endpoint ([`fm_mcp::stdio`]).

fn main() -> std::process::ExitCode {
    fm_mcp::stdio::main()
}
