# Font Manager

A free, open-source, cross-platform font manager for Windows, macOS and Linux. A Rust
core with a Tauri 2 + Vue 3 desktop shell, and a CLI and an MCP server over one typed
API.

Status: Phase 1 (MVP) in progress. Not usable yet.

## Build

Needs Rust (the version in `rust-toolchain.toml`), Node 22+ and pnpm 12, plus the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```
pnpm install && cargo build
pnpm lint            # clippy -D warnings, rustfmt, cargo-deny, ESLint, Prettier, vue-tsc
cargo nextest run --workspace
pnpm test
pnpm gen:types       # regenerate apps/desktop/src/api/types.ts from fm-api
pnpm dev             # run the app
```

`pnpm lint` needs `cargo install --locked cargo-nextest cargo-deny`.

## Dev builds

Every night with new commits on `main`, the `nightly` workflow builds unsigned
installers and attaches them to a GitHub pre-release named `nightly-<version>-<hash>`:
`.msi` and `.exe` for Windows, `.dmg` for macOS, `.AppImage` for Linux. The version is
the `[workspace.package]` version of `Cargo.toml` plus the short commit hash, for example
`font-manager_0.1.0-1a2b3c4_windows_x64.msi`. The installers are not signed: Windows
SmartScreen and macOS Gatekeeper warn before they open. Each nightly has a `SHA256SUMS`
file. Build the installers locally with
`node scripts/release/build.mjs installer --os windows` (or `macos`, `linux`).
[docs/building.md](docs/building.md) lists the pinned toolchain and the exact commands,
says what is reproducible, and tells you how to verify a download.

## Reference library

The performance budgets are measured against ~10,000 OFL, Apache and UFL fonts (the
Google Fonts repository at a pinned commit, the static instances Google Fonts serves for
its variable families, and the OFL families of Fontshare). Download them once:

```
scripts/build-reference-library.sh --dest ~/fm-reference-library    # several GB; resumable
export FM_REFERENCE_LIBRARY=~/fm-reference-library
```

The script writes `manifest.tsv` (sha256, size, licence, source, path) into the folder.
Run it again after an interruption: it continues where it stopped and re-downloads
nothing that is complete. Performance tests are skipped when `FM_REFERENCE_LIBRARY` is
not set. The 200-font test corpus in [fixtures/fonts](fixtures/fonts/) is picked from
this library by `scripts/reference-library/fixtures.mjs`.

## Performance

`scripts/bench.sh` measures the budgets on this machine against the reference library:
cold start, first scan, incremental update, search latency, 100-face activation and idle
memory. It builds `fm-bench` in release, reads the library without changing it, writes a
JSON report (`target/bench/bench-<os>.json`) and exits 1 when a metric is over budget.

```
scripts/bench.sh                         # uses FM_REFERENCE_LIBRARY
scripts/bench.sh --library DIR --out report.json
```

CI runs it nightly on `main` on Windows, macOS and Linux and writes the trend against
the earlier nightly runs into the job summary of the `bench` workflow. A PR gets the
same run when it carries the `bench` label.

Contributors and coding agents: read [CLAUDE.md](CLAUDE.md) first. Decisions are in
[docs/adr](docs/adr/).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
