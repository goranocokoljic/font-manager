# Contributing to Font Manager

Thank you for helping. This guide tells you how to build the project, which rules every
change must keep, and how to send a pull request. Please also read the
[Code of Conduct](CODE_OF_CONDUCT.md).

Coding agents and people who want the short technical version: read [CLAUDE.md](CLAUDE.md).
Design decisions are in [docs/adr](docs/adr/README.md). Read the decision record (ADR) for
the area you change before you change it.

## Report a bug or ask for a feature

Open an issue from one of the forms: bug report, feature request or missing language
preset. A bug report needs your OS, the app version and the steps. Attach the log file:
**Settings → Advanced → Logs folder** opens the folder.

Do not report a security problem in a public issue. Use the repository's private reporting
channel on GitHub (Security → Report a vulnerability).

## Build

You need Rust (the version in [rust-toolchain.toml](rust-toolchain.toml)), Node 22 or newer,
pnpm 12 and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.
`pnpm lint` also needs `cargo install --locked cargo-nextest cargo-deny`.

```
pnpm install && cargo build                 # first build
pnpm dev                                    # run the app
pnpm lint                                   # clippy -D warnings, rustfmt, cargo-deny, ESLint, Prettier, community files
cargo nextest run --workspace               # Rust tests
pnpm test                                   # Vitest and the Node script tests
pnpm test:e2e                               # Playwright (browser mode on a mock API)
pnpm gen:types                              # regenerate apps/desktop/src/api/types.ts from fm-api
```

[docs/building.md](docs/building.md) has the pinned toolchain and the installer builds.

Before every push, run all of these. They must pass:

```
pnpm lint && cargo nextest run --workspace && pnpm test && pnpm gen:types && git diff --exit-code apps/desktop/src/api/types.ts
```

The last command fails when the TypeScript types in the frontend do not match the Rust
API. Commit the regenerated file.

## Rules that do not bend

A pull request that breaks one of these rules is not merged.

1. **Logic lives in the Rust core.** `fm-core` does all the work on fonts. `fm-api` is the
   only way in. The desktop UI, the CLI, the MCP server and the plugins only pass requests
   through. If the UI needs to compute something about fonts, add it to the core.
2. **Never touch the user's files without being asked.** The library is an index over
   folders the user chose. Font Manager moves, renames or deletes a font file only when
   the user asks, and the user can undo it. Deleting means the OS trash, never a
   permanent delete.
3. **Never turn off a font the app did not turn on.** The database records which fonts
   the app activated. Deactivation only acts on those.
4. **No administrator rights, no writes to system font folders.** Activation for the
   session uses the file where it is. Permanent activation is per user.
5. **All three systems or none.** A feature ships on Windows, macOS and Linux, or it does
   not ship. OS-specific code lives in `fm-platform` and is described in an ADR.
6. **The API is a public contract.** Do not rename, remove or change the type of a field
   in an `fm-api` request or response. Add new fields as optional. Ask first if you think
   a change is necessary.
7. **Offline and private.** No network call unless the user asks for it. Telemetry is off
   unless the user turns it on. No accounts.
8. **Keep it fast.** Cold start under 1 s, first scan of 10,000 fonts under 10 s, search
   under 50 ms, activation of 100 faces under 500 ms (1 s on macOS), idle memory under
   350 MB. A change that makes the bench job fail is not done.

Also:

- Do not add a Rust crate or an npm package without asking in the issue first. It must
  pass `cargo deny check`.
- No `unwrap()` outside tests. A broken font file is reported as broken; it must never
  crash the app.
- Tests go with the code, in the same commit, at the layer where the logic lives. UI
  changes get a Playwright check.

## Branches, commits and pull requests

- **Branch:** `<story id>-<short-slug>`, for example `E3.2-fts-index`. Without a story,
  use a short slug, for example `fix-empty-folder-scan`.
- **Commit message:** imperative mood, one line of what changed (`Add the Thai preset`).
  Add a body that says why, if it is not obvious.
- **Pull request title:** `<story id> <story title>`, for example `E3.2 FTS index`.
- **Pull request body:** `Closes #<issue>`, then what changed and how you verified it:
  the commands, test names, bench numbers, and screenshots for UI changes. The
  [pull request template](.github/pull_request_template.md) has the sections.
- CI runs on Windows, macOS and Linux. A pull request is green only when all three are.

## Add a language preset

The "supports language" filter and the coverage report use the presets in
[data/charsets](data/charsets/README.md). Each file is JSON and follows
[charset.schema.json](data/charsets/charset.schema.json).

1. Find the file for the script (`latin.json`, `cyrillic.json`, …). Add an entry to its
   `languages` list: `code` (BCP 47), `name`, `script` (ISO 15924), `otLanguage`, `base`,
   `auxiliary` and `source`. The [README](data/charsets/README.md) explains each field.
2. A new script needs a new file. Add it to `FILES` in `crates/fm-core/src/charsets.rs`
   too: the data is built into the app.
3. Run `cargo nextest run -p fm-core charsets`. It checks the schema, duplicates, and
   that every character is in its script's Unicode blocks.
4. Open a pull request with only that data change. Name your source in the description.

## Add or change another data file

Every file in [data/](data/) has a JSON schema next to it (`*.schema.json`) and a Rust
test in `fm-core` that validates every file against it. When you change a data file,
run that test. When you add a new kind of data file:

1. Write the schema first, next to the data, and point the data file's `$schema` at it.
2. Add a `README.md` to the folder: what the data is for, the format, the sources and
   their licences.
3. Add the test in `fm-core` that loads every file and checks it against the schema.

Only add data you are allowed to share under the project license. Name the source.

## License

Font Manager is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option. Unless you say otherwise, every contribution you submit is dual-licensed
under the same terms, without any additional terms or conditions. Fonts in the test
corpus are OFL, Apache or UFL only.
