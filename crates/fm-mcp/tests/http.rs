//! The MCP server on the local API endpoint (E27.1): an rmcp client over
//! streamable HTTP reaches `/mcp` on an ephemeral 127.0.0.1 port over
//! `MockApi`, only with the endpoint's token and only while the user has the
//! server on; the switch applies without restarting the endpoint.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;

use fm_api::mock::MockApi;
use fm_api::settings::McpUpdate;
use fm_api::{API_VERSION, Api, SettingsService as _};
use fm_endpoint::Endpoint;
use fm_endpoint::mount::{Mount, MountBody, MountFuture, MountRequest};
use fm_mcp::HttpServer;
use rmcp::ServiceExt as _;
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;

/// The app's glue (`McpMount` in `fm_app_lib`): the endpoint's `/mcp` mount
/// over the rmcp service.
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

struct Served {
    api: Arc<MockApi>,
    mcp: Arc<HttpServer>,
    endpoint: Endpoint,
}

impl Served {
    fn new() -> Self {
        let api = Arc::new(MockApi::new());
        let dyn_api: Arc<dyn Api> = api.clone();
        let mcp = Arc::new(HttpServer::new(Arc::clone(&dyn_api)));
        let mount = Arc::new(McpMount(Arc::clone(&mcp)));
        let endpoint = Endpoint::start_with_mcp(dyn_api, mount).expect("endpoint");
        Self { api, mcp, endpoint }
    }

    fn addr(&self) -> SocketAddr {
        self.endpoint.addr()
    }

    fn url(&self) -> String {
        format!("http://{}/mcp", self.addr())
    }

    fn token(&self) -> String {
        self.api.endpoint_info().expect("info").token
    }

    fn set(&self, enabled: bool) {
        self.api
            .set_mcp_enabled(McpUpdate { enabled })
            .expect("switch");
    }

    async fn client(&self, token: &str) -> Result<RunningService<RoleClient, ()>, String> {
        let transport = StreamableHttpClientTransport::from_config(
            StreamableHttpClientTransportConfig::with_uri(self.url()).auth_header(token),
        );
        ().serve(transport).await.map_err(|e| e.to_string())
    }
}

/// `POST /mcp` with an `initialize` body; the status code. `token` `None`
/// sends no `Authorization` header.
fn post_initialize(addr: SocketAddr, token: Option<&str>) -> u16 {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: {addr}\r\n{auth}Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{INITIALIZE}",
        INITIALIZE.len()
    )
    .expect("write");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("read");
    answer
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_rmcp_client_with_the_token_initializes_and_lists_tools() {
    let served = Served::new();
    served.set(true);
    assert_eq!(served.addr().ip().to_string(), "127.0.0.1");

    let client = served.client(&served.token()).await.expect("initialize");
    let info = client.peer_info().expect("server info");
    let server = info.server_info.as_ref().expect("implementation");
    assert_eq!(server.name, "font-manager");
    assert_eq!(server.version, API_VERSION.to_string());
    assert!(info.capabilities.tools.is_some());

    let tools = client.list_all_tools().await.expect("tools/list");
    assert!(tools.is_empty(), "no tools until E27.2: {tools:?}");
    client.cancel().await.expect("close");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_without_the_token_gets_401() {
    let served = Served::new();
    served.set(true);
    let addr = served.addr();
    let token = served.token();
    let (none, wrong, right) = tokio::task::spawn_blocking(move || {
        (
            post_initialize(addr, None),
            post_initialize(addr, Some(&"0".repeat(64))),
            post_initialize(addr, Some(&token)),
        )
    })
    .await
    .expect("requests");
    assert_eq!(none, 401);
    assert_eq!(wrong, 401);
    assert_eq!(right, 200);

    let refused = served.client(&"0".repeat(64)).await;
    assert!(
        refused.is_err(),
        "an rmcp client with a wrong token is refused"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_switch_turns_mcp_on_and_off_without_a_restart() {
    let served = Served::new();
    let addr = served.addr();
    let token = served.token();
    let status = {
        let token = token.clone();
        move || {
            let token = token.clone();
            async move {
                tokio::task::spawn_blocking(move || post_initialize(addr, Some(&token)))
                    .await
                    .expect("request")
            }
        }
    };

    // Off by default: not served, and nothing was built.
    assert!(!served.api.mcp_status().expect("status").enabled);
    assert_eq!(status().await, 404);
    assert!(served.client(&token).await.is_err());
    assert!(!served.mcp.is_built(), "an off server builds nothing");

    served.set(true);
    assert_eq!(status().await, 200);
    let client = served.client(&token).await.expect("on");
    client.cancel().await.expect("close");
    assert!(served.mcp.is_built());

    served.set(false);
    assert_eq!(status().await, 404);
    assert!(served.client(&token).await.is_err());

    served.set(true);
    let client = served.client(&token).await.expect("on again");
    assert!(client.list_all_tools().await.expect("tools").is_empty());
    client.cancel().await.expect("close");
}
