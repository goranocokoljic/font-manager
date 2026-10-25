# 0015 — The local API endpoint

Status: Accepted (2026-10-24, E25.1)

## Context

The CLI (when the app is open), the MCP server's stdio shim and the design-app
plugins need to reach the running app. ADR 0006 names "the localhost endpoint"
as one of the four adapters over `fm-api`. It must serve the whole contract,
stay out of the window's way at start (cold start < 1 s), and admit only
programs of the logged-in user: other users of the machine, and web pages in a
browser, must not get in. Rule 7 (no network without a user action) is not
touched: the socket is loopback only.

The story asked for `docs/adr/0011-local-api-endpoint.md`; 0011 was already
taken by start at login, so this is 0015.

## Decision

**Crate.** `crates/fm-endpoint` is transport only, over `Arc<dyn Api>`:
hyper 1 (HTTP/1.1 server), hyper-util (the tokio adapter) and tokio (a
current-thread runtime on a thread of its own, `spawn_blocking` for every
service call as ADR 0006 says). No other HTTP crate; the two body helpers it
needs are twenty lines in `body.rs`. `fm-app` hosts it: `AppState::start_endpoint`
starts it on a thread after the window is set up, so the window never waits;
the quit path stops it. Only the real library (`CoreApi`) is served; the
`MockApi` fallback has no data folder to publish into.

**Routes.** One per service method: `POST /v1/<method>`, the method's Rust
name (`list_families`, `regenerate_endpoint_token`). The body is the method's
request value as JSON (the same camelCase wire format as the UI); a method
without one takes an empty body, `null` or `{}`. The answer is the value as
JSON, `null` for `()`. The table is a macro in `routes.rs`; a test parses
every `pub trait *Service` in `fm-api` and fails when a method has no route or
a route no method. `EventSource::subscribe` is not a service; it is the
event stream below.

**Event stream** (E25.2). `GET /v1/events` streams every `fm_api::Event`
as Server-Sent Events (`Content-Type: text/event-stream`), after the same
three checks as every route (another method gets 405, `Allow: GET`). The
first frame is the comment `: stream open`, so a client knows it is
subscribed. Each event is one frame:

```
id: 3
event: scanFinished
data: {"type":"scanFinished","job":7,"added":12,"changed":0,"removed":0,"problems":0}

```

`event` is the event's `type` tag, `data` the event as the same JSON the UI
gets (always one line), `id` the stream's own sequence number from 1. A
stream sees the events sent after it opened; there is no replay, so
`Last-Event-ID` is ignored and a client that reconnects reads again the
state it needs. A stream that sent nothing for 15 s gets the comment
`: keep-alive`.

Each connection is one `EventSource::subscribe` and one thread
(`fm-endpoint-events`). The thread moves events from the subscription (an
unbounded channel, so the sender never waits) into a queue of 1,024 frames
that the response body reads. **Slow-client rule:** a client that stops
reading fills the queue; on the next frame the server closes that
connection (not only the body: hyper polls a body only when the socket
takes bytes) and the thread ends, which drops the subscription. The API,
the UI's own subscription and other streams never wait on a stream. A
client closed this way reconnects. A client that went away is noticed on
the next frame the server writes (an event or the keep-alive); its thread
ends within 200 ms of that.

**Status by `ErrorCode`.** An `ApiError` keeps its JSON shape
(`{ code, message, detail }`); the status is fixed:

| `ErrorCode` | HTTP |
|-------------|------|
| `notFound` | 404 |
| `invalidRequest` | 400 |
| `conflict` | 409 |
| `permissionDenied` | 403 |
| `unsupported` | 422 |
| `busy` | 503 |
| `io`, `internal` | 500 |
| `cancelled` | 410 |

The transport's own refusals use the same shape: 401 (no or wrong token,
`permissionDenied`, `WWW-Authenticate: Bearer`), 403 (Host or Origin), 404
(no such route), 405 (not POST, `Allow: POST`), 413 (body over 32 MiB), 400
(a body that is not the method's JSON; its `detail` starts `request body: `).

**Checks, in this order.** (1) Any `Origin` header → 403: every cross-site
`fetch` or form post from a web page carries one, a local program does not.
(2) `Host` must be `127.0.0.1:<port>` or `localhost:<port>` → else 403: a
page that rebinds its own host name to 127.0.0.1 sends that name. (3)
`Authorization: Bearer <token>` compared in constant time with the token
`SettingsService::endpoint_info` reports at that moment → else 401. Nothing
else runs before these pass, except `GET /v1/version` (below), which runs
after (1) and (2) and needs no token. (4) The client's `Fm-Api-Version`,
when it sends one (below) → 400 or 422.

**Versions** (E25.3). The rules live in `fm_api::version`, so the
endpoint, the CLI and the MCP server negotiate the same way.

- Every answer, refusals and the event stream included, carries
  `Fm-Api-Version: <API_VERSION>` (`major.minor.patch`).
- `GET /v1/version` answers `fm_api::VersionInfo`:
  `{ "appVersion", "apiVersion": { major, minor, patch }, "deprecations": [...] }`.
  No token, so a client can tell whether it can talk to this app before it
  reads the discovery file; the `Origin` and `Host` checks still apply.
  Another method gets 405, `Allow: GET`.
- **Additive within a major.** A minor adds types, routes and
  `Option`/`#[serde(default)]` fields (ADR 0006); a patch changes no
  contract. So a client of the same major is served at any minor or patch:
  an older one asks for less, and the request fields a newer one adds are
  ignored (no request type denies unknown fields).
- **Refusal on major mismatch.** A client sends `Fm-Api-Version` with the
  contract it was built against. Not `major.minor.patch` → 400
  `invalidRequest`. Another major → 422 `unsupported` with one sentence
  naming both versions and what to update: "The client speaks API 1.0.0 but
  Font Manager serves API 0.22.0: update Font Manager to a version that
  serves API 1.x." (an older client is told to update the client). No
  header → served, as before E25.3. The check runs after the token, so a
  program without the token learns nothing more than `/v1/version` says.
- The served major is `fm_api::NEGOTIATED_MAJOR`, kept apart from
  `API_VERSION` on purpose: a test fails when `API_VERSION`'s major
  changes and it does not, so a major bump is a deliberate step that
  revisits these rules.
- **Deprecation notes.** `fm_api::deprecations()` lists the fields on
  their way out (`typeName`, wire `field`, `since`, `note`); it is empty
  today. A field stays working until the next major removes it. An answer
  that contains one (its type is the method's answer type or reachable
  from it, and the field is a key in the answer) carries one
  `Fm-Deprecation: <Type>.<field> (since <version>): <note>` header per
  field. Notes are plain ASCII, one sentence: what to use instead.

**Token.** 32 bytes from the OS random source (`getrandom`), 64 lowercase hex
digits, in `<data>/endpoint-token`, so it survives a restart. Settings →
Local API shows it masked with Show, Copy and Regenerate
(`SettingsService::endpoint_info`, `regenerate_endpoint_token`).
Regenerate saves the new token at once; because the endpoint asks for the
current token on every request, the old one fails from the next request on.

**Discovery file.** While the endpoint runs, `<data>/endpoint.json` holds
`{ "url": "http://127.0.0.1:<port>", "token", "pid", "apiVersion" }`
(`fm_core::endpoint::Discovery`). It is written after the socket is bound,
rewritten on Regenerate, and removed by the quit path, only when its `pid` is
this process. The port is the one the OS picked (bind to port 0), so it
changes from run to run; programs read the file.

**Private files, per OS** (`fm_platform::fs::write_private_file`, used for
both files). The bytes go to a temporary sibling that is private before it
is written, then it is renamed over the target.

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Permission | a protected DACL (`D:P(A;;FA;;;<user SID>)`): one entry, full access for the user; nothing inherited, so no Administrators or SYSTEM entry | mode `0600` | mode `0600` |
| Set by | `SetNamedSecurityInfoW` on the empty temporary file; a rename on the same volume keeps it | `open(…, 0600)` plus `fchmod` (the mode applies only to a new file) | the same |
| Checked by | `is_private_file`: the DACL is protected and is exactly that one entry (Windows adds `AI` itself) | no group or other bits | the same |

A token file that another program made readable is written again privately
the next time it is read.

**Client** (E25.4). `crates/fm-client` is the one client: `Client`
implements `fm_api::Api` by calling the routes above (every service method,
and `EventSource::subscribe` over `GET /v1/events`). `fm-cli` and `fm-mcp`
hold it as `Arc<dyn Api>` (`fm_client::running_app()`) and have no HTTP
code or HTTP crate of their own.

- **Discovery.** `fm_client::connect(&AppPaths)` reads `endpoint.json`
  (`fm_core::endpoint::Discovery::read`) and returns `None` when the file
  is missing or broken, its `pid` is gone (`fm_platform::process::is_running`:
  `OpenProcess` on Windows, `kill -0` on macOS, `/proc/<pid>` on Linux),
  its `url` is not loopback (the token is never sent elsewhere), or the
  port does not accept a connection within 500 ms.
- **Blocking, on `std::net`.** The service traits are synchronous and the
  MCP host is async: a client with its own async runtime would panic when
  called from inside another one. So each call is one connection
  (`Connection: close`) with a 500 ms connect timeout; hyper and tokio stay
  in the server.
- **Versions.** Every request carries the bearer token and
  `Fm-Api-Version`. The app's error answer (422 `unsupported` on a major
  mismatch) comes back as it is; an answer whose `Fm-Api-Version` names
  another major is refused by the client itself
  (`fm_api::check_server_version`, the same sentence).
- **Failures.** No answer → `io` "Font Manager is not running, or its local
  API does not answer."; a connection lost mid-call or an answer cut short
  → `io` "The connection to Font Manager was lost."; an answer that is not
  the local API → `internal`. Never a panic.
- **Events.** `subscribe` returns once the server's `: stream open` frame
  arrived, so the subscription sees every event sent after it. When the
  stream ends the receiver disconnects; an event this client does not know
  (a newer minor) is skipped. A client that regenerates the token keeps
  working with the new one.

**CLI modes** (E26.6). `fm` calls `fm_client::connect` over its app
folders (`FM_HOME` included) on start (`fm_cli::link::reach`). The app
answers → endpoint mode: every command goes through the `Client` and `fm`
never opens the index, so activation state and events are the app's (`fm
scan` reads progress from `GET /v1/events`). No answer → direct-link mode on
`fm-core` and the index; a stale `endpoint.json` costs at most the 500 ms
connect timeout. `--direct` skips the lookup. A wrong token is not a stale
file: the client turns the 401 into `permissionDenied` with a message that
names Settings > Local API, and `fm` exits 4 instead of falling back.
`fm version --json` adds `mode` (`endpoint` or `direct`) to the `AppInfo`
of the API it reached.

**MCP server** (E27.1). `fm-mcp` serves the MCP server over streamable
HTTP at `/mcp` on this endpoint, after the same checks as every route
(`Origin`, `Host`, token, `Fm-Api-Version`). `fm-endpoint` stays free of
rmcp: it has a `Mount` trait (`fm_endpoint::mount`) for that one path.
`fm-mcp` keeps no HTTP crate of its own (E25.4): `fm_mcp::HttpServer`
gives rmcp's service, and the app's `McpMount` (a few lines in `fm-app`)
joins the two for `Endpoint::start_with_mcp`. The
endpoint asks `SettingsService::mcp_status` on every `/mcp` request, as it
asks for the token: while the user has the server off (the default, stored
as `mcpEnabled` in `config.json`), `/mcp` is 404 and the mount is never
called, so the switch applies at once and needs no restart. The mount
builds rmcp's service on its first request and runs it stateless (no
sessions, JSON answers; `GET` and `DELETE` get 405), so nothing runs while
the server is off and nothing is left when it is turned off.
`initialize` reports the server `font-manager` at `fm_api::API_VERSION`.
Settings → AI assistants shows the switch and config snippets for Claude
Code and generic HTTP clients, which `fm-core` writes with the real URL and
token (`fm_core::mcp`); the port changes from run to run, so the snippets do
too.

## Consequences

- The CLI's endpoint mode (when the app is open), the MCP shim and the
  plugins read `endpoint.json` and send the token; none needs its own
  discovery.
- A new service method needs a line in `routes.rs`, or CI fails.
- An MCP client is a program of the same user with the token, so it is
  inside the same trust boundary; the switch only decides whether `/mcp`
  answers at all.
- A program of the same user can do everything the UI can. That is the
  trust boundary: the same as the user's own files.
- The endpoint is HTTP/1.1 without TLS: it never leaves the loopback
  interface.
- Clients get events from `GET /v1/events` instead of polling. One that
  falls 1,024 frames behind is closed and must reconnect.
- A client that sends `Fm-Api-Version` is refused plainly across a major
  instead of failing on a changed field; one that reads `Fm-Deprecation`
  learns about a removal a whole major ahead.
