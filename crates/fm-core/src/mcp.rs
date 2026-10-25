//! The MCP server's status and the config snippets that connect a client to
//! it (E27.1). The server itself is `fm-mcp`, mounted on the local API
//! endpoint at [`MCP_PATH`]; this module only says where it is and how a
//! client reaches it, with the endpoint's token.

use fm_api::settings::{EndpointInfo, MCP_PATH, MCP_SERVER_NAME, McpClient, McpSnippet, McpStatus};

/// The status for the switch `enabled` and the endpoint `endpoint`: the URL
/// and the snippets while the endpoint runs, none while it does not.
pub fn status(enabled: bool, endpoint: &EndpointInfo) -> McpStatus {
    let url = endpoint.url.as_ref().map(|base| mcp_url(base));
    let snippets = url
        .as_deref()
        .map(|url| snippets(url, &endpoint.token))
        .unwrap_or_default();
    McpStatus {
        enabled,
        url,
        snippets,
    }
}

/// The MCP URL on the endpoint at `base` (`http://127.0.0.1:<port>`).
pub fn mcp_url(base: &str) -> String {
    format!("{}{MCP_PATH}", base.trim_end_matches('/'))
}

/// One snippet per client for the server at `url` with `token`.
pub fn snippets(url: &str, token: &str) -> Vec<McpSnippet> {
    let bearer = format!("Bearer {token}");
    // The token is hex and the URL a loopback address: neither needs quoting
    // beyond the double quotes around the header, in any shell.
    let claude = format!(
        "claude mcp add --transport http {MCP_SERVER_NAME} {url} --header \"Authorization: {bearer}\""
    );
    let http = serde_json::json!({
        "mcpServers": {
            MCP_SERVER_NAME: {
                "type": "http",
                "url": url,
                "headers": { "Authorization": bearer },
            }
        }
    });
    vec![
        McpSnippet {
            client: McpClient::ClaudeCode,
            text: claude,
        },
        McpSnippet {
            client: McpClient::Http,
            text: serde_json::to_string_pretty(&http).unwrap_or_else(|_| http.to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(url: Option<&str>) -> EndpointInfo {
        EndpointInfo {
            url: url.map(str::to_owned),
            token: "0a".repeat(32),
        }
    }

    #[test]
    fn the_snippets_carry_the_real_url_and_token() {
        let st = status(true, &endpoint(Some("http://127.0.0.1:51234")));
        assert!(st.enabled);
        assert_eq!(st.url.as_deref(), Some("http://127.0.0.1:51234/mcp"));
        assert_eq!(st.snippets.len(), 2);

        let claude = &st.snippets[0];
        assert_eq!(claude.client, McpClient::ClaudeCode);
        assert_eq!(
            claude.text,
            format!(
                "claude mcp add --transport http font-manager http://127.0.0.1:51234/mcp --header \"Authorization: Bearer {}\"",
                "0a".repeat(32)
            )
        );

        let http = &st.snippets[1];
        assert_eq!(http.client, McpClient::Http);
        let json: serde_json::Value = serde_json::from_str(&http.text).expect("valid JSON");
        let server = &json["mcpServers"]["font-manager"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], "http://127.0.0.1:51234/mcp");
        assert_eq!(
            server["headers"]["Authorization"],
            format!("Bearer {}", "0a".repeat(32))
        );
    }

    #[test]
    fn no_url_and_no_snippets_while_the_endpoint_does_not_run() {
        let st = status(false, &endpoint(None));
        assert!(!st.enabled);
        assert_eq!(st.url, None);
        assert!(st.snippets.is_empty());
    }

    #[test]
    fn a_trailing_slash_on_the_base_is_not_doubled() {
        assert_eq!(mcp_url("http://127.0.0.1:1/"), "http://127.0.0.1:1/mcp");
    }
}
