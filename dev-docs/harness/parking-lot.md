# Parking lot

Out-of-diff review observations that go-harness parked instead of turning them
into issues. A human triages this list in batches: make a story, fold into an
existing story, or delete the line.

Format: `- [<story id> #<issue>] path:line — <one sentence>` under a date heading.

## 2026-10-16

- [E1.7 #43] crates/fm-core/src/config.rs:22 — E1.6 (PR #213) `Config.telemetry: bool` and `io_error` (config.rs:96, always `ErrorCode::Io`) overlap E1.7 `TelemetrySettings` and `FmError::io`; whichever PR merges second should switch to `TelemetrySettings` and `FmError::io`/`FmResult`.
- [E1.7 #43] crates/fm-core/src/error.rs:51 — `FmError` constructors and `Display` mirror `ApiError` (crates/fm-api/src/error.rs:49) line for line; a message change on one side must be copied to the other.

## 2026-10-17

- [E2.1 #44] scripts/reference-library/lib.mjs:44 — its own font-extension regex omits `dfont`, so reference-library counts can diverge from `fm_core::scanner::FONT_EXTENSIONS` if `.dfont` files are added.
- [E5.6 #69] crates/fm-api/src/mock.rs:1032 — `st.face(id)?` inside the mock activate loop fails the whole batch on one unknown face id instead of one outcome per face; apps/desktop/src/api/mock.ts mirrors it.
- [E5.6 #69] apps/desktop/src/components/MainLayout.vue:64 — the inspector is collapsed by default below 1,100 px (E5.1); it opens only through the "Inspector" toggle.
- [E2.2 #45] crates/fm-core/fuzz/Cargo.toml:24 — the fuzz crate is outside the workspace and no CI job builds it, so the `parse` target can stop compiling unnoticed until E9.4 wires the weekly run.
- [E2.3 #46] crates/fm-core/src/scanner/discovery.rs:179 — a file reachable by a plain path and a link is listed under whichever path the walk reaches first, so a new link or a changed walk order makes `FileIndex::rescan` report the old path gone and the new one added for the same file.
- [E2.4 #47] crates/fm-api/src/mock.rs:827 — MockApi `get_family` sorts faces on `slant != Upright`, so Italic and Oblique tie, looser than the `Family` doc ("then slant") and the core order Upright < Italic < Oblique in `fm_core::grouping`.
