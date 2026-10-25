//! An rmcp client connected in process to an [`McpServer`], and the check
//! that a result carries no font bytes.

#![allow(dead_code, clippy::expect_used, clippy::panic)]

use fm_api::ApiError;
use fm_mcp::McpServer;
use rmcp::ServiceExt as _;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceError};
use serde_json::Value;

/// An rmcp client talking to one server over an in-memory pipe.
pub struct Client(RunningService<RoleClient, ()>);

/// Starts `server` and connects a client to it.
pub async fn connect(server: McpServer) -> Client {
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        if let Ok(running) = server.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    Client(().serve(client_io).await.expect("initialize"))
}

impl Client {
    pub async fn list_all_tools(&self) -> Result<Vec<Tool>, ServiceError> {
        self.0.list_all_tools().await
    }

    pub async fn call(&self, tool: &str, args: Value) -> Result<CallToolResult, ServiceError> {
        let Value::Object(args) = args else {
            panic!("arguments are an object");
        };
        self.0
            .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args))
            .await
    }

    /// The structured result of a call that must succeed; its text content
    /// must be the same JSON.
    pub async fn ok(&self, tool: &str, args: Value) -> Value {
        let result = self.call(tool, args).await.expect("tools/call");
        assert_ne!(result.is_error, Some(true), "{tool}: {result:?}");
        let value = result
            .structured_content
            .clone()
            .expect("structured content");
        assert!(value.is_object(), "{tool}: results are objects");
        let text = result.content[0].as_text().expect("text").text.clone();
        assert_eq!(
            serde_json::from_str::<Value>(&text).expect("JSON text"),
            value
        );
        assert_no_font_bytes(&Value::String(text));
        value
    }

    /// The text and the API error of a call that must be a tool error.
    pub async fn err_text(&self, tool: &str, args: Value) -> (String, ApiError) {
        let result = self.call(tool, args).await.expect("tools/call");
        assert_eq!(result.is_error, Some(true), "{tool}: {result:?}");
        let text = result.content[0].as_text().expect("text").text.clone();
        let e = serde_json::from_value(result.structured_content.expect("error data"))
            .expect("an ApiError");
        (text, e)
    }

    pub async fn err(&self, tool: &str, args: Value) -> ApiError {
        self.err_text(tool, args).await.1
    }

    pub async fn close(self) {
        let _ = self.0.cancel().await;
    }
}

/// The first bytes of every font file format, raw and as base64.
const MAGICS: [&str; 12] = [
    // Raw.
    "wOF2",
    "wOFF",
    "OTTO",
    "ttcf",
    "\0\u{1}\0\0",
    "true",
    // Base64.
    "d09GMg",
    "d09GRg",
    "T1RUTw",
    "dHRjZg",
    "AAEAAA",
    "dHJ1ZQ",
];

/// Panics when `value` looks like it carries font bytes: a string that
/// starts like a font file (raw or base64), a `data:` URL, or a long array of
/// numbers.
pub fn assert_no_font_bytes(value: &Value) {
    match value {
        Value::String(s) => {
            for magic in MAGICS {
                assert!(!s.starts_with(magic), "font bytes ({magic}) in {s:.80}");
            }
            assert!(
                !s.contains("data:font") && !s.contains("data:application"),
                "a data URL: {s:.200}"
            );
            assert!(!s.contains(";base64,"), "base64 data: {s:.200}");
        }
        Value::Array(items) => {
            let numbers = items.iter().filter(|v| v.is_number()).count();
            assert!(
                numbers < 256,
                "{numbers} numbers in one array look like bytes"
            );
            items.iter().for_each(assert_no_font_bytes);
        }
        Value::Object(map) => map.values().for_each(assert_no_font_bytes),
        _ => {}
    }
}
