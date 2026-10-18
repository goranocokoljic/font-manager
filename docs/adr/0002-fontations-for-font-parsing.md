# 0002 — fontations for font parsing

Status: Accepted (2026-10-14)

## Context

The app reads every face of 10,000+ files on the first scan in under 10 s, and must
never crash on a malformed font: a bad file is flagged, never fatal. It needs names,
metrics, axes, features, scripts and coverage, outlines for thumbnails, and subsetting
for web export.

## Decision

Parse with Google's [fontations](https://github.com/googlefonts/fontations):
`read-fonts` for tables, `skrifa` for metadata, outlines and variations, and `klippa`
for subsetting. Shaping uses `harfrust` (the HarfBuzz port on `read-fonts`; it replaced
`rustybuzz`, which RustSec marks unmaintained, by human decision on #60); thumbnails
rasterize with `swash` or `cosmic-text`. The parser path has `cargo-fuzz` targets.

## Consequences

- Memory-safe Rust parsing, no C dependency in the parse path, maintained by the team
  behind Google Fonts.
- Malformed data comes back as an error the core turns into `Health::Broken(reason)`;
  no `panic!` and no `unwrap()` on font data.
- Type 1 and other legacy formats fontations does not read are detected and reported
  as unsupported, not parsed.
