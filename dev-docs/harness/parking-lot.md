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
- [E6.2 #75] .harness/notes/issue-75.md:1 — issue #76 (E6.3) criterion 1 still names `AddFontResourceExW`; ADR 0008 replaced it with the per-user HKCU value, so the criterion needs a human edit before E6.3 starts.
- [E5.10 #73] apps/desktop/src/api/mock.ts:360 — in the TS mock's `activate`, `notFound` thrown mid-map leaves faces already processed active without an `activationChanged` emit (mock only).
- [E5.4 #67] apps/desktop/src/stores/library.ts:86 — main's `refresh` could land after a newer `refreshActivation` and reset activation state; covered by the cycle-3 fetchFamilies refetch (3fb2911), parked for a human to confirm.
- [E3.7 #57] crates/fm-api/src/mock.rs:604 — the MockApi's `has_locl` is hard-coded (sr-Cyrl with 'б'), so its locl answers will differ from fm-core's `script/LANG` matching once coverage is wired to the API.
- [E4.5 #63] apps/desktop/src-tauri/src/lib.rs:322 — the shell still serves MockApi with no fm-core-backed PreviewService, so the app shows the mock's fallback rule, not `fm_core::sample`, and the criteria cannot be shown on the reference library in the app yet.
- [E4.5 #63] apps/desktop/src/list/rows.ts:82 — the treeitem `aria-label` hides the row's badges (Variable, and now Fallback) from screen readers.
- [E3.8 #58] crates/fm-api/src/mock.rs:981 — the MockApi ranks supports hits by score minus 0.1 per missing character, so a name-matching near miss can rank above a complete family, while fm-core ranks complete families first.
- [E3.8 #58] crates/fm-core/src/coverage.rs:252 — if the embedded presets fail to load, every language search reports "Unknown language preset" instead of saying the presets are unavailable.
- [E3.8 #58] crates/fm-core/src/index/search/tests.rs:1117 — the 10k search timing test skips timing in debug builds and CI runs debug, so the 50 ms budget is asserted only in local release runs until the E9.1 bench job.
- [E3.8 #58] crates/fm-core/src/index/search.rs:521 — a malformed `loclLanguages` JSON in `face.parsed` is silently read as no locl, without a test.
- [E3.8 #58] crates/fm-api/src/mock.rs:54 — the MockApi keeps its own `NEAR_MISS = 5` apart from `fm_core::coverage::NEAR_MISS`; the crate boundary forces the copy.
- [E9.1 #89] scripts/harness/ci-wait.mjs:31 — the `\bjob\b` regex fallback in `matchCheck` would match `measure (windows)` for `ci / windows` if the exact `windows` check were missing; the bench workflow avoids it only by its job names.
- [E9.1 #89] scripts/harness/epic-sync.mjs:20 — prints `null` for not-measured bench metrics (`activation_100`, `<os>.report`) because it formats `m.value` without a null check.
- [E7.1 #82] apps/desktop/src-tauri/src/lib.rs:36 — the shell still serves `MockApi`, so the fm-core collection methods (`Index::*_collection`) are not reachable from the app; the DoD "demonstrated on the reference library" waits for the core `Api` impl.
- [E7.1 #82] crates/fm-api/src/mock.rs:691 — the MockApi's `collection_view` counts members of gone/paused sources and evaluates smart rules, while fm-core counts visible faces only and gives smart collections 0.
- [E7.2 #83] crates/fm-core/src/index/collections.rs:252 — the inline "every face exists" loop duplicates the new `ensure_faces` in `index/tags.rs`; the two can drift.
- [E7.2 #83] apps/desktop/src/stores/search.test.ts:148 — the "10,000 result families within 50 ms" timing test failed 3 of 6 local full-suite Vitest runs (54–65 ms) and passes alone; its search is a stub, so it measures machine load.
- [E7.3 #84] crates/fm-core/src/index/sources.rs:70 — `add_source` accepts OS font folders (incl. the per-user folder permanent activation targets) as ordinary watched sources; nothing assigns `SourceKind::System` outside mocks, so sidecars can be written into such a folder.
- [E7.3 #84] crates/fm-api/src/library.rs:627 — `list_problems` exists only in the mock; `Index::sidecar_problems()` reaches the Problems view only once E8.2 aggregates it.
- [E8.1 #86] apps/desktop/src-tauri/src/lib.rs:36 — the only `LibraryService` implementation is `MockApi`, so `Index::duplicates` (like `list_problems`) is not reachable through a real fm-api service until the core `Api` impl lands.
- [E8.1 #86] apps/desktop/e2e/search.spec.ts:127 — the "80 ms debounce plus under 50 ms" e2e timing check failed twice on the macOS runner (263.9 ms, 188.4 ms vs < 164 ms) with no code change since a green run; it measures runner load in browser mode, not fm-core search.
- [E9.3 #91] apps/desktop/src-tauri/tauri.conf.json:22 — `bundle.windows.wix` is at the Tauri default, a per-machine MSI that asks for UAC at install time (NSIS is currentUser); a packaging story could record the MSI install scope in an ADR.
- [E9.3 #91] .github/workflows/nightly.yml:85 — macos-latest builds arm64 only, so Intel Macs get no nightly; the criteria do not ask for per-arch builds.
- [E2.8 #257] crates/fm-core/src/scanner/job.rs:113 — the `fm-scan` thread holds the `FileIndex` mutex for the whole scan, so a long first scan blocks every index reader.
- [E2.8 #257] scripts/bench/lib.mjs:138 — the trend's Previous/Change columns compare the per-10,000-files `first_scan` with older raw-ms nightly values until those runs leave the history window.
- [E9.2 #90] scripts/bench/bench.mjs:173 — before E9.2 the nightly trend table listed only successful nightly runs, so it had no history while the first scan is known red (fixed in E9.2 by `nightlyRunsArgs`; the E9.1 trend summaries already published lacked the comparison).
- [E4.3 #61] crates/fm-api/src/settings.rs:68 — `Settings::default().cache_limit_mb` is 1,024 MB while E4.3 criterion 1 says the preview cache defaults to 500 MB; once the cache is wired via `limit_from_mb`, the user-facing default differs (an fm-api default change needs a human).
- [E4.3 #61] crates/fm-core/src/config.rs:92 — four private temp-then-rename writers now exist (config.rs, index/sources.rs:218, index/sidecars.rs:1005, preview/cache.rs); a shared fs helper is a refactor for a human.

## 2026-10-19
- [E6.3 #76] issue #76 / docs/adr/0008-font-activation.md — criterion 1's text ("AddFontResourceExW plus WM_FONTCHANGE") contradicts accepted ADR 0008 (HKCU value for both modes); the issue text could be updated.
- [E6.3 #76] crates/fm-platform/src/activator/mock.rs:93 — `MockActivator::activate_one` answers AlreadyActive on path alone, so a Permanent request for a session-active file keeps the Session registration (same in WindowsActivator); E6.6 should decide the upgrade rule.
- [E6.3 #76] crates/fm-platform/tests/windows_activation.rs:1350 — scratch registry fixtures delete only their own subkey, leaving an empty `HKCU\Software\FontManager-test` on machines that run the tests.
- [E6.3 #76] crates/fm-core/src/parser.rs:383 — name strings keep embedded control characters (NUL included), which flow into the index and the UI.
- [E6.4 #77] crates/fm-platform/src/activator/mod.rs:183 — activate(Permanent) on a file already active in Session returns AlreadyActive(Session) on macOS too (as on Windows), so clear_session drops it at quit; E6.6 should decide the upgrade rule.
- [E6.5 #78] crates/fm-platform/src/activator/mod.rs:183 — nothing in the app calls `clear_session` on quit or at start yet, on any OS; the activation-service story must wire it.
- [E6.5 #78] apps/desktop/e2e/search.spec.ts:127 — the e2e search timing check failed again on the macOS runner (201.6 ms, 168.3 ms vs < 164 ms) on a commit that changed only this file; the CI run before it was green on all three OS.
- [E9.4 #92] apps/desktop/src-tauri/src/lib.rs:434 — the app still serves `MockApi`; no fm-core `LibraryService::list_problems` calls `Index::problems` yet, so the new entries are not visible in the app's Problems view.
- [E9.4 #92] crates/fm-cli/src/main.rs:5 — `fm` does not call `startup::init`, so the crash log hook covers the app but not the CLI.
- [E9.4 #92] crates/fm-core/src/index/sidecars.rs:294 — sidecar sync skips system, paused and missing sources while `missing_source` lists paused ones too; the two filters may drift.
- [E9.4 #92] crates/fm-platform/src/activator/mod.rs:183 — nothing calls `clear_session` at start, so ADR 0008's "the next start removes session links after a crash" is not wired (activation-service story).
- [E9.4 #92] crates/fm-core/src/crash.rs:205 — pruning to 10 crash logs counts unreported ones too, so more than 10 crashes before a start drop the oldest unread logs (app-owned files only).
- [E9.5 #93] apps/desktop/src/views/HomeView.vue:78 — the view-mode actions are registered as `view.list`/`view.table` while KEYMAP binds the digits to `view.mode1..5`, so the `1`–`5` shortcuts of the spec's keyboard model do nothing.
- [E9.5 #93] apps/desktop/src/stores/commands.ts:57 — `void action.run()` drops rejected promises, so a failed palette action is an unhandled rejection the user never sees.
- [E6.6 #79] crates/fm-platform/src/windows/activator.rs:169 — `live.iter().find(|r| r.path == face.path)` compares paths case-sensitively, so the same file through different casing gets a second registry value instead of `AlreadyActive`.
- [E6.6 #79] crates/fm-platform/src/linux/activator.rs:56 — `fonts_dir(paths.data)` derives the link folder from the app data folder, so `fm-bench activate` with a throwaway home writes links fontconfig never scans, while the Windows and macOS benches register real fonts.
- [E6.6 #79] crates/fm-platform/src/macos/coretext.rs:273 — `batch_call` returns every path `Ok` when `CFIndex::try_from` or `CFArrayCreate` fails, without calling CoreText, so those faces are reported Activated without being registered.
- [E5.3 #66] apps/desktop/src-tauri/src/lib.rs:43 — the shell serves `MockApi`, whose `render_preview` returns a 1×1 PNG, so grid thumbnails in the real app stay empty until fm-core implements `Api`.
- [E5.3 #66] apps/desktop/src/components/FontList.vue:229 — the list's "Styles not available" branch (a family whose faces fail to load) has no test.
- [E5.3 #66] apps/desktop/src/components/FontList.vue:763 — the list's row height changes with the preview size without re-anchoring, so another family moves to the top.
- [E6.7 #80] crates/fm-platform/src/activator/mock.rs:132 — `MockActivator` cannot remove `installed` fonts, so a "never deregister a foreign font" test cannot observe the app deregistering one.
- [E6.7 #80] crates/fm-core/src/activation.rs:600 — `clashes()` turns every non-app clash into `ConflictWith::System`, also for `ConflictKind::User`, so the prompt and the Conflicts view call a user-installed font a system font.
- [E8.2 #87] crates/fm-api/src/mock.rs:968 — MockApi lists a SystemConflict problem for every face with a system clash, active or not, while fm-core's `ActivationManager::problems` lists only active faces.
- [E8.2 #87] crates/fm-core/src/activation.rs:545 — `list_conflicts` fails the whole call (NotFound from `activation_faces`) when an active row's face is missing from the index; `ActivationManager::problems` inherits it.
- [E7.4 #85] crates/fm-core/src/scanner/changes.rs:42 — `FileHash` formats hex but cannot parse it, so callers use `blake3::Hash::from_hex` directly and the hash format lives in two places.

## 2026-10-20

- [E5.11 #285] crates/fm-core/src/index/search.rs:93 — the real API leaves `FamilySummary.classification` empty (the scan stores `ClassGuess` in `face.parsed` but search does not read it), so the Class column and the classification filter counts are empty where the MockApi-based checks showed serif/sans.
- [E5.11 #285] crates/fm-core/src/index/faces.rs:150 — `FaceDetail.scripts` is always empty (the `face.scripts` column is never written by the scan), so the Info tab's Scripts row shows "—" on real data while the mock lists `latn`, `cyrl`.
- [E5.11 #285] crates/fm-core/src/index/faces.rs:140 — `FeatureInfo.name` is always `None` (no feature-name table in fm-core), so the Info tab lists bare tags where the mock shows "liga (Standard ligatures)".
- [E5.11 #285] crates/fm-core/src/api.rs:330 — `find_similar`, `export_web`, `license_inventory`, `pick_fonts_for_brief` and `prepare_for_web` answer `Unsupported` on the real API; the MockApi answers them, and no phase-1 UI calls them yet.
- [E5.11 #285] crates/fm-core/src/api.rs:120 — scans write the whole rescan while holding the index lock, so UI reads wait for the write of a large first scan (seconds on 10k fonts).
- [E5.11 #285] crates/fm-core/src/api.rs:151 — if the settings file becomes unreadable between `startup::init` and `CoreApi::start`, `start` returns early at `get_settings()?` and skips the crash-session cleanup for that run.
