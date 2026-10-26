//! The stdio shim (E27.7): `fm-mcp-stdio` forwards JSON-RPC between its
//! stdin/stdout and the running app's `/mcp` endpoint, for MCP clients that
//! only start a process (Claude Desktop).
//!
//! It finds the app the way the CLI does ([`fm_client::connect`] over
//! `<data>/endpoint.json`, ADR 0015) and asks it for the token and whether
//! the user turned the MCP server on. Then it pumps messages, unchanged,
//! between rmcp's stdio transport and its streamable HTTP client transport:
//! no tool or method of its own, so it never falls behind the server.
//!
//! stdout carries JSON-RPC only; everything the shim says goes to stderr.
//! When the app is not there, the MCP server is off, or the app goes away
//! mid-session, the client gets a JSON-RPC error with [`UNAVAILABLE`] and
//! the shim exits non-zero.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use fm_api::SettingsService as _;
use fm_api::settings::MCP_PATH;
use fm_platform::AppPaths;
use rmcp::model::{
    ClientJsonRpcMessage, ErrorData, JsonRpcMessage, RequestId, ServerJsonRpcMessage,
};
use rmcp::service::RoleServer;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{IntoTransport as _, StreamableHttpClientTransport, Transport};
use tokio::task::JoinSet;

/// What the client is told when it cannot reach the app.
pub const UNAVAILABLE: &str = "Open Font Manager and turn on AI assistants in Settings.";

/// How long the shim waits for the client's first message when it has only
/// an error to answer with: long enough for a client that starts the shim
/// and then writes `initialize`, short enough that nobody waits on a shim
/// that cannot work.
pub const FIRST_MESSAGE_WAIT: Duration = Duration::from_secs(5);

/// The app's MCP server: where it is and the token it wants.
pub struct Target {
    /// `http://127.0.0.1:<port>/mcp`.
    pub url: String,
    /// The local API's bearer token.
    pub token: String,
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the token.
        f.debug_struct("Target").field("url", &self.url).finish()
    }
}

/// The MCP server of the app whose `endpoint.json` is in `paths.data`, or
/// why there is none to talk to (for stderr). Blocking: a few calls to the
/// local API.
pub fn find(paths: &AppPaths) -> Result<Target, String> {
    let client = fm_client::connect(paths).ok_or("Font Manager is not running.")?;
    let status = client
        .mcp_status()
        .map_err(|e| format!("Font Manager did not report its MCP server: {e}"))?;
    if !status.enabled {
        return Err("AI assistants are turned off in Settings.".to_owned());
    }
    let token = client
        .endpoint_info()
        .map_err(|e| format!("Font Manager did not report its local API: {e}"))?
        .token;
    Ok(Target {
        // The address the app answered on: the one it publishes.
        url: format!("http://{}{MCP_PATH}", client.addr()),
        token,
    })
}

/// Why [`forward`] stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The client closed stdin (or stdout): a normal end.
    ClientClosed,
    /// The app went away; every open request got the [`UNAVAILABLE`] error.
    AppGone,
}

/// The shim as a program: `fm-mcp-stdio`, and the app's executable started
/// with [`fm_platform::mcp_shim::SHIM_ARG`] (the AppImage command). The app
/// folders are the user's, or those under `FM_HOME` (a scratch home, as for
/// `fm`).
pub fn main() -> ExitCode {
    let target = app_paths().and_then(|paths| find(&paths));
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            log(&format!("no async runtime: {e}"));
            return ExitCode::FAILURE;
        }
    };
    let code = runtime.block_on(run(target));
    // The stdin reader sits on a blocking thread that only a new line or EOF
    // wakes: do not wait for it.
    runtime.shutdown_background();
    code
}

/// `FM_HOME` when it is set to an absolute path, else the user's app folders.
fn app_paths() -> Result<AppPaths, String> {
    if let Some(home) = std::env::var_os("FM_HOME").map(PathBuf::from)
        && home.is_absolute()
    {
        return Ok(AppPaths::under(&home));
    }
    AppPaths::current().map_err(|e| format!("The app folders could not be found: {e}"))
}

/// The shim's whole run on the process's stdin/stdout: `target` is what
/// [`find`] returned. Success when the client closed stdin, failure when
/// there was no app to forward to or it went away.
pub async fn run(target: Result<Target, String>) -> ExitCode {
    let stdio = rmcp::transport::stdio().into_transport();
    let end = match target {
        Ok(target) => forward(&target, stdio).await,
        Err(why) => {
            log(&why);
            refuse(stdio).await;
            End::AppGone
        }
    };
    match end {
        End::ClientClosed => ExitCode::SUCCESS,
        End::AppGone => ExitCode::FAILURE,
    }
}

/// One line on stderr: stdout is the client's.
fn log(message: &str) {
    eprintln!("fm-mcp-stdio: {message}");
}

fn unavailable() -> ErrorData {
    ErrorData::internal_error(UNAVAILABLE, None)
}

/// Answers the client's first message (waiting at most
/// [`FIRST_MESSAGE_WAIT`]) with the [`UNAVAILABLE`] error: with its id when
/// it is a request, without one when it is not or nothing came.
async fn refuse<T: Transport<RoleServer>>(mut client: T) {
    let first = tokio::time::timeout(FIRST_MESSAGE_WAIT, client.receive()).await;
    let id = match first {
        Ok(Some(JsonRpcMessage::Request(request))) => Some(request.id),
        _ => None,
    };
    if let Err(e) = client
        .send(ServerJsonRpcMessage::error(unavailable(), id))
        .await
    {
        log(&format!("the error could not be written: {e}"));
    }
    let _ = client.close().await;
}

/// Pumps messages between `client` (stdio) and the app until the client
/// closes stdin or the app goes away.
pub async fn forward<T: Transport<RoleServer>>(target: &Target, mut client: T) -> End {
    let mut app = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(target.url.clone())
            .auth_header(target.token.clone()),
    );
    // The requests the app has not answered yet: they get an error when it
    // goes away.
    let mut open: HashSet<RequestId> = HashSet::new();
    let mut sends: JoinSet<(Option<RequestId>, Result<(), String>)> = JoinSet::new();
    let end = loop {
        tokio::select! {
            message = client.receive() => match message {
                Some(message) => {
                    let id = request_id(&message);
                    if let Some(id) = &id {
                        open.insert(id.clone());
                    }
                    // Sent concurrently, as rmcp's own client does: a slow
                    // tool call does not hold up the next request.
                    let send = app.send(message);
                    sends.spawn(async move { (id, send.await.map_err(|e| e.to_string())) });
                }
                None => break End::ClientClosed,
            },
            message = app.receive() => match message {
                Some(message) => {
                    if let Some(id) = answered_id(&message) {
                        open.remove(id);
                    }
                    if let Err(e) = client.send(message).await {
                        log(&format!("stdout is closed: {e}"));
                        break End::ClientClosed;
                    }
                }
                None => {
                    log("Font Manager closed the connection.");
                    break End::AppGone;
                }
            },
            Some(sent) = sends.join_next(), if !sends.is_empty() => {
                if let Ok((id, Err(e))) = sent {
                    log(&format!("a message did not reach Font Manager: {e}"));
                    if let Some(id) = id.filter(|id| open.remove(id)) {
                        let error = ServerJsonRpcMessage::error(unavailable(), Some(id));
                        let _ = client.send(error).await;
                    }
                }
            }
        }
    };
    if end == End::AppGone {
        for id in open.drain() {
            let error = ServerJsonRpcMessage::error(unavailable(), Some(id));
            let _ = client.send(error).await;
        }
    }
    sends.abort_all();
    let _ = app.close().await;
    let _ = client.close().await;
    end
}

fn request_id(message: &ClientJsonRpcMessage) -> Option<RequestId> {
    match message {
        JsonRpcMessage::Request(request) => Some(request.id.clone()),
        _ => None,
    }
}

fn answered_id(message: &ServerJsonRpcMessage) -> Option<&RequestId> {
    match message {
        JsonRpcMessage::Response(response) => Some(&response.id),
        JsonRpcMessage::Error(error) => error.id.as_ref(),
        _ => None,
    }
}
