# 0014 — Online catalogs: adapter interface, Google Fonts endpoint, metadata cache

Status: Accepted (2026-10-24, E22.1)

## Context

Epic E22 lets the user browse and download free fonts from online catalogs (Google
Fonts first, Fontshare later). This is the app's first network feature besides the
updater (ADR 0012). Rule 7 says no network call without a user action, and the browse
view has to work offline. Each catalog has its own endpoints and JSON shape, so the
part that differs per catalog needs one narrow seam.

## Decision

- **Adapter interface.** `fm_core::catalog::CatalogAdapter`, one per catalog:
  `id()`, `homepage()`, `fetch_index(&dyn Fetcher)`, `parse_index(&[u8])` into
  `fm_api::catalog::CatalogFamily` values, and `resolve_files(&dyn Fetcher, family)`
  (the font files' URLs and the license file shipped with them, for E22.3). Parsing
  is a pure function over bytes, so it is tested on recorded answers.
- **One fetcher.** Every request goes through the `Fetcher` trait (`get(url) -> bytes`).
  The app uses `HttpFetcher`: `reqwest` 0.13, blocking, rustls (approved by the owner
  for E22.1), a 15 s connect and 120 s total timeout, answers over 64 MB refused, the
  client made on the first request so a start costs nothing. Tests pass a local server
  on 127.0.0.1 or a fetcher that fails on any call; no test reaches the internet.
- **Off by default.** `fm_api::CatalogService` (part of `Api`): `list_catalogs`,
  `set_catalog_enabled`, `refresh_catalog`, `list_catalog_families`. Only
  `refresh_catalog` (and later the download of a family) makes a request, and it is
  refused with `Conflict` before any request while the catalog is off. Scanning,
  search and listing never fetch. An off catalog lists no families; turning it off
  keeps its cache.
- **Google Fonts endpoints.** The index is `GET https://fonts.google.com/metadata/fonts`
  (the JSON the site itself reads; about 3 MB, 1,950 families in October 2026; no API
  key). It gives the family, category, styles (`"400"`, `"700i"`), axes, subsets,
  designers, `lastModified` and `isOpenSource`, plus an axis registry with axis names.
  It names no license per family: the cache stores `openSource`, and the exact license
  comes from the files, `GET https://fonts.google.com/download/list?family=<name>`
  (`OFL.txt`, `LICENSE.txt` = Apache 2.0, `UFL.txt`). Both answers may start with
  Google's `)]}'` guard, which is skipped. The `menu` subset (Google's subset for
  drawing the family's own name) is dropped. The base URL is configurable
  (`GoogleFonts::with_base_url`) for tests.
- **Fontshare endpoint (E22.5).** The index is `GET https://api.fontshare.com/v2/fonts`
  (public JSON, no key, no account; about 830 KB, 100 families in October 2026, one
  answer). It gives the family, category, script, designers, version, `license_type`
  (`itf_ffl`, `sil_ofl`), axes and styles; each style names its weight, italic, variable
  and its CDN file (`//cdn.fontshare.com/wf/...`, served as TrueType at that URL plus
  `.ttf`). `CatalogFamily.licenseName` (added with E22.5) carries the license's name,
  because the ITF Free Font License is not an `OpenLicense`: such a family has
  `license: None` and `openSource: false`. Variable fonts give axes, not styles; an axis
  listed twice (upright and italic variable font) is kept once. An answer that says it
  is only part of the index (`has_more`, or `count_total` above the families sent) is an
  error, so the cache stays. There is no per-family file list by name, so
  `resolve_files` reads the index again and names the files like Fontshare's own
  download (`GeneralSans-BoldItalic.ttf`). Base URL configurable
  (`Fontshare::with_base_url`) for tests.
- **Cache location.** `<data>/catalogs/` in the app data folder (ADR 0007):
  `<catalog>.json` (`google-fonts.json`) holds `{ format, refreshedAt, families }`,
  `refreshedAt` in RFC 3339 UTC; `catalogs.json` holds which catalogs are on. A refresh
  parses the whole answer first and then replaces the cache through a temporary file,
  so a failed, refused or malformed answer leaves the previous cache as it was. An
  index entry that cannot be read is left out with a warning; an index with no
  readable family is an error. A damaged cache is an error that asks for a refresh; an
  unreadable on/off file means every catalog off.
- **Previews (E22.2).** `CatalogService::catalog_font_source` fetches one file of a
  family in the cached list (the upright Regular, else an upright variable file, else
  any upright file; TTF, OTF, WOFF or WOFF2, checked to be a font) into
  `<cache>/catalogs/<catalog>/<blake3 of the name>.<ext>` in the app cache folder and
  reads it from there after. It is refused with `Conflict` while the catalog is off and
  `NotFound` for a family the list does not have, both before any request. The webview
  loads it as live text through the face loader on the app's font scheme
  (`fmfont`, path `/catalog-<catalog id>-<the name's UTF-8 as hex>`), so a request is
  made only when a row of an open catalog is on screen. Turning a never-refreshed
  catalog on fetches its list at once; the switch says it connects to the internet.
- **Downloads (E22.3).** `CatalogService::download_family` (catalog, family, static or
  variable, a source) and `install_family` (below) are the only calls that write into the
  library, and only on the user's "Download…" or "Install". The target must be a `watched` source (never the system fonts or
  the library folder, rule 4); the files go into `<source>/<family>/`, the names made
  safe on every OS (`fm_core::catalog::safe_file_name`; a catalog's `static/` prefix or
  `../` never leaves the folder). Static vs variable: Google Fonts' file name names its
  axes (`Inter[opsz,wght].ttf`), Fontshare's style says `is_variable`; a family without
  the kind asked for gets the other kind, and the answer says which. Every file is
  fetched and checked to be a font before any is written, so a failed request writes
  nothing; each is written to `.<name>.<pid>-<n>.part` beside it and hard-linked into
  place (renamed after a check where the file system has no hard links), so an existing
  file is never replaced: it is reported in `skipped`. The source is then rescanned (the
  returned `JobId`; the usual scan events bring the faces), so no new `Event` exists.
- **Provenance.** Index tables `catalog_download` and `catalog_download_file` (schema
  17): catalog, its family id (Google Fonts: the name, its only key; Fontshare: the
  slug), version (Google Fonts gives none: its `lastModified` date), kind, date, source,
  folder, and each file's name, path and blake3 hash. No foreign key to `font_file`, so
  a rescan or rebuild keeps them; `list_catalog_downloads` finds each file's current
  path by its hash among the files the library has (not `gone`), so a moved file is
  followed.
- **Update check (E22.4).** `CatalogService::check_updates` runs only on the user's
  "Check for updates" (and, with `cachedOnly`, right after a catalog Refresh, making no
  request); nothing checks on start or on a timer. It refreshes each catalog that is on
  and has downloads, then compares the version recorded at each family's newest download
  with the cached list's version (else its `lastModified`), by family name. Runs of digits
  compare as numbers, left to right (`fm_core::catalog::is_newer_version`), so dates and
  dotted versions both work; only a newer one is an update. A family the list no longer
  has is `noLongerListed`, not an error. An off catalog is skipped. The UI badges the
  catalog row and the library family of the same name.
- **Install (E22.7).** `CatalogService::install_family` (catalog, family, kind) is
  "Download…" into one folder, then activation, in one call on the user's "Install". The
  folder is `Settings::download_folder`: the one folder the app writes into without a
  choice per download. The user chooses it once (rule 2): the first Install proposes
  `<Documents>/Font Manager Fonts` and makes it only when the user takes it (or picks
  another with the native picker); Settings → Online catalogs changes it. It is never a
  system font folder (`fm_platform::paths::system_font_folders`, or a `system` source):
  that is `Conflict` before any request (rule 4), as are no folder and an off catalog.
  The folder becomes a `watched` source, indexed in place, when no watched source holds
  it (one inside a watched source uses that source, so no file is indexed twice). The
  call writes `<folder>/<family>/` as a download does, records the provenance, rescans
  the source on the calling thread and returns when the index holds the faces; then it
  activates the faces of the files written in `Settings::activation_mode` with
  `Settings::on_conflict`, sending only faces not active yet (a permanent face sent again
  would turn into a session one). A family already in the library (a recorded download
  whose files the library still has, else faces in `<folder>/<family>/`) is not
  downloaded again: its faces are activated and the answer says `alreadyInstalled`.
- **Fetch on demand (E29.5).** `Settings::fetch_missing_fonts` (off by default; Settings
  → Online catalogs, under the download folder, disabled while there is no download
  folder or no catalog is on) lets `fonts_needed` install a font a document needs and
  the library does not have. It keeps rule 7: the user turns the switch on, and only
  catalogs the user turned on are used. Only `fonts_needed` with `activate: true`
  fetches (a design app opened a document); `activate: false` and
  `check_document_fonts` never do. The lookup is cache-only: each unmatched font's
  `family` (else the family part of its full name: the longest cached family name it
  starts with, followed by a space or nothing) is compared with the cached list of each
  catalog that is on, ignoring case, so the lookup makes no request and a catalog never
  refreshed finds nothing. A hit runs the install path without its activation (static
  files into `download_folder`, provenance, rescan); `fonts_needed` then matches again
  and session-activates the new faces itself, tagged with the document (E29.3). Each
  family is fetched once per request; a failure is logged and leaves its fonts
  `unmatched`, never an error for the others. All requests of one `fonts_needed` end
  within 60 s (a request still running then is abandoned and its answer dropped, so
  nothing it fetched is written). The answer lists what was downloaded in `fetched`, so
  the plugins can say "Downloaded 1 family from Google Fonts: Alegreya".

## Consequences

- A new catalog (E22.5 Fontshare) is a new `CatalogId` variant and one adapter; the API
  and the cache do not change.
- `refresh_catalog` is synchronous (ADR 0006): one request of a few seconds; the async
  adapters call it from `spawn_blocking`.
- `fonts.google.com/metadata/fonts` is not a documented public API; if Google changes
  it, the parse fails, the error says so and the previous cache stays. The fixture under
  `fixtures/catalogs/` is re-recorded then.
- `cargo-deny` allows `CDLA-Permissive-2.0` for the one crate `webpki-root-certs`
  (Mozilla's CA certificates as data), which `rustls-platform-verifier` uses only on
  wasm32; no shipping target compiles it.
