//! The IPC endpoint over `MockApi` (E29.1 criteria 3 and 4): the envelope,
//! a method of every service, `hello`, and every refusal on a connection
//! that stays usable.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fm_api::mock::MockApi;
use fm_api::{API_VERSION, Api, LibraryService as _, SettingsService as _};
use fm_endpoint::ipc::{IpcEndpoint, MAX_FRAME_BYTES};
use fm_platform::IpcName;
use serde_json::{Value, json};

/// A fresh pipe name, or a socket in a scratch folder removed at the end.
struct Name {
    name: IpcName,
    #[cfg(unix)]
    dir: std::path::PathBuf,
}

impl Name {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let tag = format!(
            "fm-ipc-ep-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        #[cfg(windows)]
        {
            Self {
                name: IpcName::Pipe(format!(r"\\.\pipe\{tag}")),
            }
        }
        #[cfg(unix)]
        {
            let dir = std::env::temp_dir().join(tag);
            Self {
                name: IpcName::Socket(dir.join("fm.sock")),
                dir,
            }
        }
    }
}

#[cfg(unix)]
impl Drop for Name {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Served {
    api: Arc<MockApi>,
    endpoint: IpcEndpoint,
    name: Name,
}

impl Served {
    fn new() -> Self {
        let name = Name::new();
        let api = Arc::new(MockApi::new());
        let endpoint =
            IpcEndpoint::start(Arc::clone(&api) as Arc<dyn Api>, &name.name).expect("start");
        Self {
            api,
            endpoint,
            name,
        }
    }

    fn client(&self) -> Client {
        Client::connect(&self.name.name)
    }
}

struct Client(BufReader<fm_platform::IpcStream>);

impl Client {
    fn connect(name: &IpcName) -> Self {
        Self(BufReader::new(
            fm_platform::ipc::connect(name).expect("connect"),
        ))
    }

    fn send_raw(&mut self, bytes: &[u8]) {
        let stream = self.0.get_mut();
        stream.write_all(bytes).expect("send");
        stream.flush().expect("flush");
    }

    fn read_reply(&mut self) -> Value {
        let mut line = String::new();
        let n = self.0.read_line(&mut line).expect("reply");
        assert!(n > 0, "the server closed the connection");
        assert!(line.ends_with('\n'), "a reply is one line: {line:?}");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("reply is JSON ({e}): {line}"))
    }

    fn call(&mut self, request: &Value) -> Value {
        let mut line = serde_json::to_vec(request).expect("json");
        line.push(b'\n');
        self.send_raw(&line);
        self.read_reply()
    }

    fn hello_works(&mut self) {
        let reply = self.call(&json!({"v": 1, "id": "ping", "method": "hello"}));
        assert_eq!(reply["id"], "ping");
        assert_eq!(reply["result"], serde_json::to_value(API_VERSION).unwrap());
    }
}

fn error_code(reply: &Value) -> &str {
    assert!(reply.get("result").is_none(), "{reply}");
    assert!(reply["error"]["message"].is_string(), "{reply}");
    reply["error"]["code"].as_str().expect("an error code")
}

#[test]
fn hello_answers_the_api_version_in_the_envelope() {
    let served = Served::new();
    let mut client = served.client();
    let reply = client.call(&json!({"v": 1, "id": 7, "method": "hello"}));
    assert_eq!(
        reply,
        json!({"v": 1, "id": 7, "result": serde_json::to_value(API_VERSION).unwrap()})
    );
}

#[test]
fn a_method_of_every_service_answers_over_the_pipe() {
    let served = Served::new();
    let mut client = served.client();
    // One method per service trait in fm-api (eleven of them).
    let calls = [
        ("library", "app_info", None),
        ("search", "list_languages", None),
        (
            "preview",
            "sample_texts",
            Some(json!({"faces": [1], "text": "Ag"})),
        ),
        ("activation", "list_active", None),
        ("organization", "list_collections", Some(Value::Null)),
        (
            "export",
            "framework_snippet",
            Some(json!({"faces": [1], "framework": "tailwind"})),
        ),
        (
            "workflows",
            "pick_fonts_for_brief",
            Some(json!({"brief": "a friendly sans"})),
        ),
        ("settings", "get_settings", Some(json!({}))),
        ("layout", "layout_presets", None),
        ("comparison", "list_comparisons", None),
        ("catalog", "list_catalogs", None),
    ];
    for (i, (service, method, params)) in calls.into_iter().enumerate() {
        let mut request = json!({"v": 1, "id": i, "method": method});
        if let Some(params) = params {
            request["params"] = params;
        }
        let reply = client.call(&request);
        assert_eq!(reply["v"], 1, "{service}: {reply}");
        assert_eq!(reply["id"], i, "{service}: {reply}");
        assert!(
            reply.get("result").is_some() && reply.get("error").is_none(),
            "{service}.{method}: {reply}"
        );
    }
    // The value is the service's own, as JSON.
    let info = client.call(&json!({"v": 1, "id": "a", "method": "app_info"}));
    assert_eq!(
        info["result"],
        serde_json::to_value(served.api.app_info().unwrap()).unwrap()
    );
    let settings = client.call(&json!({"v": 1, "id": "s", "method": "get_settings"}));
    assert_eq!(
        settings["result"],
        serde_json::to_value(served.api.get_settings().unwrap()).unwrap()
    );
}

#[test]
fn a_service_error_keeps_its_shape() {
    let served = Served::new();
    let mut client = served.client();
    let reply = client.call(&json!({"v": 1, "id": 1, "method": "get_face", "params": 999_999}));
    assert_eq!(reply["id"], 1);
    assert_eq!(error_code(&reply), "notFound", "{reply}");
    client.hello_works();
}

#[test]
fn another_major_version_is_unsupported_and_the_connection_stays() {
    let served = Served::new();
    let mut client = served.client();
    let reply = client.call(&json!({"v": 2, "id": "x", "method": "app_info"}));
    assert_eq!(reply["v"], 1);
    assert_eq!(reply["id"], "x");
    assert_eq!(error_code(&reply), "unsupported");
    // `v` missing or not a whole number: not a request.
    for bad in [
        json!({"id": 1, "method": "hello"}),
        json!({"v": "1", "id": 1, "method": "hello"}),
    ] {
        let reply = client.call(&bad);
        assert_eq!(reply["id"], 1);
        assert_eq!(error_code(&reply), "invalidRequest", "{bad}");
    }
    client.hello_works();
}

#[test]
fn an_unknown_method_is_unsupported_and_the_connection_stays() {
    let served = Served::new();
    let mut client = served.client();
    let reply = client.call(&json!({"v": 1, "id": 3, "method": "format_disk"}));
    assert_eq!(reply["id"], 3);
    assert_eq!(error_code(&reply), "unsupported");
    // `subscribe` is the event source, not a method of the table.
    let reply = client.call(&json!({"v": 1, "id": 4, "method": "subscribe"}));
    assert_eq!(error_code(&reply), "unsupported");
    let reply = client.call(&json!({"v": 1, "id": 5, "method": 12}));
    assert_eq!(error_code(&reply), "invalidRequest");
    client.hello_works();
}

#[test]
fn malformed_json_is_an_invalid_request_and_the_connection_stays() {
    let served = Served::new();
    let mut client = served.client();
    for bad in [
        &b"{\"v\":1,\"id\":1,\"method\"\n"[..],
        b"not json\n",
        b"[1,2]\n",
        b"\xff\xfe\n",
    ] {
        client.send_raw(bad);
        let reply = client.read_reply();
        assert_eq!(reply["id"], Value::Null, "{reply}");
        assert_eq!(
            error_code(&reply),
            "invalidRequest",
            "{:?}",
            String::from_utf8_lossy(bad)
        );
    }
    // Params that do not fit the method: refused before the service runs.
    let reply = client.call(&json!({"v": 1, "id": 9, "method": "get_face", "params": "one"}));
    assert_eq!(reply["id"], 9);
    assert_eq!(error_code(&reply), "invalidRequest");
    // Blank lines are skipped, not answered.
    client.send_raw(b"\n\r\n");
    client.hello_works();
}

#[test]
fn a_frame_over_one_mib_is_refused_and_the_connection_stays() {
    let served = Served::new();
    let mut client = served.client();
    let padding = "x".repeat(MAX_FRAME_BYTES);
    let reply = client.call(&json!({"v": 1, "id": 1, "method": "hello", "params": padding}));
    assert_eq!(reply["id"], Value::Null);
    assert_eq!(error_code(&reply), "invalidRequest");
    client.hello_works();
}

#[test]
fn other_clients_are_served_while_one_is_mid_message() {
    let served = Served::new();
    let mut slow = served.client();
    slow.send_raw(br#"{"v":1,"id":"slow","#);
    // A second and a third client, while the first has half a line out.
    let mut others: Vec<Client> = (0..2).map(|_| served.client()).collect();
    for other in &mut others {
        other.hello_works();
    }
    slow.send_raw(b"\"method\":\"hello\"}\n");
    assert_eq!(slow.read_reply()["id"], "slow");
}

#[test]
fn a_client_that_goes_away_mid_message_harms_no_one() {
    let served = Served::new();
    {
        let mut gone = served.client();
        gone.send_raw(br#"{"v":1,"id":1,"me"#);
    }
    served.client().hello_works();
}

#[test]
fn stopping_closes_the_pipe_or_socket() {
    let served = Served::new();
    served.client().hello_works();
    let name = served.endpoint.name().clone();
    served.endpoint.stop();
    assert!(fm_platform::ipc::connect(&name).is_err(), "nothing listens");
    #[cfg(unix)]
    if let IpcName::Socket(path) = &name {
        assert!(
            std::fs::symlink_metadata(path).is_err(),
            "the socket file is removed on a clean stop"
        );
    }
    drop(served.name);
}

/// A client that stays connected is cut off by the stop: a request it
/// sends after it is never served (the quit path ends the session fonts
/// right after the stop), and the same name binds again at once (Windows
/// refuses a first pipe instance while an old one is open).
#[test]
fn stopping_ends_open_connections_and_the_name_binds_again() {
    use fm_api::ActivationService as _;

    let served = Served::new();
    let mut open = served.client();
    open.hello_works();
    let before = served.api.list_active().unwrap();
    let name = served.endpoint.name().clone();
    served.endpoint.stop();

    // The request may not even be written: the connection is gone.
    let request = json!({
        "v": 1, "id": "late", "method": "activate",
        "params": {"target": {"kind": "faces", "faces": [1]}, "mode": "session"},
    });
    let mut line = serde_json::to_vec(&request).unwrap();
    line.push(b'\n');
    let _ = open.0.get_mut().write_all(&line);
    let _ = open.0.get_mut().flush();
    let mut reply = String::new();
    let read = open.0.read_line(&mut reply);
    assert!(
        matches!(read, Ok(0) | Err(_)),
        "a request after the stop was answered: {reply}"
    );
    assert_eq!(
        served.api.list_active().unwrap(),
        before,
        "activated after the stop"
    );

    let again = IpcEndpoint::start(Arc::clone(&served.api) as Arc<dyn Api>, &name)
        .expect("the name binds again after the stop");
    Client::connect(&name).hello_works();
    again.stop();
    drop(served.name);
}
