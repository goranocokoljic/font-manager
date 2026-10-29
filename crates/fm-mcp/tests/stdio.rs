//! The stdio shim (E27.7): the built `fm-mcp-stdio` against a real endpoint
//! over `MockApi` on an ephemeral 127.0.0.1 port, published in a scratch
//! home's `endpoint.json` (`FM_HOME`). It forwards initialize and
//! tools/list; without the app, with MCP off, or when the app goes away it
//! answers a JSON-RPC error and exits non-zero without hanging.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use fm_api::mock::MockApi;
use fm_api::settings::McpUpdate;
use fm_api::{API_VERSION, Api, SettingsService as _};
use fm_core::endpoint::{DISCOVERY_FILE, Discovery};
use fm_endpoint::Endpoint;
use fm_endpoint::mount::{Mount, MountBody, MountFuture, MountRequest};
use fm_mcp::HttpServer;
use fm_mcp::stdio::{FIRST_MESSAGE_WAIT, UNAVAILABLE};
use serde_json::Value;

/// Long enough for a slow CI runner, short enough to catch a hang.
const ANSWER: Duration = Duration::from_secs(20);

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const TOOLS_LIST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

/// The app's glue (`McpMount` in `fm_app_lib`).
struct McpMount(Arc<HttpServer>);

impl Mount for McpMount {
    fn call(&self, req: MountRequest) -> MountFuture {
        let service = self.0.service();
        Box::pin(async move {
            service
                .handle(req)
                .await
                .map(|body| Box::pin(body) as MountBody)
        })
    }
}

/// A scratch home (`FM_HOME`), removed on drop.
struct Home(PathBuf);

impl Home {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fm-mcp-stdio-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).expect("temp home");
        Self(dir)
    }

    /// Writes `endpoint.json` as the app does, naming this (running) process.
    fn publish(&self, endpoint: &Endpoint, token: &str) {
        let discovery = Discovery {
            url: format!("http://{}", endpoint.addr()),
            token: token.to_owned(),
            pid: std::process::id(),
            api_version: API_VERSION.to_string(),
        };
        std::fs::write(
            self.0.join("data").join(DISCOVERY_FILE),
            serde_json::to_vec(&discovery).expect("json"),
        )
        .expect("write endpoint.json");
    }

    fn shim(&self) -> Shim {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fm-mcp-stdio"))
            .env("FM_HOME", &self.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the shim starts");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stdin = child.stdin.take();
        Shim {
            child,
            stdin,
            lines,
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An app: the endpoint over `MockApi` with the MCP mount, published.
struct App {
    api: Arc<MockApi>,
    endpoint: Endpoint,
}

impl App {
    fn start(home: &Home, mcp_on: bool) -> Self {
        let api = Arc::new(MockApi::new());
        let dyn_api: Arc<dyn Api> = api.clone();
        let mount = Arc::new(McpMount(Arc::new(HttpServer::new(Arc::clone(&dyn_api)))));
        let endpoint = Endpoint::start_with_mcp(dyn_api, mount).expect("endpoint");
        api.set_mcp_enabled(McpUpdate { enabled: mcp_on })
            .expect("switch");
        home.publish(&endpoint, &api.endpoint_info().expect("info").token);
        Self { api, endpoint }
    }
}

/// The running shim: its stdin, and its stdout line by line.
struct Shim {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
}

impl Shim {
    fn send(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{line}").expect("write");
        stdin.flush().expect("flush");
    }

    /// The next stdout line as JSON; every stdout line must be JSON-RPC.
    fn next(&self, within: Duration) -> Value {
        let line = match self.lines.recv_timeout(within) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => panic!("no answer within {within:?}"),
            Err(RecvTimeoutError::Disconnected) => panic!("stdout closed without an answer"),
        };
        let value: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("stdout carries JSON-RPC only, got {line:?}: {e}"));
        assert_eq!(value["jsonrpc"], "2.0", "{line}");
        value
    }

    fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Waits for the exit; a shim still running after `within` is a hang.
    fn exit(&mut self, within: Duration) -> ExitStatus {
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status;
            }
            if start.elapsed() > within {
                let _ = self.child.kill();
                panic!("the shim did not exit within {within:?}");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stderr(&mut self) -> String {
        let mut text = String::new();
        if let Some(mut stderr) = self.child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
        }
        text
    }
}

impl Drop for Shim {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn assert_unavailable(answer: &Value, id: Option<u64>) {
    match id {
        Some(id) => assert_eq!(answer["id"], id, "{answer}"),
        None => assert!(answer.get("id").is_none_or(Value::is_null), "{answer}"),
    }
    assert_eq!(answer["error"]["message"], UNAVAILABLE, "{answer}");
    assert!(answer.get("result").is_none());
}

#[test]
fn the_shim_forwards_initialize_and_tools_list_to_the_app() {
    let home = Home::new("forward");
    let app = App::start(&home, true);
    let mut shim = home.shim();

    shim.send(INITIALIZE);
    let init = shim.next(ANSWER);
    assert_eq!(init["id"], 1, "{init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "font-manager");
    assert_eq!(
        init["result"]["serverInfo"]["version"],
        API_VERSION.to_string()
    );
    assert!(init["result"]["capabilities"]["tools"].is_object());

    shim.send(INITIALIZED);
    shim.send(TOOLS_LIST);
    let tools = shim.next(ANSWER);
    assert_eq!(tools["id"], 2, "{tools}");
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, fm_mcp::tools::NAMES, "the server's own tools");

    // The client closes stdin: the shim ends cleanly, and said nothing
    // more on stdout.
    shim.close_stdin();
    let status = shim.exit(ANSWER);
    assert!(status.success(), "{status:?}: {}", shim.stderr());
    assert!(shim.lines.try_recv().is_err(), "nothing after the answers");
    assert!(app.api.mcp_status().expect("status").enabled);
}

#[test]
fn without_the_app_the_shim_answers_the_error_and_exits_non_zero() {
    let home = Home::new("no-app");
    let mut shim = home.shim();
    shim.send(INITIALIZE);
    assert_unavailable(&shim.next(ANSWER), Some(1));
    let status = shim.exit(ANSWER);
    assert!(!status.success(), "{status:?}");
    assert!(
        shim.stderr().contains("not running"),
        "the reason goes to stderr"
    );
}

#[test]
fn a_stale_endpoint_file_is_no_app() {
    let home = Home::new("stale");
    // An app that published and stopped without removing the file.
    let app = App::start(&home, true);
    app.endpoint.stop();
    let mut shim = home.shim();
    shim.send(INITIALIZE);
    assert_unavailable(&shim.next(ANSWER), Some(1));
    assert!(!shim.exit(ANSWER).success());
}

#[test]
fn with_mcp_off_the_shim_answers_the_error_and_exits_non_zero() {
    let home = Home::new("off");
    let _app = App::start(&home, false);
    let mut shim = home.shim();
    shim.send(INITIALIZE);
    assert_unavailable(&shim.next(ANSWER), Some(1));
    let status = shim.exit(ANSWER);
    assert!(!status.success(), "{status:?}");
    assert!(shim.stderr().contains("turned off"));
}

#[test]
fn a_client_that_writes_nothing_gets_the_error_without_an_id_and_no_hang() {
    let home = Home::new("silent");
    let mut shim = home.shim();
    // stdin stays open and empty.
    assert_unavailable(&shim.next(FIRST_MESSAGE_WAIT + ANSWER), None);
    assert!(!shim.exit(ANSWER).success());
}

#[test]
fn when_the_app_goes_away_mid_session_open_requests_get_the_error() {
    let home = Home::new("gone");
    let app = App::start(&home, true);
    let mut shim = home.shim();
    shim.send(INITIALIZE);
    assert_eq!(shim.next(ANSWER)["id"], 1);
    shim.send(INITIALIZED);

    app.endpoint.stop();
    shim.send(TOOLS_LIST);
    assert_unavailable(&shim.next(ANSWER), Some(2));
}

#[test]
fn a_request_after_the_shim_saw_the_app_go_gets_the_error() {
    let home = Home::new("gone-first");
    let app = App::start(&home, true);
    let mut shim = home.shim();
    shim.send(INITIALIZE);
    assert_eq!(shim.next(ANSWER)["id"], 1);
    shim.send(INITIALIZED);

    app.endpoint.stop();
    // The shim sees the connection close before the next request comes.
    std::thread::sleep(Duration::from_secs(1));
    shim.send(TOOLS_LIST);
    assert_unavailable(&shim.next(ANSWER), Some(2));
    shim.close_stdin();
    assert!(!shim.exit(ANSWER).success());
}
