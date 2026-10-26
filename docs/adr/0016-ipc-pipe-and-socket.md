# 0016 — The IPC endpoint: named pipe and Unix socket

Status: Accepted (2026-10-26, E29.1)

## Context

The design-app plugins (E29) and other local programs need a way into the
running app that does not depend on a TCP port and a token file. ADR 0015's
HTTP endpoint needs both: the port changes from run to run, and the bearer
token is what keeps other users of the machine out. An OS-level channel that
only the logged-in user can open needs neither. It must serve the same
contract as the HTTP endpoint, with no font logic of its own, and behave the
same on all three platforms (rule 5).

## Decision

**Crates.** The listener is OS code, so it lives in `fm-platform`
(`fm_platform::ipc`: `IpcListener`, `IpcStream`, `connect`), one file per OS
(`src/windows/ipc.rs`, `src/macos/ipc.rs`, `src/linux/ipc.rs`), on the
`interprocess` crate (2.x, MIT OR Apache-2.0, from the spec's library table).
The message layer is transport, so it lives in `fm-endpoint`
(`fm_endpoint::ipc::IpcEndpoint`), over `Arc<dyn Api>`, and dispatches
through the same method table as the HTTP routes (`routes::dispatch`): a new
service method needs one line in `routes.rs` and is served on both. `fm-app`
starts it with the HTTP endpoint (`AppState::with_ipc`, `start_endpoint`)
and the quit path stops it. Only the real library (`CoreApi`) is served.

**Names** (`AppPaths::ipc_name`):

| OS | Name |
|----|------|
| Windows | named pipe `\\.\pipe\org.fontmanager.app-<user>`; `<user>` is `%USERNAME%`, characters other than letters, digits, `-`, `_` and `.` become `_` |
| macOS | socket `fm.sock` in the data folder (`~/Library/Application Support/org.fontmanager.app/`) |
| Linux | socket `$XDG_RUNTIME_DIR/font-manager/fm.sock`; without an absolute `XDG_RUNTIME_DIR`, `fm.sock` in the data folder |

The names are fixed per user, so a client needs no discovery file.

**Permissions, per OS.**

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Who may connect | a protected DACL `D:P(A;;FA;;;<user SID>)` on every pipe instance, set at creation: the user only, no Administrators or SYSTEM entry (the same as the private files of ADR 0015) | socket mode `0600` in a folder made `0700` before the bind | the same |
| Remote clients | refused: `PIPE_REJECT_REMOTE_CLIENTS` | not reachable (a Unix socket is local) | the same |
| Mode set by | `CreateNamedPipeW` with the descriptor | `chmod` right after `bind()` (macOS ignores a mode set on a socket before it); the `0700` folder keeps others out in between | `fchmod` before `bind()` (interprocess), so the socket is never wider |
| Squatting | the first instance uses `FILE_FLAG_FIRST_PIPE_INSTANCE`: a pipe of that name another program made first is an error, not a server we share | a socket that answers is another app: an error | the same |
| Stale name | none: a pipe goes with its last handle | a socket file nothing answers on (a crashed run) is removed and bound again; a file that is not a socket is never touched (an error) | the same |
| Clean quit | the pipe goes with the listener | the socket file is removed when the listener drops | the same |

`IpcListener::is_private` checks the result (the DACL is exactly that one
entry; the modes are `0600` and `0700`), and tests on the three-OS CI matrix
check it, the stale socket and the cleanup.

**No bearer token.** Unlike the HTTP endpoint, the pipe and the socket need
no token: the OS lets only this user open them, so a client that connects is
already a program of this user, inside the same trust boundary as ADR 0015's
token holder. A web page cannot open a pipe or a Unix socket, so the `Origin`
and `Host` checks have nothing to guard either.

**Messages.** Newline-delimited JSON (UTF-8), one object per line, in both
directions:

- request `{"v":1,"id":<any JSON>,"method":"<service method>","params":<request>}`;
  `method` is the Rust name of an `fm-api` service method, as in
  `POST /v1/<method>`, and `params` is its request value in the UI's
  camelCase wire format; for a method without one, `params` is left out,
  `null` or `{}`.
- reply `{"v":1,"id":<the request's id>,"result":<answer>}` or
  `{"v":1,"id":<the request's id>,"error":<ApiError>}`; `ApiError` keeps its
  JSON shape (`{ code, message, detail }`).
- `hello` (no params) answers `fm_api::API_VERSION`
  (`{"major","minor","patch"}`), so a client learns the contract before it
  calls anything.

A connection carries any number of requests, answered in order. Each
connection has a thread of its own, so a slow client never holds up another.

**Refusals** (the connection stays open after each, and the next line is
served):

| Line | Reply |
|------|-------|
| not JSON, not an object, or longer than 1 MiB (newline not counted; the rest of the line is read and dropped) | `invalidRequest`, `id` `null` |
| `v` missing or not a whole number; `method` missing or not a string; `params` that do not fit the method (`detail` starts `request body: `) | `invalidRequest`, `id` echoed |
| another `v` | `unsupported`, naming both versions |
| a method the table does not have | `unsupported` |
| a service that panics | `internal` |

Blank lines are skipped.

**Versioning.** `v` is the envelope's major version, and this app speaks 1.
The envelope may gain keys within a major: a reader ignores keys it does not
know. Removing or changing the meaning of a key is a new `v`, and a request
with a `v` the app does not speak gets `unsupported`. The contract inside
`params` and `result` follows ADR 0015's rules (additive within an API
major); a client compares `hello`'s major with its own as
`fm_api::check_server_version` does.

## Consequences

- Plugins and scripts reach the app at a fixed name, with no port, no
  discovery file and no token to copy.
- A new service method is served over HTTP and IPC from one line in
  `routes.rs`.
- Events are not on the IPC endpoint yet; a client that needs them uses
  `GET /v1/events` (ADR 0015).
- Two apps of the same user cannot both listen: the second one's IPC
  endpoint does not start (it logs a warning); its HTTP endpoint is not
  affected. A scratch home (`FM_HOME`) uses the same pipe name on Windows
  and the same `$XDG_RUNTIME_DIR` socket on Linux as the real app.
- Deprecation notes (`Fm-Deprecation` over HTTP) are not carried in the
  envelope; a client reads them from the HTTP endpoint if it cares.
