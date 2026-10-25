//! The local API endpoint's token and discovery file (ADR 0015).
//!
//! The token is 32 bytes from the OS random source, kept as 64 hex digits in
//! `<data>/endpoint-token` so it survives a restart. While the app serves the
//! endpoint, `<data>/endpoint.json` tells other programs (the CLI, plugins)
//! where it is: url, token, process id and API version. Both files are
//! private to the user ([`fm_platform::fs::write_private_file`]); the
//! discovery file is removed when the app quits.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use fm_api::settings::EndpointInfo;
use serde::{Deserialize, Serialize};

use crate::error::FmError;

/// The token file in the data folder.
pub const TOKEN_FILE: &str = "endpoint-token";
/// The discovery file in the data folder.
pub const DISCOVERY_FILE: &str = "endpoint.json";
/// Random bytes in a token.
const TOKEN_BYTES: usize = 32;

/// What `endpoint.json` holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    /// `http://127.0.0.1:<port>`.
    pub url: String,
    /// The bearer token.
    pub token: String,
    /// The process that serves the endpoint.
    pub pid: u32,
    /// The `fm-api` contract version it serves, as `major.minor.patch`.
    pub api_version: String,
}

/// The token and, while the endpoint runs, its address.
#[derive(Debug)]
pub struct EndpointKeys {
    dir: PathBuf,
    state: Mutex<KeyState>,
}

#[derive(Debug, Default)]
struct KeyState {
    /// Read from the token file on first use, so startup reads nothing.
    token: Option<String>,
    url: Option<String>,
}

impl EndpointKeys {
    /// The keys kept in the data folder `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            state: Mutex::new(KeyState::default()),
        }
    }

    /// The address (while published) and the token; the first call reads the
    /// token file, or makes a token when there is none.
    pub fn info(&self) -> Result<EndpointInfo, FmError> {
        let mut st = self.lock();
        let token = self.token(&mut st)?;
        Ok(EndpointInfo {
            url: st.url.clone(),
            token,
        })
    }

    /// A new token, saved at once: requests with the old one fail from now
    /// on. A published discovery file gets the new token too.
    pub fn regenerate(&self) -> Result<EndpointInfo, FmError> {
        let mut st = self.lock();
        let token = new_token()?;
        self.save_token(&token)?;
        st.token = Some(token.clone());
        if let Some(url) = &st.url {
            self.write_discovery(url, &token)?;
        }
        Ok(EndpointInfo {
            url: st.url.clone(),
            token,
        })
    }

    /// Records that the endpoint listens on `addr` and writes the discovery file.
    pub fn publish(&self, addr: SocketAddr) -> Result<EndpointInfo, FmError> {
        let mut st = self.lock();
        let token = self.token(&mut st)?;
        let url = format!("http://{addr}");
        self.write_discovery(&url, &token)?;
        st.url = Some(url.clone());
        Ok(EndpointInfo {
            url: Some(url),
            token,
        })
    }

    /// The endpoint stopped: removes the discovery file when this process
    /// wrote it (another instance's file is left alone).
    pub fn unpublish(&self) -> Result<(), FmError> {
        let mut st = self.lock();
        st.url = None;
        let path = self.discovery_path();
        let ours = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Discovery>(&bytes).ok())
            .is_some_and(|d| d.pid == std::process::id());
        if ours {
            std::fs::remove_file(&path)
                .map_err(|e| FmError::io("The local API file could not be removed.", &path, &e))?;
        }
        Ok(())
    }

    /// `<data>/endpoint.json`.
    pub fn discovery_path(&self) -> PathBuf {
        self.dir.join(DISCOVERY_FILE)
    }

    /// `<data>/endpoint-token`.
    pub fn token_path(&self) -> PathBuf {
        self.dir.join(TOKEN_FILE)
    }

    fn lock(&self) -> MutexGuard<'_, KeyState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn token(&self, st: &mut KeyState) -> Result<String, FmError> {
        if let Some(token) = &st.token {
            return Ok(token.clone());
        }
        let path = self.token_path();
        let token = match read_token(&path) {
            Some(token) => {
                // A token file another program made readable is made private again.
                if !fm_platform::fs::is_private_file(&path).unwrap_or(false) {
                    self.save_token(&token)?;
                }
                token
            }
            None => {
                let token = new_token()?;
                self.save_token(&token)?;
                token
            }
        };
        st.token = Some(token.clone());
        Ok(token)
    }

    fn save_token(&self, token: &str) -> Result<(), FmError> {
        let path = self.token_path();
        write_private(&self.dir, &path, token.as_bytes())
            .map_err(|e| FmError::io("The local API token could not be saved.", &path, &e))
    }

    fn write_discovery(&self, url: &str, token: &str) -> Result<(), FmError> {
        let discovery = Discovery {
            url: url.to_owned(),
            token: token.to_owned(),
            pid: std::process::id(),
            api_version: fm_api::API_VERSION.to_string(),
        };
        let json = serde_json::to_vec_pretty(&discovery)
            .map_err(|e| FmError::internal(format!("endpoint.json: {e}")))?;
        let path = self.discovery_path();
        write_private(&self.dir, &path, &json)
            .map_err(|e| FmError::io("The local API file could not be written.", &path, &e))
    }
}

fn write_private(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    fm_platform::fs::write_private_file(path, bytes)
}

/// The token in `path` when it holds one (64 lowercase hex digits).
fn read_token(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let token = text.trim();
    is_token(token).then(|| token.to_owned())
}

fn is_token(s: &str) -> bool {
    s.len() == TOKEN_BYTES * 2 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// 32 bytes from the OS random source, as 64 lowercase hex digits.
fn new_token() -> Result<String, FmError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|e| FmError::internal(format!("OS random source: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fm-endpoint-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn addr() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 41234))
    }

    #[test]
    fn a_token_is_64_hex_digits_saved_privately_and_kept_across_restarts() {
        let dir = scratch("keep");
        let first = EndpointKeys::new(&dir).info().expect("info");
        assert!(is_token(&first.token), "{}", first.token);
        assert_eq!(first.url, None, "not published yet");
        let keys = EndpointKeys::new(&dir);
        assert!(fm_platform::fs::is_private_file(&keys.token_path()).expect("check"));
        // A restart: a new value over the same folder reads the same token.
        assert_eq!(keys.info().expect("info").token, first.token);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_tokens_differ() {
        let a = new_token().expect("token");
        let b = new_token().expect("token");
        assert_ne!(a, b);
    }

    #[test]
    fn a_broken_token_file_is_replaced_by_a_new_token() {
        let dir = scratch("broken");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join(TOKEN_FILE), "not a token").expect("write");
        let info = EndpointKeys::new(&dir).info().expect("info");
        assert!(is_token(&info.token));
        assert_eq!(read_token(&dir.join(TOKEN_FILE)), Some(info.token));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn publish_writes_a_private_discovery_file_and_unpublish_removes_it() {
        let dir = scratch("publish");
        let keys = EndpointKeys::new(&dir);
        let info = keys.publish(addr()).expect("publish");
        assert_eq!(info.url.as_deref(), Some("http://127.0.0.1:41234"));
        let path = keys.discovery_path();
        assert!(fm_platform::fs::is_private_file(&path).expect("check"));
        let d: Discovery =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(
            d,
            Discovery {
                url: "http://127.0.0.1:41234".into(),
                token: info.token.clone(),
                pid: std::process::id(),
                api_version: fm_api::API_VERSION.to_string(),
            }
        );
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert!(json.get("apiVersion").is_some(), "camelCase keys: {json}");
        keys.unpublish().expect("unpublish");
        assert!(!path.exists(), "endpoint.json is left after quit");
        assert_eq!(keys.info().expect("info").url, None);
        // The token outlives the endpoint.
        assert_eq!(keys.info().expect("info").token, info.token);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn regenerate_replaces_the_token_everywhere() {
        let dir = scratch("regenerate");
        let keys = EndpointKeys::new(&dir);
        let old = keys.publish(addr()).expect("publish");
        let new = keys.regenerate().expect("regenerate");
        assert_ne!(new.token, old.token);
        assert_eq!(new.url, old.url);
        assert_eq!(keys.info().expect("info").token, new.token);
        assert_eq!(read_token(&keys.token_path()), Some(new.token.clone()));
        let d: Discovery =
            serde_json::from_slice(&std::fs::read(keys.discovery_path()).expect("read"))
                .expect("json");
        assert_eq!(d.token, new.token);
        assert_eq!(
            EndpointKeys::new(&dir).info().expect("restart").token,
            new.token
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn another_instances_discovery_file_is_left_alone() {
        let dir = scratch("other");
        std::fs::create_dir_all(&dir).expect("dir");
        let other = Discovery {
            url: "http://127.0.0.1:1".into(),
            token: "0".repeat(64),
            pid: std::process::id().wrapping_add(1),
            api_version: "0.0.0".into(),
        };
        let path = dir.join(DISCOVERY_FILE);
        std::fs::write(&path, serde_json::to_vec(&other).expect("json")).expect("write");
        EndpointKeys::new(&dir).unpublish().expect("unpublish");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
