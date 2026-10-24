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
