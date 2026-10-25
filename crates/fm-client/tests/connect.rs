//! `fm_client::connect` finds the running app through `endpoint.json`, and
//! says `None` when the app is not there (E25.4 criterion 2).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::Served;
use fm_api::{LibraryService as _, SettingsService as _};
use fm_client::{CONNECT_TIMEOUT, connect};
use fm_core::endpoint::{DISCOVERY_FILE, Discovery};
use fm_platform::AppPaths;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fm-client-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn paths(data: &Path) -> AppPaths {
    AppPaths {
        data: data.to_path_buf(),
        config: data.join("config"),
        cache: data.join("cache"),
        logs: data.join("logs"),
        library: data.join("library"),
    }
}

fn publish(dir: &Path, url: &str, token: &str, pid: u32) {
    let discovery = Discovery {
        url: url.to_owned(),
        token: token.to_owned(),
        pid,
        api_version: fm_api::API_VERSION.to_string(),
    };
    std::fs::write(
        dir.join(DISCOVERY_FILE),
        serde_json::to_vec(&discovery).expect("json"),
    )
    .expect("write");
}

/// The id of a process that has ended: the test binary itself, asked only
/// to list its tests.
fn ended_pid() -> u32 {
    let exe = std::env::current_exe().expect("test binary");
    let mut child = std::process::Command::new(exe)
        .args(["--list", "--format", "terse"])
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

#[test]
fn the_connect_timeout_is_500_ms() {
    assert_eq!(CONNECT_TIMEOUT, Duration::from_millis(500));
}

#[test]
fn a_running_app_is_found_and_answers() {
    let served = Served::new();
    let dir = scratch("found");
    let token = served.api.endpoint_info().unwrap().token;
    let url = format!("http://{}", served.endpoint.addr());
    publish(&dir, &url, &token, std::process::id());
    let client = connect(&paths(&dir)).expect("the app is running");
    assert_eq!(client.addr(), served.endpoint.addr());
    assert_eq!(
        client.list_sources().unwrap(),
        served.api.list_sources().unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_discovery_file_is_none() {
    let dir = scratch("missing");
    assert!(connect(&paths(&dir)).is_none());
    assert!(connect(&paths(&dir.join("no-such-folder"))).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_discovery_file_is_none() {
    let dir = scratch("broken");
    std::fs::write(dir.join(DISCOVERY_FILE), "{ not json").expect("write");
    assert!(connect(&paths(&dir)).is_none());
    std::fs::write(dir.join(DISCOVERY_FILE), r#"{"url":"http://127.0.0.1:1"}"#).expect("write");
    assert!(connect(&paths(&dir)).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_process_that_is_gone_is_none_even_when_the_port_answers() {
    // The port answers (another program took it, say), but the app that
    // wrote the file is gone: not the app.
    let served = Served::new();
    let dir = scratch("gone");
    let url = format!("http://{}", served.endpoint.addr());
    publish(&dir, &url, "t", ended_pid());
    assert!(connect(&paths(&dir)).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_port_that_refuses_is_none_within_the_timeout() {
    let dir = scratch("refused");
    let url = format!("http://{}", closed_port());
    publish(&dir, &url, "t", std::process::id());
    let started = Instant::now();
    assert!(connect(&paths(&dir)).is_none());
    // A refusal on loopback is immediate; the bound is the timeout plus slack.
    assert!(
        started.elapsed() < CONNECT_TIMEOUT + Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_address_off_this_machine_is_none_and_gets_no_token() {
    let dir = scratch("remote");
    for url in [
        "http://192.0.2.1:4100",
        "https://127.0.0.1:4100",
        "127.0.0.1:4100",
        "http://localhost:4100",
    ] {
        publish(&dir, url, "secret", std::process::id());
        let started = Instant::now();
        assert!(connect(&paths(&dir)).is_none(), "{url}");
        assert!(
            started.elapsed() < Duration::from_millis(400),
            "{url} was dialled"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
