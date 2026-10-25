//! A `MockApi` behind a real endpoint, and a fake server that answers with
//! whatever bytes a test gives it.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use fm_api::mock::MockApi;
use fm_api::{Api, SettingsService};
use fm_client::Client;
use fm_endpoint::Endpoint;

/// A served `MockApi` and a client of it.
pub struct Served {
    pub api: Arc<MockApi>,
    pub endpoint: Endpoint,
    pub client: Client,
}

impl Served {
    pub fn new() -> Self {
        let api = Arc::new(MockApi::new());
        let endpoint = Endpoint::start(Arc::clone(&api) as Arc<dyn Api>).expect("start");
        let token = api.endpoint_info().expect("info").token;
        let client = Client::new(endpoint.addr(), token);
        Self {
            api,
            endpoint,
            client,
        }
    }

    /// Waits up to 10 s for the endpoint to have `n` event streams,
    /// running `between` while it waits.
    pub fn wait_for_streams(&self, n: usize, mut between: impl FnMut()) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.endpoint.event_streams() != n {
            assert!(
                Instant::now() < deadline,
                "expected {n} event stream(s), have {}",
                self.endpoint.event_streams()
            );
            between();
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// A one-connection server: it reads the request head, sends it to the
/// returned receiver, writes `answer` and closes.
pub fn fake_server(answer: Vec<u8>) -> (SocketAddr, Receiver<String>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut head = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read") == 0 || line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let _ = tx.send(head);
        let mut stream = stream;
        let _ = stream.write_all(&answer);
        let _ = stream.flush();
    });
    (addr, rx, thread)
}
