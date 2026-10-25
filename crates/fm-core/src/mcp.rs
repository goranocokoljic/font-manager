//! The MCP server's status and the config snippets that connect a client to
//! it (E27.1), and what its mutating tools may do (E27.6). The server itself
//! is `fm-mcp`, mounted on the local API endpoint at [`MCP_PATH`]; this module
//! says where it is, how a client reaches it with the endpoint's token, which
//! tools the user granted, and asks the user before a destructive call.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use fm_api::settings::{
    EndpointInfo, MCP_CONFIRM_TIMEOUT_SECS, MCP_CONFIRM_TOOLS, MCP_MUTATING_TOOLS, MCP_PATH,
    MCP_SERVER_NAME, McpClient, McpGrant, McpSnippet, McpStatus,
};
use fm_api::{ApiError, ApiResult, ErrorCode, Event};

/// The status for the switch `enabled`, the endpoint `endpoint` and the
/// granted tools `granted`: the URL and the snippets while the endpoint
/// runs, none while it does not.
pub fn status(enabled: bool, endpoint: &EndpointInfo, granted: &BTreeSet<String>) -> McpStatus {
    let url = endpoint.url.as_ref().map(|base| mcp_url(base));
    let snippets = url
        .as_deref()
        .map(|url| snippets(url, &endpoint.token))
        .unwrap_or_default();
    McpStatus {
        enabled,
        url,
        snippets,
        grants: grants(granted),
    }
}

/// One grant per mutating tool, in [`MCP_MUTATING_TOOLS`] order: on when
/// the tool is in `granted`.
pub fn grants(granted: &BTreeSet<String>) -> Vec<McpGrant> {
    MCP_MUTATING_TOOLS
        .iter()
        .map(|&tool| McpGrant {
            tool: tool.to_owned(),
            granted: granted.contains(tool),
            confirm: needs_confirmation(tool),
        })
        .collect()
}

/// Whether `tool` needs a grant: it changes something.
pub fn needs_grant(tool: &str) -> bool {
    MCP_MUTATING_TOOLS.contains(&tool)
}

/// Whether every call of `tool` also asks the user.
pub fn needs_confirmation(tool: &str) -> bool {
    MCP_CONFIRM_TOOLS.contains(&tool)
}

/// The name of the Settings switch that grants `tool`, as Settings → AI
/// assistants shows it in English.
pub fn switch_label(tool: &str) -> &str {
    match tool {
        "activate_fonts" => "Activate fonts",
        "deactivate_fonts" => "Deactivate fonts",
        "tag_fonts" => "Tag fonts",
        "export_web" => "Export web fonts",
        other => other,
    }
}

/// The error of a call to `tool` while its grant is off: it names the switch.
pub fn not_granted(tool: &str) -> ApiError {
    let label = switch_label(tool);
    ApiError::new(
        ErrorCode::PermissionDenied,
        format!(
            "AI assistants may not use {tool} yet. Turn on \"{label}\" in Settings → AI assistants to allow it."
        ),
    )
    .with_detail(format!("tool {tool} is not granted"))
}

/// The error of a call to `tool` the user denied or did not answer in time.
pub fn not_confirmed(tool: &str) -> ApiError {
    ApiError::new(
        ErrorCode::PermissionDenied,
        format!("The user did not allow {tool}. Nothing was changed."),
    )
    .with_detail(format!("tool {tool} was denied or timed out"))
}

/// The confirmations waiting for the user's Allow or Deny.
#[derive(Debug)]
pub struct Confirmations {
    /// Waiting calls by id: the sender wakes the call with the answer.
    pending: Mutex<HashMap<u32, mpsc::Sender<bool>>>,
    next: AtomicU32,
    timeout: Duration,
}

impl Default for Confirmations {
    fn default() -> Self {
        Self::new(Duration::from_secs(u64::from(MCP_CONFIRM_TIMEOUT_SECS)))
    }
}

impl Confirmations {
    /// No confirmation waiting; a call waits up to `timeout`.
    pub fn new(timeout: Duration) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
            timeout,
        }
    }

    /// Asks the user whether `tool` may act on `faces` faces: sends
    /// [`Event::McpConfirmationRequested`] through `emit` and blocks until
    /// [`Self::answer`] or the timeout. No answer is a Deny, and then
    /// [`Event::McpConfirmationClosed`] tells the UI to take the toast away.
    pub fn ask(&self, tool: &str, faces: Option<u32>, emit: &dyn Fn(&Event)) -> bool {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.lock().insert(id, tx);
        let timeout_secs =
            u32::try_from(self.timeout.as_millis().div_ceil(1000)).unwrap_or(u32::MAX);
        emit(&Event::McpConfirmationRequested {
            id,
            tool: tool.to_owned(),
            faces,
            timeout_secs,
        });
        if let Ok(allowed) = rx.recv_timeout(self.timeout) {
            return allowed;
        }
        // Timed out. An answer that came in the meantime already took the
        // entry, under the lock, and sent before it let go: it counts.
        if self.lock().remove(&id).is_none() {
            return rx.try_recv().unwrap_or(false);
        }
        emit(&Event::McpConfirmationClosed { id, allowed: false });
        false
    }

    /// The user's answer to confirmation `id`; the waiting call wakes and
    /// [`Event::McpConfirmationClosed`] goes out through `emit`. `NotFound`
    /// once it was answered or timed out.
    pub fn answer(&self, id: u32, allow: bool, emit: &dyn Fn(&Event)) -> ApiResult<()> {
        {
            let mut pending = self.lock();
            let Some(tx) = pending.remove(&id) else {
                return Err(ApiError::new(
                    ErrorCode::NotFound,
                    "This request from an AI assistant was already answered or timed out.",
                )
                .with_detail(format!("confirmation {id}")));
            };
            // The call may have stopped waiting; then nobody hears the answer.
            let _ = tx.send(allow);
        }
        emit(&Event::McpConfirmationClosed { id, allowed: allow });
        Ok(())
    }

    /// How many confirmations wait for an answer.
    pub fn waiting(&self) -> usize {
        self.lock().len()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<u32, mpsc::Sender<bool>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
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
        let st = status(
            true,
            &endpoint(Some("http://127.0.0.1:51234")),
            &BTreeSet::new(),
        );
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
        let st = status(false, &endpoint(None), &BTreeSet::new());
        assert!(!st.enabled);
        assert_eq!(st.url, None);
        assert!(st.snippets.is_empty());
    }

    #[test]
    fn a_trailing_slash_on_the_base_is_not_doubled() {
        assert_eq!(mcp_url("http://127.0.0.1:1/"), "http://127.0.0.1:1/mcp");
    }

    #[test]
    fn every_mutating_tool_has_a_grant_off_by_default_and_deactivate_asks() {
        let st = status(false, &endpoint(None), &BTreeSet::new());
        let tools: Vec<&str> = st.grants.iter().map(|g| g.tool.as_str()).collect();
        assert_eq!(tools, MCP_MUTATING_TOOLS);
        assert!(st.grants.iter().all(|g| !g.granted), "all off");
        let asks: Vec<&str> = st
            .grants
            .iter()
            .filter(|g| g.confirm)
            .map(|g| g.tool.as_str())
            .collect();
        assert_eq!(asks, ["deactivate_fonts"]);

        let granted = BTreeSet::from(["tag_fonts".to_owned()]);
        let st = status(false, &endpoint(None), &granted);
        let on: Vec<&str> = st
            .grants
            .iter()
            .filter(|g| g.granted)
            .map(|g| g.tool.as_str())
            .collect();
        assert_eq!(on, ["tag_fonts"]);
        assert!(!needs_grant("search_fonts"), "read tools need no grant");
    }

    #[test]
    fn the_refusal_names_the_settings_switch() {
        let e = not_granted("deactivate_fonts");
        assert_eq!(e.code, ErrorCode::PermissionDenied);
        assert!(e.message.contains("\"Deactivate fonts\""), "{}", e.message);
        assert!(
            e.message.contains("Settings → AI assistants"),
            "{}",
            e.message
        );
        for tool in MCP_MUTATING_TOOLS {
            assert_ne!(switch_label(tool), tool, "{tool} has a switch label");
        }
    }

    type Seen = std::sync::Arc<Mutex<Vec<Event>>>;

    fn collect() -> (Seen, impl Fn(&Event) + Send + 'static) {
        let seen: Seen = std::sync::Arc::default();
        let sink = std::sync::Arc::clone(&seen);
        (seen, move |e: &Event| {
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(e.clone());
        })
    }

    fn events(seen: &Seen) -> Vec<Event> {
        seen.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    #[test]
    fn an_answer_wakes_the_waiting_call() {
        let confirmations = std::sync::Arc::new(Confirmations::new(Duration::from_secs(10)));
        let (asked, emit) = collect();
        let asker = std::sync::Arc::clone(&confirmations);
        let call = std::thread::spawn(move || asker.ask("deactivate_fonts", Some(3), &emit));
        while confirmations.waiting() == 0 {
            std::thread::yield_now();
        }
        let (answered, emit) = collect();
        confirmations.answer(1, true, &emit).expect("answered");
        assert!(call.join().expect("joined"), "allowed");
        assert_eq!(
            events(&asked),
            [Event::McpConfirmationRequested {
                id: 1,
                tool: "deactivate_fonts".into(),
                faces: Some(3),
                timeout_secs: 10,
            }]
        );
        assert_eq!(
            events(&answered),
            [Event::McpConfirmationClosed {
                id: 1,
                allowed: true
            }]
        );
        let again = confirmations.answer(1, false, &|_| {}).expect_err("gone");
        assert_eq!(again.code, ErrorCode::NotFound);
    }

    #[test]
    fn no_answer_in_time_is_a_deny_and_closes_the_toast() {
        let confirmations = Confirmations::new(Duration::from_millis(30));
        let (seen, emit) = collect();
        assert!(!confirmations.ask("deactivate_fonts", None, &emit));
        assert_eq!(confirmations.waiting(), 0);
        let seen = events(&seen);
        assert_eq!(seen.len(), 2);
        assert!(matches!(
            seen[0],
            Event::McpConfirmationRequested {
                id: 1,
                faces: None,
                timeout_secs: 1,
                ..
            }
        ));
        assert_eq!(
            seen[1],
            Event::McpConfirmationClosed {
                id: 1,
                allowed: false
            }
        );
        let late = confirmations
            .answer(1, true, &|_| {})
            .expect_err("timed out");
        assert_eq!(late.code, ErrorCode::NotFound);
    }
}
