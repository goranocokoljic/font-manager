//! The MCP server's status and the config snippets that connect a client to
//! it (E27.1). The server itself is `fm-mcp`, mounted on the local API
//! endpoint at [`MCP_PATH`]; this module only says where it is and how a
//! client reaches it, with the endpoint's token. Clients that only start a
//! process (Claude Desktop) run the stdio shim `fm-mcp-stdio` (E27.7); the
//! command that starts it is per OS ([`fm_platform::mcp_shim::command`]).

use fm_api::settings::{EndpointInfo, MCP_PATH, MCP_SERVER_NAME, McpClient, McpSnippet, McpStatus};
use fm_platform::mcp_shim::ShimCommand;

/// The status for the switch `enabled` and the endpoint `endpoint`: the URL
/// and the snippets while the endpoint runs, none while it does not. `shim`
/// is the command that starts the stdio shim; without one there is no
/// Claude Desktop snippet.
pub fn status(enabled: bool, endpoint: &EndpointInfo, shim: Option<&ShimCommand>) -> McpStatus {
    let url = endpoint.url.as_ref().map(|base| mcp_url(base));
    let snippets = url
        .as_deref()
        .map(|url| snippets(url, &endpoint.token, shim))
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

/// One snippet per client for the server at `url` with `token`, and a
/// Claude Desktop one when `shim` starts the stdio shim.
pub fn snippets(url: &str, token: &str, shim: Option<&ShimCommand>) -> Vec<McpSnippet> {
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
    let mut all = vec![
        McpSnippet {
            client: McpClient::ClaudeCode,
            text: claude,
        },
        McpSnippet {
            client: McpClient::Http,
            text: pretty(&http),
        },
    ];
    if let Some(shim) = shim {
        all.push(McpSnippet {
            client: McpClient::ClaudeDesktop,
            text: pretty(&desktop(shim)),
        });
    }
    all
}

/// The Claude Desktop `mcpServers` block: the shim's command by its absolute
/// path. The shim finds the address and the token itself when it starts, so
/// a new port or token needs no new snippet.
fn desktop(shim: &ShimCommand) -> serde_json::Value {
    serde_json::json!({
        "mcpServers": {
            MCP_SERVER_NAME: {
                "command": shim.program.to_string_lossy(),
                "args": shim.args,
            }
        }
    })
}

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use fm_platform::mcp_shim::SHIM_ARG;

    fn shim(program: PathBuf, args: &[&str]) -> ShimCommand {
        ShimCommand {
            program,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        }
    }

    fn endpoint(url: Option<&str>) -> EndpointInfo {
        EndpointInfo {
            url: url.map(str::to_owned),
            token: "0a".repeat(32),
        }
    }

    #[test]
    fn the_snippets_carry_the_real_url_and_token() {
        let st = status(true, &endpoint(Some("http://127.0.0.1:51234")), None);
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
        let st = status(
            false,
            &endpoint(None),
            Some(&shim(PathBuf::from("/opt/fm/fm-mcp-stdio"), &[])),
        );
        assert!(!st.enabled);
        assert_eq!(st.url, None);
        assert!(st.snippets.is_empty());
    }

    #[test]
    fn the_claude_desktop_snippet_starts_the_shim_by_its_absolute_path() {
        // A path with a space and, on Windows, backslashes: both must survive
        // as one JSON string.
        let program = std::env::temp_dir()
            .join("Font Manager")
            .join("fm-mcp-stdio.exe");
        let st = status(
            true,
            &endpoint(Some("http://127.0.0.1:51234")),
            Some(&shim(program.clone(), &[])),
        );
        assert_eq!(st.snippets.len(), 3);
        let desktop = &st.snippets[2];
        assert_eq!(desktop.client, McpClient::ClaudeDesktop);
        let json: serde_json::Value = serde_json::from_str(&desktop.text).expect("valid JSON");
        let server = &json["mcpServers"]["font-manager"];
        assert_eq!(server["command"], program.to_string_lossy().as_ref());
        assert_eq!(server["args"], serde_json::json!([]));
        assert!(
            !desktop.text.contains(&"0a".repeat(32)),
            "the shim reads the token itself; the snippet does not carry it"
        );
        assert!(server.get("url").is_none());
    }

    #[test]
    fn the_appimage_snippet_starts_the_appimage_with_the_shim_argument() {
        let appimage = PathBuf::from("/home/ada/Apps/Font_Manager.AppImage");
        let st = status(
            true,
            &endpoint(Some("http://127.0.0.1:51234")),
            Some(&shim(appimage.clone(), &[SHIM_ARG])),
        );
        let json: serde_json::Value =
            serde_json::from_str(&st.snippets[2].text).expect("valid JSON");
        let server = &json["mcpServers"]["font-manager"];
        assert_eq!(server["command"], appimage.to_string_lossy().as_ref());
        assert_eq!(server["args"], serde_json::json!(["--mcp-stdio"]));
    }

    #[test]
    fn a_trailing_slash_on_the_base_is_not_doubled() {
        assert_eq!(mcp_url("http://127.0.0.1:1/"), "http://127.0.0.1:1/mcp");
    }
}
