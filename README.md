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

Contributors and coding agents: read [CLAUDE.md](CLAUDE.md) first. Decisions are in
[docs/adr](docs/adr/).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
