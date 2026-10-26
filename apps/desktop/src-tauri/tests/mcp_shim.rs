//! The app's executable started with `--mcp-stdio` (the AppImage's Claude
//! Desktop command, E27.7) is the stdio shim: no window, JSON-RPC on stdout.
//! Without a running app it answers the shim's error and exits non-zero
//! quickly; a window that opened instead would never answer.

use std::error::Error;
use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use fm_mcp::stdio::UNAVAILABLE;
use fm_platform::mcp_shim::SHIM_ARG;

type TestResult = Result<(), Box<dyn Error>>;

const ANSWER: Duration = Duration::from_secs(20);

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;

#[test]
fn the_app_started_with_the_shim_argument_is_the_shim() -> TestResult {
    // An empty scratch home: no endpoint.json, so no app to forward to.
    let home = std::env::temp_dir().join(format!("fm-app-mcp-shim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("data"))?;

    let mut child = Command::new(env!("CARGO_BIN_EXE_fm-app"))
        .arg(SHIM_ARG)
        .env("FM_HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("stdin")?;
    writeln!(stdin, "{INITIALIZE}")?;
    stdin.flush()?;

    let stdout = child.stdout.take().ok_or("stdout")?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let answer = rx.recv_timeout(ANSWER);

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if start.elapsed() > ANSWER {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&home);

    let answer: serde_json::Value = serde_json::from_str(&answer?)?;
    assert_eq!(answer["id"], 1, "{answer}");
    assert_eq!(answer["error"]["message"], UNAVAILABLE, "{answer}");
    let status = status.ok_or("the app did not exit: it did not run as the shim")?;
    assert!(!status.success(), "{status:?}");
    Ok(())
}
