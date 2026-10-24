//! Helpers shared by the unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A fresh folder under the system temp folder, removed on drop.
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("fm-core-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An HTTP/1.1 server on 127.0.0.1 that answers fixed routes, for tests that
/// must not reach the internet. Every connection gets one answer and is
/// closed; the server thread lives as long as the test process.
pub(crate) struct LocalServer {
    base: String,
    hits: std::sync::Arc<AtomicU32>,
}

impl LocalServer {
    /// Serves `routes` (path with query → status and body); any other path is a 404.
    pub(crate) fn start(routes: Vec<(&str, u16, Vec<u8>)>) -> Self {
        let routes: Vec<(String, u16, Vec<u8>)> = routes
            .into_iter()
            .map(|(path, status, body)| (path.to_owned(), status, body))
            .collect();
        Self::start_with(|_| routes)
    }

    /// Serves the routes `routes` makes from the server's base URL, for an
    /// answer that links to the server itself (a file list's URLs).
    pub(crate) fn start_with(routes: impl FnOnce(&str) -> Vec<(String, u16, Vec<u8>)>) -> Self {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let routes: std::collections::HashMap<String, (u16, Vec<u8>)> = routes(&base)
            .into_iter()
            .map(|(path, status, body)| (path, (status, body)))
            .collect();
        let hits = std::sync::Arc::new(AtomicU32::new(0));
        let counter = std::sync::Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                counter.fetch_add(1, Ordering::Relaxed);
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
                let mut request = String::new();
                let _ = reader.read_line(&mut request);
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) if line == "\r\n" || line == "\n" => break,
                        Ok(_) => {}
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap_or("");
                let (status, body) = routes
                    .get(path)
                    .cloned()
                    .unwrap_or((404, b"not found".to_vec()));
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = stream.flush();
            }
        });
        Self { base, hits }
    }

    /// `http://127.0.0.1:<port>`.
    pub(crate) fn base(&self) -> &str {
        &self.base
    }

    /// Requests answered so far.
    pub(crate) fn hits(&self) -> u32 {
        self.hits.load(Ordering::Relaxed)
    }
}
