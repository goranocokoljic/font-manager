//! A second process activates a fixture face over the IPC endpoint and
//! gets the outcomes back (E29.1 criterion 5): `CoreApi` with a mock
//! activator, the client a child run of this test binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use fm_api::library::{AddSourceRequest, ImportMode, ListFamiliesRequest};
use fm_api::{ActivationService as _, Api, Event, EventSource as _, LibraryService as _};
use fm_core::{CoreApi, Index, SettingsStore};
use fm_endpoint::ipc::IpcEndpoint;
use fm_platform::{AppPaths, FontActivator, IpcName, MockActivator};
use serde_json::{Value, json};

/// The child's environment: the pipe or socket, and the face to activate.
const CHILD_NAME: &str = "FM_IPC_CHILD_NAME";
const CHILD_FACE: &str = "FM_IPC_CHILD_FACE";
/// The line the child prints its reply after.
const REPLY_MARK: &str = "FM_IPC_REPLY:";

/// A scratch folder, removed at the end.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn core_api(base: &Path) -> CoreApi {
    let paths = AppPaths::under(base);
    fm_core::startup::prepare(&paths).expect("app folders");
    let index = Index::open(&paths.data.join(fm_core::index::INDEX_FILE))
        .expect("index")
        .index;
    CoreApi::new(
        index,
        Arc::new(MockActivator::new()) as Arc<dyn FontActivator>,
        SettingsStore::new(&paths),
        None,
    )
}

/// One fixture family, indexed where it is (read only).
fn fixture_family() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts/google-static/asap")
}

fn ipc_name(scratch: &Path) -> IpcName {
    if cfg!(windows) {
        IpcName::Pipe(format!(r"\\.\pipe\fm-ipc-activate-{}", std::process::id()))
    } else {
        IpcName::Socket(scratch.join("run").join("fm.sock"))
    }
}

#[test]
fn a_second_process_activates_a_fixture_face_over_ipc() {
    // Short on purpose: a socket path must fit in 104 bytes on macOS.
    let scratch = Scratch(std::env::temp_dir().join(format!("fm-ipa-{}", std::process::id())));
    let api = Arc::new(core_api(&scratch.0));
    let events = api.subscribe();
    let source = api
        .add_source(AddSourceRequest {
            path: fixture_family().display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    let job = api
        .scan(fm_api::library::ScanRequest {
            source: Some(source.id),
        })
        .expect("scan");
    loop {
        match events
            .recv_timeout(Duration::from_secs(60))
            .expect("scan events")
        {
            Event::ScanFinished { job: j, added, .. } if j == job => {
                assert!(added > 0, "the fixture family added no face");
                break;
            }
            Event::JobFailed { job: j, error } if j == job => panic!("scan failed: {error:?}"),
            _ => {}
        }
    }
    let family = api
        .list_families(ListFamiliesRequest::default())
        .expect("families")
        .items
        .into_iter()
        .next()
        .expect("a family");
    let face = api.get_family(family.id).expect("family").faces[0].id;

    let name = ipc_name(&scratch.0);
    let endpoint = IpcEndpoint::start(Arc::clone(&api) as Arc<dyn Api>, &name).expect("start");

    // The client: this test binary again, running only `child_client`.
    let name_text = match &name {
        IpcName::Pipe(pipe) => pipe.clone(),
        IpcName::Socket(path) => path.display().to_string(),
    };
    let output = Command::new(std::env::current_exe().expect("test binary"))
        .args(["child_client", "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_NAME, &name_text)
        .env(CHILD_FACE, serde_json::to_string(&face).expect("face id"))
        .output()
        .expect("run the client process");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "client failed: {stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reply_line = stdout
        .lines()
        .find_map(|l| l.split_once(REPLY_MARK).map(|(_, reply)| reply))
        .unwrap_or_else(|| panic!("no reply in the client's output: {stdout}"));
    let reply: Value = serde_json::from_str(reply_line).expect("reply JSON");

    assert_eq!(reply["v"], 1);
    assert_eq!(reply["id"], "activate-1");
    assert_eq!(
        reply["result"],
        json!([{"face": face, "outcome": {"status": "done"}}]),
        "{reply}"
    );
    // The app's own state agrees: the face is active.
    let active = api.list_active().expect("active");
    assert!(active.iter().any(|a| a.face == face), "{active:?}");

    endpoint.stop();
}

/// Runs only as the child of the test above (with its environment set);
/// alone it does nothing.
#[test]
fn child_client() {
    let (Some(name), Some(face)) = (std::env::var_os(CHILD_NAME), std::env::var(CHILD_FACE).ok())
    else {
        return;
    };
    let name = if cfg!(windows) {
        IpcName::Pipe(name.to_string_lossy().into_owned())
    } else {
        IpcName::Socket(PathBuf::from(name))
    };
    let face: Value = serde_json::from_str(&face).expect("face id");
    let mut stream = BufReader::new(fm_platform::ipc::connect(&name).expect("connect"));
    let request = json!({
        "v": 1,
        "id": "activate-1",
        "method": "activate",
        "params": {"target": {"kind": "faces", "faces": [face]}, "mode": "session"},
    });
    let mut line = serde_json::to_vec(&request).expect("json");
    line.push(b'\n');
    stream.get_mut().write_all(&line).expect("send");
    stream.get_mut().flush().expect("flush");
    let mut reply = String::new();
    stream.read_line(&mut reply).expect("reply");
    println!("{REPLY_MARK}{}", reply.trim_end());
}
