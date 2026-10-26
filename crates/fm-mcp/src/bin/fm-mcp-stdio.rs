//! `fm-mcp-stdio` — the Font Manager MCP server for clients that start a
//! process (Claude Desktop). It forwards stdin/stdout to the running app's
//! `/mcp` endpoint ([`fm_mcp::stdio`]). The app folders are the user's, or
//! those under `FM_HOME` (a scratch home, as for `fm`).

use std::path::PathBuf;
use std::process::ExitCode;

use fm_platform::AppPaths;

fn main() -> ExitCode {
    let target = app_paths().and_then(|paths| fm_mcp::stdio::find(&paths));
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("fm-mcp-stdio: no async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let code = runtime.block_on(fm_mcp::stdio::run(target));
    // The stdin reader sits on a blocking thread that only a new line or EOF
    // wakes: do not wait for it.
    runtime.shutdown_background();
    code
}

/// `FM_HOME` when it is set to an absolute path, else the user's app folders.
fn app_paths() -> Result<AppPaths, String> {
    if let Some(home) = std::env::var_os("FM_HOME").map(PathBuf::from)
        && home.is_absolute()
    {
        return Ok(AppPaths::under(&home));
    }
    AppPaths::current().map_err(|e| format!("The app folders could not be found: {e}"))
}
