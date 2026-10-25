//! The built `fm` against a running app (E26.6): a real endpoint over
//! `MockApi`, published in a scratch home's `endpoint.json` (`FM_HOME`).
//! When it answers `fm` goes through it and never opens the index; a stale
//! file, no file or `--direct` is direct-link mode on the index.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use fm_api::mock::MockApi;
use fm_api::{
    API_VERSION, ActivationService, Api, Event, EventSource, FaceId, LibraryService,
    SettingsService,
};
use fm_client::CONNECT_TIMEOUT;
use fm_core::endpoint::{DISCOVERY_FILE, Discovery};
use fm_core::index::INDEX_FILE;
use fm_endpoint::Endpoint;
use fm_platform::AppPaths;
use serde_json::Value;

/// A scratch home (`FM_HOME`), removed on drop.
struct Home(PathBuf);

impl Home {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fm-cli-endpoint-{label}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("data")).expect("temp home");
        Self(dir)
    }

    fn paths(&self) -> AppPaths {
        AppPaths::under(&self.0)
    }

    /// Writes `endpoint.json` as the app does.
    fn publish(&self, addr: SocketAddr, token: &str, pid: u32) {
        let discovery = Discovery {
            url: format!("http://{addr}"),
            token: token.to_owned(),
            pid,
            api_version: API_VERSION.to_string(),
        };
        std::fs::write(
            self.paths().data.join(DISCOVERY_FILE),
            serde_json::to_vec(&discovery).expect("json"),
        )
        .expect("write endpoint.json");
    }

    fn index_file(&self) -> PathBuf {
        self.paths().data.join(INDEX_FILE)
    }

    fn fm(&self, args: &[&str]) -> Output {
        self.fm_in(&self.0, args)
    }

    /// `fm <args>` with `cwd` as its working folder.
    fn fm_in(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fm"))
            .args(args)
            .env("FM_HOME", &self.0)
            .current_dir(cwd)
            .output()
            .expect("fm runs")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The app: `MockApi` behind a real endpoint, published in `home`.
struct App {
    api: Arc<MockApi>,
    endpoint: Endpoint,
}

impl App {
    fn running(home: &Home) -> Self {
        let api = Arc::new(MockApi::new());
        let endpoint = Endpoint::start(Arc::clone(&api) as Arc<dyn Api>).expect("start");
        let token = api.endpoint_info().expect("info").token;
        home.publish(endpoint.addr(), &token, std::process::id());
        Self { api, endpoint }
    }
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("an exit code")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("one JSON value ({e}): {}", text(&out.stdout)))
}

/// The id of a process that has ended.
fn ended_pid() -> u32 {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fm"))
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    pid
}

/// A loopback port nothing listens on.
fn closed_port() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr")
}

/// The next event that `pick` takes, within 10 s.
fn wait_for<T>(events: &std::sync::mpsc::Receiver<Event>, pick: impl Fn(Event) -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = events.recv_timeout(left).expect("the event came");
        if let Some(found) = pick(event) {
            return found;
        }
    }
}

fn mode(home: &Home, args: &[&str]) -> String {
    let out = home.fm(args);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    json(&out)["mode"].as_str().expect("mode").to_owned()
}

#[test]
fn with_the_app_running_fm_goes_through_it_and_never_opens_the_index() {
    let home = Home::new("endpoint");
    let _app = App::running(&home);

    let out = home.fm(&["version", "--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v = json(&out);
    assert_eq!(v["mode"], "endpoint");
    // The app's answer: the mock's name and the API it serves.
    assert_eq!(v["name"], "Font Manager (mock)");
    assert_eq!(v["apiVersion"]["major"], API_VERSION.major);
    assert_eq!(v["apiVersion"]["minor"], API_VERSION.minor);
    assert_eq!(v["apiVersion"]["patch"], API_VERSION.patch);

    let out = home.fm(&["version"]);
    assert!(
        text(&out.stdout).contains("Mode: endpoint"),
        "{}",
        text(&out.stdout)
    );

    // The mock's families, which the empty scratch home does not have.
    let out = home.fm(&["--json", "list"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).contains("\"Inter\""),
        "{}",
        text(&out.stdout)
    );

    assert!(
        !home.index_file().exists(),
        "endpoint mode opened the index itself"
    );
}

#[test]
fn direct_forces_direct_link_while_the_app_runs() {
    let home = Home::new("direct-flag");
    let _app = App::running(&home);
    let out = home.fm(&["--direct", "version", "--json"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v = json(&out);
    assert_eq!(v["mode"], "direct");
    assert_ne!(v["name"], "Font Manager (mock)");
    assert!(home.index_file().is_file(), "direct-link opens the index");
    // The flag is global, like --json.
    assert_eq!(mode(&home, &["version", "--json", "--direct"]), "direct");

    let out = home.fm(&["--direct", "version"]);
    assert!(
        text(&out.stdout).contains("Mode: direct"),
        "{}",
        text(&out.stdout)
    );
}

#[test]
fn without_endpoint_json_fm_links_the_index_directly() {
    let home = Home::new("no-file");
    assert_eq!(mode(&home, &["version", "--json"]), "direct");
    assert!(home.index_file().is_file());
}

#[test]
fn activate_and_deactivate_in_endpoint_mode_go_through_the_app() {
    let home = Home::new("activate");
    let app = App::running(&home);
    let events = app.api.subscribe();
    let italic = FaceId(2);

    let out = home.fm(&["--json", "activate", "Inter-Italic"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let outcomes = json(&out);
    assert_eq!(outcomes[0]["face"], 2, "{outcomes}");
    assert_eq!(outcomes[0]["outcome"]["status"], "done", "{outcomes}");
    // The app's own activation state, and its event.
    let active = app.api.list_active().unwrap();
    assert!(active.iter().any(|a| a.face == italic), "{active:?}");
    let activated = wait_for(&events, |e| match e {
        Event::ActivationChanged { activated, .. } => Some(activated),
        _ => None,
    });
    assert_eq!(activated, vec![italic]);

    let out = home.fm(&["deactivate", "Inter-Italic"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("done"), "{}", text(&out.stdout));
    assert!(
        !app.api
            .list_active()
            .unwrap()
            .iter()
            .any(|a| a.face == italic),
        "still active"
    );
    let deactivated = wait_for(&events, |e| match e {
        Event::ActivationChanged { deactivated, .. } => Some(deactivated),
        _ => None,
    });
    assert_eq!(deactivated, vec![italic]);
    assert!(!home.index_file().exists());
}

#[test]
fn scan_in_endpoint_mode_follows_the_app_event_stream() {
    let home = Home::new("scan");
    let app = App::running(&home);
    let sources = app.api.list_sources().unwrap();
    assert!(!sources.is_empty(), "the mock has a source");

    let out = home.fm(&["scan"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    // Progress came from the app's ScanProgress events over GET /v1/events.
    let progress = text(&out.stderr);
    assert!(
        progress.contains(&format!("Scanning {}:", sources[0].path)),
        "{progress}"
    );
    assert!(progress.contains(" files"), "{progress}");
    // The counts are the app's ScanFinished.
    let out = home.fm(&["--json", "scan"]);
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    assert!(json(&out).is_array());
    assert!(!home.index_file().exists());
}

#[test]
fn a_stale_endpoint_json_falls_back_to_direct_link() {
    // The process that wrote it is gone (the port still answers: another
    // program took it, say).
    let home = Home::new("dead-pid");
    let app = App::running(&home);
    let token = app.api.endpoint_info().unwrap().token;
    home.publish(app.endpoint.addr(), &token, ended_pid());
    assert_eq!(mode(&home, &["version", "--json"]), "direct");

    // The port refuses.
    let home = Home::new("refused");
    home.publish(closed_port(), "t", std::process::id());
    assert_eq!(mode(&home, &["version", "--json"]), "direct");
}

#[test]
fn a_stale_endpoint_json_is_given_up_within_the_connect_timeout() {
    let slack = Duration::from_millis(250);
    let home = Home::new("stale-timing");
    let paths = home.paths();

    home.publish(closed_port(), "t", std::process::id());
    let started = Instant::now();
    assert!(fm_cli::link::endpoint(&paths, false).is_none());
    let refused = started.elapsed();
    assert!(
        refused < CONNECT_TIMEOUT + slack,
        "refused port: {refused:?}"
    );

    home.publish(closed_port(), "t", ended_pid());
    let started = Instant::now();
    assert!(fm_cli::link::endpoint(&paths, false).is_none());
    let dead = started.elapsed();
    assert!(dead < CONNECT_TIMEOUT + slack, "dead pid: {dead:?}");
    eprintln!("stale endpoint.json given up: refused port {refused:?}, dead pid {dead:?}");
}

#[test]
fn link_endpoint_finds_the_app_and_direct_skips_it() {
    let home = Home::new("link");
    let app = App::running(&home);
    let paths = home.paths();
    let client = fm_cli::link::endpoint(&paths, false).expect("the app answers");
    assert_eq!(client.addr(), app.endpoint.addr());
    assert!(fm_cli::link::endpoint(&paths, true).is_none());
}

#[test]
fn a_wrong_token_exits_4_and_points_to_settings_local_api() {
    let home = Home::new("wrong-token");
    let app = App::running(&home);
    home.publish(app.endpoint.addr(), "not-the-token", std::process::id());

    let out = home.fm(&["--json", "list"]);
    assert_eq!(code(&out), 4, "{}", text(&out.stderr));
    // With --json the error is the one JSON value on stderr.
    assert!(out.stdout.is_empty());
    let e: Value = serde_json::from_slice(&out.stderr).expect("the error as JSON");
    assert_eq!(e["code"], "permissionDenied", "{e}");
    assert!(
        e["message"]
            .as_str()
            .unwrap()
            .contains("Settings > Local API"),
        "{e}"
    );

    let out = home.fm(&["list"]);
    assert_eq!(code(&out), 4);
    assert!(
        text(&out.stderr).contains("Settings > Local API"),
        "{}",
        text(&out.stderr)
    );
    // A wrong token is not a stale file: no fallback to the index.
    assert!(!home.index_file().exists());
}

/// `path` names `name` in the folder `dir` (both canonical, so a temp
/// folder behind a symlink, as on macOS, compares equal).
fn is_in(path: &str, dir: &Path, name: &str) -> bool {
    let path = Path::new(path);
    path.is_absolute()
        && path.file_name().is_some_and(|n| n == name)
        && path
            .parent()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .is_some_and(|p| p == std::fs::canonicalize(dir).expect("dir"))
}

#[test]
fn relative_out_paths_are_fm_s_own_not_the_app_s_in_endpoint_mode() {
    // The app writes the file in endpoint mode: the path it gets must be
    // the one the user meant, in fm's working folder.
    let home = Home::new("relative-out");
    let _app = App::running(&home);
    let work = home.0.join("work");
    std::fs::create_dir_all(work.join("web")).expect("work folder");

    let out = home.fm_in(
        &work,
        &["--json", "specimen", "Inter", "--out", "inter.html"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let path = json(&out)["path"].as_str().expect("path").to_owned();
    assert!(is_in(&path, &work, "inter.html"), "{path}");

    let out = home.fm_in(
        &work,
        &["--json", "export", "Inter", "--woff2", "--out", "web"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out.stderr));
    let v = json(&out);
    let files = v["files"].as_array().expect("files");
    assert!(!files.is_empty(), "{v}");
    for file in files {
        let path = file.as_str().expect("a path");
        let name = Path::new(path)
            .file_name()
            .expect("name")
            .to_str()
            .expect("utf-8");
        assert!(is_in(path, &work.join("web"), name), "{path}");
    }

    // A file of that name in fm's working folder is refused without --force.
    std::fs::write(work.join("taken.html"), b"keep").expect("write");
    let out = home.fm_in(&work, &["specimen", "Inter", "--out", "taken.html"]);
    assert_eq!(code(&out), 2, "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read(work.join("taken.html")).expect("read"),
        b"keep"
    );
}
