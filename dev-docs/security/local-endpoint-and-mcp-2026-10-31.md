# Security review: the local API endpoint and the MCP grants (E36.1)

Date: 2026-10-31. Scope: `crates/fm-endpoint` (HTTP and the pipe), the token and
discovery file (`fm-core/src/endpoint.rs`, `fm-platform/src/fs.rs`), the MCP server, gate
and shim (`crates/fm-mcp`, `fm-core/src/mcp.rs`), the client (`crates/fm-client`), the
webview policy (`tauri.conf.json`). Method: read against ADR 0015 and ADR 0016, run the
existing tests, and probe the installed app (0.1.0, API 0.23.0) on Windows 11 with raw
HTTP requests (`probe-endpoint.py`, transcript in §4).

## 1. Threat model

- **Who is kept out:** other users of the machine, and web pages in a browser (direct
  requests, DNS rebinding, CSRF-style posts). The token is the wall for programs; the
  OS is the wall for the pipe (ADR 0016).
- **Who is let in:** every program of the logged-in user that reads the token. The CLI
  and the plugins need that. An AI assistant's host (Claude Code with the HTTP snippet,
  Claude Desktop through the shim) is such a program.
- **What the MCP switches are for:** to limit what an assistant does *through its Font
  Manager tools*: nothing changes until the user grants a tool, and a deactivation asks
  every time. They are not, and cannot be, a wall against a program that runs with the
  user's rights: such a program can run `fm activate` directly.

## 2. Verified

| Control | Where | How it was checked |
|---|---|---|
| Loopback only, port picked by the OS | `Endpoint::serve` binds `127.0.0.1:0` | code; `it_listens_on_loopback_on_a_port_the_os_picked` |
| Any `Origin` header → 403 before the token is read | `check_origin` | code; live probe (token + `Origin: https://evil.example` → 403) |
| `Host` must be `127.0.0.1:<port>` or `localhost:<port>` | `allowed_host` | code; unit test; live probes (`evil.example`, other port → 403) |
| Token: 32 bytes from `getrandom`, 64 hex digits, constant-time compare | `new_token`, `same_token` | code; `two_tokens_differ`, `tokens_match_only_when_equal`; live probe (wrong token → 401, `WWW-Authenticate: Bearer`) |
| Regenerate replaces the token at once | `EndpointKeys::regenerate` | `a_regenerated_token_works_and_the_old_one_fails_at_once` |
| Token and discovery file private to the user | `write_private_file` (0600; protected DACL on Windows), re-privatised when found open | code; `icacls` on the live files: `MichaPC\Micha:(F)` only |
| Token never logged | grep of `tracing!`/`println!`/`console.*` for `token` | none; the shim's `Debug` for `Target` omits it |
| Snippets with the token hidden until Show | `McpSettings.vue` | unit test `hides the snippet text until Show snippets is pressed` |
| Body limit 32 MiB (HTTP), 1 MiB per frame (pipe) | `MAX_BODY_BYTES`, `MAX_FRAME_BYTES` | `a_body_over_the_limit_is_refused`, `a_frame_over_one_mib_is_refused_and_the_connection_stays` |
| MCP off by default; `/mcp` is 404 while off, checked per request | `mcp()` asks `mcp_status` every time | `the_switch_turns_mcp_on_and_off_without_a_restart`; live probe (no token → 401) |
| Every mutating tool asks the gate; the gate is `fm-core`'s grants | `tools::call` → `ToolGate::check` → `authorize_mcp_tool` | `workflow_tools_run_on_the_core_and_are_refused_until_granted`; E27.8 transcript (refusals name the switch) |
| `deactivate_fonts` asks the user; no answer in 60 s is Deny | `Confirmations::ask` | E27.8 transcript: timeout → `permissionDenied`; Allow → `done` |
| Deactivate only touches faces the app registered | core activation | `deactivate_fonts_only_reaches_faces_the_app_registered_on_the_core` |
| Preview inputs bounded | `MAX_SIZE_PX` 300, `MAX_COMPARE_TEXT_CHARS` 500, 2–4 faces | code |
| Shim sends the token only to a loopback address of a live process | `fm_client::connect`: pid check, `loopback_addr`, 500 ms connect | code |
| Webview policy | `default-src 'self'`, no remote scripts, `connect-src` only the IPC schemes | `tauri.conf.json`; `window_config.rs` tests |
| Pipe private to the user (DACL / 0600 socket, 0700 dir) | `IpcListener::is_private` | ADR 0016; CI tests on three OSes |

## 3. Findings

### F1 — Token holders could change the MCP grants and answer confirmations (medium) — fixed

Every `fm-api` service method had a route, so `set_mcp_enabled`, `set_mcp_grant`,
`authorize_mcp_tool` and `answer_mcp_confirmation` were served to any token holder over
HTTP and the pipe. Reproduced live: `POST /v1/set_mcp_grant` with the token changed a
grant; `POST /v1/answer_mcp_confirmation` reached the service (404 for an unknown id).
An assistant's host holds the token (Claude Code's config carries it; the shim reads it),
so an agent with a shell could have granted itself every tool and approved its own
deactivations, which empties the consent the switches stand for.

**Fix (this PR):** `fm_endpoint::routes::WINDOW_ONLY` lists the four methods;
`routes::dispatch` refuses them with `permissionDenied` ("Only the Font Manager window
can change what AI assistants may do.") before any body is parsed, over HTTP (403) and
the pipe alike. The window calls them through Tauri commands; the app's MCP server asks
the gate in-process on `CoreApi`, so tools still work. `mcp_status` stays readable.
Tests: `window_only_methods_are_refused_with_403_and_never_reach_the_service`,
`window_only_methods_are_refused_over_the_pipe`, and the client conformance test now
expects the refusal. ADR 0015 records the rule.

What this does not change: a program with the user's rights can still run `fm activate`
or `fm deactivate` (direct mode, no grants). The help page says so now ("Keep it safe").

### F2 — The confirmation toast is unseen while the window is hidden (low) — follow-up

`McpConfirmationRequested` is handled by the window's store only; nothing shows or
focuses a window hidden to the tray, and there is no system notification. The call then
times out and is denied: fail-closed, but the user does not learn that an assistant
asked. Follow-up story #417 (E27.9): bring the window to the front when a confirmation
arrives.

### F3 — Exports write where the assistant says (info) — documented

With **Export web fonts** (and **Activate fonts** for `prepare_for_web`), an assistant
names the output folder, and since E27.8 a missing folder is created. Existing files are
never replaced without `overwrite`. This is the grant's meaning; the help page now says
it plainly.

### F4 — `GET /v1/version` needs no token (info) — accepted

By design (ADR 0015): a client learns the API version before it reads the discovery
file. A browser page cannot read the answer (no CORS headers) but can tell the port
answers. It reveals that Font Manager runs and its version, nothing else.

### F5 — No connection limits or header timeouts on the HTTP listener (low) — accepted

A local process could hold many connections open. Only the user's own processes reach
the socket, and the event stream already drops slow clients. Not worth a change before
1.0; noted.

### F6 — Confirmation ids are sequential (info) — accepted

`answer_mcp_confirmation` is window-only now, so an id is worth nothing to a program.

## 4. Live probe transcript (installed app, token redacted)

```
GET /v1/version, no token                            -> 200 {"appVersion":"0.1.0","apiVersion":{"major":0,"minor":23,"patch":0},"deprecations":[]}
POST list_sources, no token                          -> 401 permissionDenied "A valid token is required (Authorization: Bearer <token>)."
POST list_sources, wrong token                       -> 401 permissionDenied
POST list_sources, token + Origin header             -> 403 permissionDenied "Requests from a web page are not allowed."
POST list_sources, token, Host evil.example          -> 403 permissionDenied "The Host header must be 127.0.0.1 or localhost with this port."
POST list_sources, token, Host 127.0.0.1 other port  -> 403 permissionDenied
POST list_sources, token (good)                      -> 200 [{"id":1,"path":"H:\\Fonts New",...}]
POST /mcp, no token                                  -> 401 permissionDenied
POST answer_mcp_confirmation id 999999, token        -> 404 notFound (reached the service: F1, before the fix)
POST set_mcp_grant tag_fonts=false, token            -> 200 (changed a grant: F1, before the fix)
GET  /v1/regenerate_endpoint_token, token            -> 405 "Every route takes POST."
```

File permissions of the live files:

```
C:\Users\Micha\AppData\Local\FontManager\data\endpoint.json   MichaPC\Micha:(F)
C:\Users\Micha\AppData\Local\FontManager\data\endpoint-token  MichaPC\Micha:(F)
```

## 5. Out of scope, for later reviews

- The Adobe UXP/CEP and Sketch plugins' use of the pipe (E28–E30): the pipe's trust
  boundary is the user account; the plugins add no secret of their own.
- The updater's signature check (ADR 0012) and code signing of the binaries (#129, #131,
  #189, postponed).
- Font parsing robustness against malformed files (fontations; `Health::Broken`).
