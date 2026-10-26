// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    // Started as the MCP stdio shim (the AppImage's Claude Desktop command,
    // E27.7): no window, only the shim.
    if fm_platform::mcp_shim::is_shim_launch(std::env::args_os()) {
        return fm_mcp::stdio::main();
    }
    fm_app_lib::run();
    ExitCode::SUCCESS
}
