# 0006 — Shape of the fm-api contract

Status: Accepted (2026-10-16, E1.3)

## Context

`fm-api` is the only way into `fm-core`. Four adapters sit on it: Tauri commands
(in-process), the localhost endpoint for the CLI and plugins, the MCP server, and the
CLI's direct-link mode. The UI is generated from it (`pnpm gen:types`), and from v1.0 it
is a public contract. Its shape has to suit all four and survive additive change.

## Decision

- **One trait per service:** Library, Search, Preview, Activation, Organization,
  Export, Workflows, plus `EventSource`. `Api` is all of them; it has a blanket impl, so
  an adapter holds `Arc<dyn Api>` and does not care whether it talks to the core or to
  `MockApi`. Catalogs (phase 3) arrive as a new trait added to `Api`.
- **One request value in, `ApiResult<T>` out.** A method takes a request struct (or a
  single id) so a field can be added without changing a signature.
- **Synchronous `&self` methods.** They stay object-safe and trivial to mock. Long work
  (a scan) returns a `JobId` at once and reports through `Event`s; async adapters
  (Tauri, rmcp, the endpoint) call the services from `spawn_blocking`.
- **Events, not polling.** `EventSource::subscribe` returns a `std::sync::mpsc`
  receiver; every subscriber sees every event sent after it subscribed.
- **Errors are data.** `ApiError { code, message, detail }`: a stable `ErrorCode` for
  programs, one plain sentence for the user, developer detail on the side. `fm-core`'s
  `FmError` (E1.7) maps into it. Activation never fails all-or-nothing: each face gets
  its own `OutcomeStatus`.
- **Wire format for TypeScript.** Fields are camelCase. Enums with data are tagged
  (`kind`, `status`, or `type` for events). Ids are `u32` newtypes and sizes are `u32`:
  specta refuses 64-bit integers, because JavaScript numbers lose precision above 2^53.
  Timestamps are RFC 3339 strings in UTC. `f32` fields come out as `number | null`,
  because serde_json writes NaN as `null`.
- **The API never prompts.** A choice the user must make (a font conflict) is a request
  field (`ConflictPolicy`); the UI asks, the API obeys.
- **`MockApi`** lives in `fm-api` with no extra dependencies. It implements every trait
  on sample data, behaves like the contract (per-face outcomes, no deactivation of fonts
  the app did not register, the license guard, events) and touches no file and no OS.

## Consequences

- Until v1.0 (`API_VERSION` 0.x), E1 and E25 may reshape types freely; after that, only
  additive changes: new fields are `Option<T>` or `#[serde(default)]`.
- A new type that crosses the API must be added to `fm_api::types()`, or the UI will not
  see it; CI fails when the committed `types.ts` differs from a fresh `pnpm gen:types`.
- The UI can be built against `MockApi` before the core exists.
- A file over 4 GB does not fit `file_size: u32`. Fonts are far smaller; a library-wide
  total will need `f64` or a string if one is ever added.
