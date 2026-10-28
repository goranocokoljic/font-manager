//! The app hosts the local API endpoint over the real library (E25.1): it
//! starts off the window's thread, writes a private `endpoint.json` with
//! the loopback address, answers with the token, and the quit path stops it
//! and removes the file.

use std::error::Error;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;

use fm_api::SettingsService;
use fm_api::settings::McpClient;
use fm_app_lib::serve;
use fm_core::endpoint::{DISCOVERY_FILE, Discovery};
use fm_platform::{AppPaths, MockActivator};

type TestResult = Result<(), Box<dyn Error>>;

struct Home {
    root: PathBuf,
    paths: AppPaths,
}

impl Home {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let root = std::env::temp_dir().join(format!("fm-app-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        };
        fm_core::startup::prepare(&paths)?;
        Ok(Self { root, paths })
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// `POST /v1/<method>` with `token`; the status code.
fn post(url: &str, token: &str, method: &str) -> Result<u16, Box<dyn Error>> {
    let host = url.strip_prefix("http://").ok_or("not an http url")?;
    let mut stream = TcpStream::connect(host)?;
    write!(
        stream,
        "POST /v1/{method} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer)?;
    let status = answer.split(' ').nth(1).ok_or("no status line")?;
    Ok(status.parse()?)
}

#[test]
fn the_app_publishes_the_endpoint_and_removes_the_file_on_quit() -> TestResult {
    let home = Home::new("endpoint")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    let core = core.ok_or("no core")?;
    let file = home.paths.data.join(DISCOVERY_FILE);
    assert!(!file.exists());

    let started = state.start_endpoint().ok_or("the endpoint did not start")?;
    started.join().map_err(|_| "start thread panicked")?;
    let addr = state.endpoint_addr().ok_or("not running")?;
    assert!(addr.ip().is_loopback(), "{addr}");
    assert_eq!(addr.ip().to_string(), "127.0.0.1");

    assert!(
        fm_platform::fs::is_private_file(&file)?,
        "endpoint.json is readable by others"
    );
    let discovery: Discovery = serde_json::from_slice(&std::fs::read(&file)?)?;
    let info = core.endpoint_info()?;
    assert_eq!(discovery.url, format!("http://{addr}"));
    assert_eq!(info.url.as_deref(), Some(discovery.url.as_str()));
    assert_eq!(discovery.token, info.token);
    assert_eq!(discovery.pid, std::process::id());
    assert_eq!(discovery.api_version, fm_api::API_VERSION.to_string());

    assert_eq!(post(&discovery.url, &discovery.token, "list_sources")?, 200);
    assert_eq!(post(&discovery.url, &"0".repeat(64), "list_sources")?, 401);

    state.run_quit_path();
    assert!(!file.exists(), "endpoint.json is left after quit");
    assert_eq!(state.endpoint_addr(), None);
    // The app tries the same ten ports first (ADR 0015), so a parallel
    // test's endpoint may take the freed port at once: the proof is that
    // nobody answers this app's token any more (a stranger refuses it, 401).
    assert_ne!(
        post(&discovery.url, &discovery.token, "list_sources").ok(),
        Some(200),
        "{addr} still serves this app after quit"
    );
    assert_eq!(core.endpoint_info()?.url, None);
    // The token is kept for the next start.
    assert_eq!(core.endpoint_info()?.token, discovery.token);
    Ok(())
}

/// `POST /mcp` with an MCP `initialize` request and `token`; the status code.
fn post_mcp(url: &str, token: &str) -> Result<u16, Box<dyn Error>> {
    let host = url.strip_prefix("http://").ok_or("not an http url")?;
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;
    let mut stream = TcpStream::connect(host)?;
    write!(
        stream,
        "POST /mcp HTTP/1.1
Host: {host}
Authorization: Bearer {token}
Content-Type: application/json
Accept: application/json, text/event-stream
Content-Length: {}
Connection: close

{body}",
        body.len()
    )?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer)?;
    let status = answer.split(' ').nth(1).ok_or("no status line")?;
    Ok(status.parse()?)
}

#[test]
fn the_mcp_server_is_off_by_default_and_the_switch_applies_without_a_restart() -> TestResult {
    let home = Home::new("endpoint-mcp")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (state, core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    let core = core.ok_or("no core")?;
    state
        .start_endpoint()
        .ok_or("no start")?
        .join()
        .map_err(|_| "panicked")?;
    let addr = state.endpoint_addr().ok_or("not running")?;
    let base = format!("http://{addr}");
    let token = core.endpoint_info()?.token;

    let off = core.mcp_status()?;
    assert!(!off.enabled, "off by default");
    assert_eq!(off.url.as_deref(), Some(format!("{base}/mcp").as_str()));
    let (desktop, http): (Vec<_>, Vec<_>) = off
        .snippets
        .iter()
        .partition(|s| s.client == McpClient::ClaudeDesktop);
    assert_eq!(http.len(), 2);
    assert!(http.iter().all(|s| s.text.contains(&token)));
    // The stdio shim beside the app's executable (E27.7), without the token.
    let shim = fm_platform::mcp_shim::current().ok_or("no folder")?;
    let [desktop] = desktop.as_slice() else {
        return Err("one Claude Desktop snippet".into());
    };
    let json: serde_json::Value = serde_json::from_str(&desktop.text)?;
    assert_eq!(
        json["mcpServers"]["font-manager"]["command"],
        shim.program.to_string_lossy().as_ref()
    );
    assert!(!desktop.text.contains(&token));
    assert_eq!(post_mcp(&base, &token)?, 404);

    core.set_mcp_enabled(fm_api::settings::McpUpdate { enabled: true })?;
    assert_eq!(post_mcp(&base, &token)?, 200);
    assert_eq!(post_mcp(&base, &"0".repeat(64))?, 401);
    // Saved in config.json, so the next start keeps it.
    let config = fm_core::config::Config::load(&home.paths.config_file())?;
    assert!(config.mcp_enabled);

    core.set_mcp_enabled(fm_api::settings::McpUpdate { enabled: false })?;
    assert_eq!(post_mcp(&base, &token)?, 404);
    assert_eq!(state.endpoint_addr(), Some(addr), "the same endpoint");
    state.run_quit_path();
    Ok(())
}

#[test]
fn an_update_that_did_not_install_serves_the_endpoint_again() -> TestResult {
    let home = Home::new("endpoint-resume")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (state, _core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    state
        .start_endpoint()
        .ok_or("no start")?
        .join()
        .map_err(|_| "panicked")?;
    state.run_quit_path();
    assert_eq!(state.endpoint_addr(), None);
    state.resume_after_quit(Vec::new());
    // `resume_after_quit` starts it on its own thread; wait for the file.
    let file = home.paths.data.join(DISCOVERY_FILE);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !file.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(file.exists(), "endpoint.json not written again");
    assert!(state.endpoint_addr().is_some());
    state.run_quit_path();
    assert!(!file.exists());
    Ok(())
}

#[test]
fn without_the_real_library_there_is_no_endpoint() {
    let state = fm_app_lib::AppState::new(Arc::new(fm_api::mock::MockApi::new()));
    assert!(state.start_endpoint().is_none());
    assert_eq!(state.endpoint_addr(), None);
}

/// The IPC endpoint (E29.1) starts with the local API and the quit path
/// stops it; a `hello` over it answers the API version.
#[test]
fn the_app_serves_the_ipc_endpoint_until_quit() -> TestResult {
    use std::io::BufRead;

    let home = Home::new("ipc")?;
    let index = fm_core::startup::open_index(&home.paths);
    let (state, _core) = serve(&home.paths, index, Arc::new(MockActivator::new()));
    // Not the user's real pipe or socket: a name of this test's own.
    let name = if cfg!(windows) {
        fm_platform::IpcName::Pipe(format!(r"\\.\pipe\fm-app-ipc-{}", std::process::id()))
    } else {
        fm_platform::IpcName::Socket(home.root.join("run").join("fm.sock"))
    };
    let state = state.with_ipc(name.clone());
    let started = state.start_endpoint().ok_or("the endpoint did not start")?;
    started.join().map_err(|_| "start thread panicked")?;

    let mut client = std::io::BufReader::new(fm_platform::ipc::connect(&name)?);
    client
        .get_mut()
        .write_all(b"{\"v\":1,\"id\":1,\"method\":\"hello\"}\n")?;
    client.get_mut().flush()?;
    let mut reply = String::new();
    client.read_line(&mut reply)?;
    let reply: serde_json::Value = serde_json::from_str(&reply)?;
    assert_eq!(reply["result"], serde_json::to_value(fm_api::API_VERSION)?);
    drop(client);

    state.run_quit_path();
    assert!(
        fm_platform::ipc::connect(&name).is_err(),
        "{name} still accepts after quit"
    );
    if let fm_platform::IpcName::Socket(path) = &name {
        assert!(!path.exists(), "the socket is left after quit");
    }
    Ok(())
}
