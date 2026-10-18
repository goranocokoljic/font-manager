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
- [E2.7 #50] apps/desktop/src-tauri/src/lib.rs:220 — `LibraryService::scan` still goes through `MockApi`, so `fm_core::scanner::ScanJob` has no production caller until a core `Api` exists (E3).
- [E2.7 #50] crates/fm-core/src/scanner/changes.rs:271 — `FileIndex::update` reports a gone entry that reappears with an unchanged stamp as `unchanged` (and with a new stamp as `changed`), so `Rescan`, `refresh` and `ScanFinished.added` never count a returned file as added.
- [E2.7 #50] crates/fm-core/src/scanner/watch.rs:507 — the watcher runs its own discover + `FileIndex::rescan` instead of `ScanJob`, so watcher-triggered scans emit no ScanStarted/ScanProgress/ScanFinished and the two scan paths can drift.

## 2026-10-18

- [E3.1 #51] crates/fm-api/src/lib.rs:51 — E2.7 (#230) added `Event::ScanStarted` without bumping `API_VERSION`, against the doc's "bumped on every contract change".
- [E3.1 #51] crates/fm-api/src/mock.rs:751 — `MockApi::remove_source` silently drops active faces from `st.active`, while the new `fm_core::Index::remove_source` refuses with `Conflict` (rule 3); the mock and the core disagree.
- [E4.1 #59] apps/desktop/src-tauri/src/lib.rs:171 — the shell still serves `MockApi`, whose face paths do not exist, so every `fmfont` request returns 404 and every preview row says "Preview not available" in the real app until `fm-core` implements `Api`.
- [E4.1 #59] apps/desktop/src/stores/library.ts:32 — the interim face feed issues one `getFamily` IPC per family on every `libraryChanged`/`scanFinished` (~10k calls on the reference library); E5.2 (#65) must replace it with paged families.
- [E4.1 #59] crates/fm-api/src/preview.rs:55 — `FontSource.path` is a `String`, so a font at a non-UTF-8 path cannot be served by the `fmfont` route exactly.
- [E2.6 #49] crates/fm-core/src/telemetry.rs:137 — a wuff panic caught by `catch_unwind` in `parser::decompress_web` still runs the installed crash hook, so a handled malformed WOFF could be sent as a crash report when crash reports are on.
- [E5.4 #67] apps/desktop/src/components/FontTable.vue:1281 — the variable-range span on the weight line (35% fg over bg) is about 2.2:1 light / 2.9:1 dark, under 3:1; the Variable badge and Weight column carry the same information.
- [E3.4 #54] crates/fm-api/src/mock.rs:592 — the MockApi weight filter checks only the static weight and ignores the `wght` axis, so in browser mode a range like 800–900 leaves variable Inter out while fm-core includes it.
- [E3.5 #55] crates/fm-core/src/index/search.rs:262 — `FamilySummary.classification` is still hard-coded `None`; fill it from `ParsedFace.classification` once E3.4 (#240) merges.
- [E5.10 #73] apps/desktop/src/api/mock.ts:360 — in the TS mock's `activate`, `notFound` thrown mid-map leaves faces already processed active without an `activationChanged` emit (mock only).
- [E5.4 #67] apps/desktop/src/stores/library.ts:86 — main's `refresh` could land after a newer `refreshActivation` and reset activation state; covered by the cycle-3 fetchFamilies refetch (3fb2911), parked for a human to confirm.
- [E3.7 #57] crates/fm-api/src/mock.rs:604 — the MockApi's `has_locl` is hard-coded (sr-Cyrl with 'б'), so its locl answers will differ from fm-core's `script/LANG` matching once coverage is wired to the API.
- [E3.8 #58] crates/fm-api/src/mock.rs:981 — the MockApi ranks supports hits by score minus 0.1 per missing character, so a name-matching near miss can rank above a complete family, while fm-core ranks complete families first.
- [E3.8 #58] crates/fm-core/src/coverage.rs:252 — if the embedded presets fail to load, every language search reports "Unknown language preset" instead of saying the presets are unavailable.
- [E3.8 #58] crates/fm-core/src/index/search/tests.rs:1117 — the 10k search timing test skips timing in debug builds and CI runs debug, so the 50 ms budget is asserted only in local release runs until the E9.1 bench job.
- [E3.8 #58] crates/fm-core/src/index/search.rs:521 — a malformed `loclLanguages` JSON in `face.parsed` is silently read as no locl, without a test.
- [E3.8 #58] crates/fm-api/src/mock.rs:54 — the MockApi keeps its own `NEAR_MISS = 5` apart from `fm_core::coverage::NEAR_MISS`; the crate boundary forces the copy.
- [E9.1 #89] scripts/harness/ci-wait.mjs:31 — the `\bjob\b` regex fallback in `matchCheck` would match `measure (windows)` for `ci / windows` if the exact `windows` check were missing; the bench workflow avoids it only by its job names.
- [E9.1 #89] scripts/harness/epic-sync.mjs:20 — prints `null` for not-measured bench metrics (`activation_100`, `<os>.report`) because it formats `m.value` without a null check.
