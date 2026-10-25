//! An rmcp client connected in process to an [`McpServer`], and the check
//! that a result carries no font bytes.

#![allow(dead_code, clippy::expect_used, clippy::panic)]

use fm_api::ApiError;
use fm_mcp::McpServer;
use rmcp::ServiceExt as _;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, GetPromptRequestParams, GetPromptResult, Prompt,
    ReadResourceRequestParams, ReadResourceResult, ResourceTemplate, Tool,
};
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

    pub async fn list_all_resource_templates(&self) -> Result<Vec<ResourceTemplate>, ServiceError> {
        self.0.list_all_resource_templates().await
    }

    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, ServiceError> {
        self.0
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
    }

    pub async fn list_all_prompts(&self) -> Result<Vec<Prompt>, ServiceError> {
        self.0.list_all_prompts().await
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        args: Value,
    ) -> Result<GetPromptResult, ServiceError> {
        let Value::Object(args) = args else {
            panic!("arguments are an object");
        };
        self.0
            .get_prompt(GetPromptRequestParams::new(name).with_arguments(args))
            .await
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

    /// The PNG bytes and the caption of a preview call that must succeed:
    /// image content first (`image/png`, base64), then one text block.
    pub async fn png(&self, tool: &str, args: Value) -> (Vec<u8>, String) {
        let result = self.call(tool, args).await.expect("tools/call");
        assert_ne!(result.is_error, Some(true), "{tool}: {result:?}");
        assert_eq!(result.content.len(), 2, "{tool}: image and text");
        let image = result.content[0].as_image().expect("image content");
        assert_eq!(image.mime_type, "image/png");
        let png = unbase64(&image.data);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
        let text = result.content[1].as_text().expect("text").text.clone();
        (png, text)
    }

    pub async fn err(&self, tool: &str, args: Value) -> ApiError {
        self.err_text(tool, args).await.1
    }

    pub async fn close(self) {
        let _ = self.0.cancel().await;
    }
}

/// Decodes standard padded base64; panics on anything else.
pub fn unbase64(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    assert_eq!(s.len() % 4, 0, "padded base64");
    let mut out = Vec::new();
    for quad in s.as_bytes().chunks(4) {
        let pad = quad.iter().filter(|&&c| c == b'=').count();
        let mut n = 0u32;
        for &c in quad {
            let v = if c == b'=' {
                0
            } else {
                ALPHABET.iter().position(|&a| a == c).expect("base64 digit") as u32
            };
            n = (n << 6) | v;
        }
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    out
}

/// A PNG's width and height, from its IHDR chunk.
pub fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[12..16], b"IHDR");
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    (be(&png[16..20]), be(&png[20..24]))
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
