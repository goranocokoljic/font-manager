# Font Manager — guide for coding agents

A free, open-source, cross-platform font manager (Windows, macOS, Linux). Rust core,
Tauri 2 + Vue 3 desktop shell, CLI and MCP server over one typed API. Read this before
changing anything; it is short on purpose.

Specification: https://claude.ai/code/artifact/b90dd55a-4c52-4f44-b868-46878caa6f11
Task breakdown: https://claude.ai/code/artifact/195e5ba6-05c4-4eae-b5a3-092d9361acd5
Decisions: `docs/adr/` — read the ADR that covers the area you are touching.

## Architecture in one paragraph

`fm-core` owns all logic: scanning, parsing (fontations), the SQLite/FTS5 index, search and
filters, coverage, previews, activation, export, catalog adapters. `fm-api` is the only
entrance: typed, versioned request/response types and service traits. Everything else is a
**thin adapter** over `fm-api` — Tauri commands for the UI, `fm-cli` (clap), `fm-mcp`
(rmcp), the plugin IPC endpoint. `fm-platform` is the one place with OS-specific code,
behind the `FontActivator` trait and the app-paths helpers.

## Rules that do not bend

1. **No logic in adapters or the frontend.** If a Vue component or a CLI subcommand needs
   to compute something about fonts, add it to `fm-core` and expose it through `fm-api`.
2. **Never touch user files uninvited.** The library is an index over folders the user
   chose. Moving, renaming or deleting a font file is an explicit user action with undo;
   deletion means the OS trash, never `remove_file`.
3. **Never deregister a font the app did not register.** Activation state is tracked in
   the database; `deactivate` only acts on faces listed there.
4. **No admin elevation, no writes to system font folders.** Session activation registers
   from the file's current path; permanent activation is per-user.
5. **Parity.** A feature ships on all three platforms or not at all. Platform differences
   live in `fm-platform` and are documented in the ADR, never hidden in `cfg` branches
   elsewhere.
6. **`fm-api` is a public contract from v1.0.** Changing a request/response type outside
   epics E1 and E25 needs a human decision — stop and ask.
7. **Offline and private by default.** No network call without a user action; telemetry is
   off unless the config flag is on; no accounts.
8. **Keep the budgets.** Cold start < 1 s, 10k-font first scan < 10 s, search < 50 ms,
   100-face activation < 500 ms, idle memory < 350 MB. A change that makes the bench job
   fail is not done. Known red until E2.8 (#257): the first scan on Windows and macOS; a
   story that does not make that metric slower is not blocked by it.

## Repository map

```
crates/fm-core        logic (scanner, parser, index, search, coverage, preview, activation service, export, catalogs)
crates/fm-api         service traits + types; `specta` exports TypeScript types
crates/fm-platform    FontActivator trait; windows/ macos/ linux/ implementations; app paths; tray
crates/fm-cli         `fm` binary (clap) — direct-link when the app is closed, endpoint when open
crates/fm-mcp         rmcp server (hosted by the app) + stdio shim
apps/desktop          Tauri 2 app: src-tauri/ (fm-app), src/ (Vue 3 + TS + Vite + Pinia)
plugins/              Adobe UXP/CEP, Sketch (Phase 4)
data/charsets         language → required characters (JSON); community-editable; schema-validated
data/classification   heuristics data
fixtures/fonts        200-font test corpus (OFL/Apache/UFL only)
scripts/              build-reference-library.sh, bench.sh, release.sh
docs/adr              architecture decision records
```

## Commands

```
pnpm install && cargo build                 # first build
pnpm lint                                   # clippy -D warnings, rustfmt, cargo-deny, ESLint, Prettier
cargo nextest run --workspace               # Rust tests
pnpm test                                   # Vitest
pnpm test:e2e                               # Playwright (browser mode on MockApi)
pnpm gen:types                              # regenerate apps/desktop/src/api/types.ts from fm-api
pnpm tauri dev                              # run the app
scripts/bench.sh                            # performance numbers against the reference library
scripts/build-reference-library.sh          # download the 10k reference library (once)
```

Before every push: `pnpm lint && cargo nextest run --workspace && pnpm test && pnpm gen:types && git diff --exit-code apps/desktop/src/api/types.ts`.

## Conventions

- Rust edition 2024. Errors are `FmError` with a user-facing message and developer detail;
  no `unwrap()` outside tests; no `panic!` on malformed font data — return
  `Health::Broken(reason)`.
- Every public `fm-api` type derives `Serialize, Deserialize, specta::Type`. Add new
  fields as `Option<T>` or with `#[serde(default)]`; never remove or rename a field.
- Events: library changes, scan progress and activation changes are emitted through the
  typed `Event` enum; the UI subscribes, it does not poll.
- Vue: Composition API with `<script setup lang="ts">`; state in Pinia stores; the font
  list is virtualized — never render all rows. Call the generated API client only; no
  `fetch`, no Tauri `invoke` outside `src/api/`.
- Platform code: one implementation file per OS under `fm-platform/src/{windows,macos,linux}/`,
  each behind the same trait, each with an integration test that proves a second process
  can see the activated font.
- Tests use `fixtures/fonts/` (200 fonts). Performance tests use the reference library and
  are skipped when it is absent (`FM_REFERENCE_LIBRARY` env var points to it).
- Data files in `data/` have a JSON schema next to them and a test that validates every file.
- Branch `E3.2-short-slug`; PR title `E3.2 <story title>`; body `Closes #<issue>`, then what
  changed, how it was verified (commands, CI links, bench numbers, screenshots for UI).
- Commit messages: imperative, one line of what, a body of why if not obvious.

## Working a story

1. Read the issue: acceptance criteria, `fm-meta` block, the epic, the spec section it cites.
2. Find the ADR for the area; if the story would contradict it, stop and ask.
3. Implement in `fm-core` first, expose through `fm-api`, then wire the adapter or UI.
4. Add tests at the layer where the logic lives; UI stories add a Playwright check.
5. Run the pre-push commands. Push. Wait for the three-OS matrix; a Windows-only pass is
   not a pass.
6. In the PR, map each acceptance criterion to its evidence.

## Things that look like shortcuts and are not

- Rendering previews as pre-baked images in the list — the list uses the webview's native
  font rendering through the asset protocol; the core rasterizer is for thumbnails, CLI and MCP.
- Copying fonts into an app-owned folder "to simplify activation" — forbidden by rule 2.
- A `#[cfg(windows)]` block in `fm-core` — belongs in `fm-platform`.
- Reading `localStorage`/`sessionStorage` in the UI for anything but view preferences.
- Adding a crate not listed in the spec's library table without checking `cargo-deny` and asking.

## Harness (go-harness)

Stories are worked by `go-harness.ps1` (one lane per terminal: `-Lane core`, `-Lane ui`)
through the `/go-cycle` skill and reviewed by `/fm-review`. Manual:
`dev-docs/harness/go-harness.md`. With `merge: auto` the harness merges green, reviewed
stories itself, platform stories included; changes to existing `fm-api` types wait for a human.

- Use the PureContext MCP index for code navigation and impact: `resolve_repo` for the
  `repoId`, `get_task_context` to orient, `prepare_change` / `check_consistency` before
  editing shared code or adding `pub` items, `index_file` after every write,
  `verify_change` and `merge_readiness` before handing over. Grep only to prove absence.
- Review findings are `must` (blocks; verified) or `should` (advisory; not auto-fixed).
  Out-of-diff notes go to `dev-docs/harness/parking-lot.md`, never to new issues.
- Graduated review lessons (always loaded):

@dev-docs/harness/review-rules.md
