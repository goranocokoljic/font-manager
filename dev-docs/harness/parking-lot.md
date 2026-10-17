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
- [E5.8 #71] apps/desktop/src/stores/selection.ts:89 — an in-flight activation outcome is written onto a newly selected face; the new A shortcut makes it easier to hit.
- [E2.3 #46] crates/fm-core/src/scanner/discovery.rs:179 — a file reachable by a plain path and a link is listed under whichever path the walk reaches first, so a new link or a changed walk order makes `FileIndex::rescan` report the old path gone and the new one added for the same file.
- [E2.4 #47] crates/fm-api/src/mock.rs:827 — MockApi `get_family` sorts faces on `slant != Upright`, so Italic and Oblique tie, looser than the `Family` doc ("then slant") and the core order Upright < Italic < Oblique in `fm_core::grouping`.
- [E5.9 #72] crates/fm-api/src/mock.rs:729 — `remove_source` forgets a source's active faces without a deactivation step or outcome, so the real core must deactivate them first; the Settings window now exposes this path.
- [E2.5 #48] crates/fm-core/src/scanner/watch.rs:795 — `rescan_soon` (notify "need rescan" flag, or an event error with no paths) is not exercised by any test, so recovery after an event-queue overflow is untested.
- [E2.5 #48] crates/fm-core/src/scanner/changes.rs:172 — `FileIndex::refresh` never takes the non-NotFound I/O error branch (permission denied) in a test, as the earlier `rescan` tests do not either.
- [E2.5 #48] crates/fm-core/src/scanner/discovery.rs:141 — which of two paths to the same file or folder discovery keeps depends on read_dir order, so the surviving entry is not stable across platforms.
- [E2.5 #48] crates/fm-core/src/scanner/watch.rs:1 — on Windows notify's ReadDirectoryChangesW keeps a handle open on each watched root, so a watched source folder cannot be renamed or removed (or its drive cleanly ejected) while the app runs; macOS and Linux differ and no ADR documents it.
- [E2.5 #48] crates/fm-core/src/scanner/watch.rs:146 — `Worker::unwatch` detects nested sources by the paths as added, not their canonical paths, so two sources nested only through a symbolic link can still drop each other's inotify watches on Linux.
- [E3.6 #56] crates/fm-api/src/mock.rs:71 — the MockApi `LANGUAGES` table (4 presets, lowercase only) differs from `data/charsets`, so browser-mode coverage counts will not match the real app once E3.7 wires coverage.
- [E5.7 #70] crates/fm-api/src/mock.rs:748 — `MockApi::remove_source` drops `active` entries for the removed faces without deactivating them; recheck rule 3 when fm-core implements `Api`.
- [E5.9 #72] crates/fm-core/src/scanner/watch.rs:61 — `WatchSettings::from_config` reads the rescan interval only when the watcher is built and no production code starts a `Watcher` yet, so a Settings change will need a reload path once it is wired.
- [E5.9 #72] crates/fm-core/src/config.rs:3 — an unknown enum value in the flattened settings (e.g. `"theme": "sepia"` from a newer build) fails the whole `Config::load`, log level and telemetry included, against the module doc's forward-compatibility claim.
