//! Web font subsets (E20.2): which characters an export keeps, and the cut.
//!
//! - **Characters**: [`code_points`] turns a request's [`Subset`] into code
//!   points: named presets from `data/subsets/presets.json` (Google Fonts'
//!   `unicode-range` splits), CSS `unicode-range` values (`U+0000-00FF`,
//!   `U+4??`), or every character of a text plus U+0020. A bad request is
//!   `InvalidRequest` with a message for the user, before anything is read.
//! - **Cut**: [`cut`] subsets a standalone font with `skera` (fontations'
//!   subsetter, formerly `klippa`). The new `cmap` maps exactly the requested
//!   characters the face maps. Glyphs that layout rules reach from them (the
//!   `ffi` ligature of `f` and `i`) are kept with every layout feature,
//!   script, name record and glyph name; a variable font keeps its axes.
//!   A face that maps none of the characters is `InvalidRequest`.
//! - **License**: a face whose OS/2 `fsType` has the no-subsetting bit
//!   (0x0100) is refused as `PermissionDenied`; it can still be exported
//!   whole.
//! - **Limits**: `skera` has no CFF or CFF2 subsetter yet, so an OpenType
//!   font with PostScript outlines is refused as `Unsupported` rather than
//!   written without outlines. Tables `skera` cannot renumber glyphs in are
//!   dropped, as HarfBuzz's subsetter does by default (`kern`, `morx`,
//!   `SVG `, `EBDT`, Graphite, ...).

use std::collections::HashSet;
use std::sync::LazyLock;

use fm_api::ErrorCode;
use fm_api::export::Subset;
use read_fonts::collections::IntSet;
use read_fonts::types::{GlyphId, NameId, Tag};
use read_fonts::{FontRef, TableProvider as _};
use serde::Deserialize;
use skera::{Plan, SubsetFlags};
use skrifa::MetadataProvider as _;

use crate::error::{FmError, FmResult};
use crate::web_font::{self, Sfnt};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./presets.schema.json";

/// The data file, embedded at build time.
const FILE: &str = include_str!("../../../data/subsets/presets.json");

/// The highest Unicode code point.
const MAX_CODE_POINT: u32 = 0x10_FFFF;

/// One named preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// The name a request uses (`latin-ext`).
    pub name: String,
    /// Its ranges, inclusive, as the file lists them.
    pub ranges: Vec<(u32, u32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    source: String,
    presets: Vec<RawPreset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPreset {
    name: String,
    ranges: Vec<String>,
}

static PRESETS: LazyLock<FmResult<Vec<Preset>>> = LazyLock::new(|| parse(FILE));

/// The embedded presets, parsed once, in file order.
pub fn presets() -> FmResult<&'static [Preset]> {
    PRESETS.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}

/// Parses and checks a preset file: every rule of `presets.schema.json`,
/// plus first ≤ last and at most U+10FFFF, which a schema cannot say.
fn parse(json: &str) -> FmResult<Vec<Preset>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(&e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(&format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if !raw.source.starts_with("https://") {
        return Err(damaged("source is not an https URL"));
    }
    if raw.presets.is_empty() {
        return Err(damaged("presets is empty"));
    }
    let mut names = HashSet::new();
    let mut out = Vec::with_capacity(raw.presets.len());
    for p in raw.presets {
        if !is_name(&p.name) {
            return Err(damaged(&format!(
                "\"{}\" is not a lower-case preset name",
                p.name
            )));
        }
        if !names.insert(p.name.clone()) {
            return Err(damaged(&format!("{} is listed twice", p.name)));
        }
        if p.ranges.is_empty() {
            return Err(damaged(&format!("{}: ranges is empty", p.name)));
        }
        let mut ranges = Vec::with_capacity(p.ranges.len());
        for r in &p.ranges {
            // The file writes hex in upper case, like the schema says.
            if r.chars().any(|c| c.is_ascii_lowercase()) {
                return Err(damaged(&format!("{}: {r} is not in upper case", p.name)));
            }
            ranges.push(
                unicode_range(r)
                    .ok_or_else(|| damaged(&format!("{}: {r} is malformed", p.name)))?,
            );
        }
        out.push(Preset {
            name: p.name,
            ranges,
        });
    }
    Ok(out)
}

fn damaged(detail: &str) -> FmError {
    FmError::internal(format!("data/subsets/presets.json: {detail}"))
}

/// `latin-ext`: lower-case letters and digits, words joined by `-`.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// One CSS `unicode-range` value as an inclusive range: `U+0041`,
/// `U+0000-00FF` or `U+4??` (a wildcard range, `?` only at the end). Case
/// does not matter, as in CSS. `None` when it is malformed, above U+10FFFF,
/// or its first code point is after its last.
pub fn unicode_range(value: &str) -> Option<(u32, u32)> {
    let rest = value
        .trim()
        .strip_prefix("U+")
        .or_else(|| value.trim().strip_prefix("u+"))?;
    let (first, last) = if let Some((a, b)) = rest.split_once('-') {
        (hex(a)?, hex(b)?)
    } else if rest.ends_with('?') {
        let digits = rest.trim_end_matches('?');
        let wild = rest.len() - digits.len();
        if rest.len() > 6 || digits.contains('?') {
            return None;
        }
        let base = if digits.is_empty() { 0 } else { hex(digits)? };
        let shift = 4 * u32::try_from(wild).ok()?;
        (base << shift, (base << shift) | ((1 << shift) - 1))
    } else {
        let one = hex(rest)?;
        (one, one)
    };
    (first <= last && last <= MAX_CODE_POINT).then_some((first, last))
}

/// 1 to 6 hex digits.
fn hex(digits: &str) -> Option<u32> {
    if digits.is_empty() || digits.len() > 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

/// The characters a subset keeps: sorted, merged, inclusive ranges, so a
/// request for all of Unicode stays small.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Keep {
    ranges: Vec<(u32, u32)>,
    /// U+0020 was added to a text that has no space; on its own it does
    /// not make a face "have" the text.
    space_added: bool,
}

impl Keep {
    /// The set of the code points in `ranges` (inclusive, any order).
    pub fn from_ranges(ranges: impl IntoIterator<Item = (u32, u32)>) -> Self {
        let mut all: Vec<(u32, u32)> = ranges.into_iter().collect();
        all.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(all.len());
        for (first, last) in all {
            match merged.last_mut() {
                Some(prev) if first <= prev.1.saturating_add(1) => prev.1 = prev.1.max(last),
                _ => merged.push((first, last)),
            }
        }
        Self {
            ranges: merged,
            space_added: false,
        }
    }

    /// Whether `cp` is kept.
    pub fn contains(&self, cp: u32) -> bool {
        let i = self.ranges.partition_point(|r| r.1 < cp);
        self.ranges.get(i).is_some_and(|r| r.0 <= cp)
    }

    /// The ranges, sorted and merged.
    pub fn ranges(&self) -> &[(u32, u32)] {
        &self.ranges
    }
}

/// The characters `subset` asks to keep. An empty request, an unknown
/// preset name or a malformed range is `InvalidRequest`.
pub fn code_points(subset: &Subset) -> FmResult<Keep> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    match subset {
        Subset::Presets { names } => {
            if names.is_empty() {
                return Err(FmError::invalid("Pick at least one character set to keep."));
            }
            let all = presets()?;
            for name in names {
                let preset = all.iter().find(|p| p.name == name.trim()).ok_or_else(|| {
                    FmError::invalid(format!(
                        "There is no character set called \"{name}\". Pick one of: {}.",
                        all.iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?;
                out.extend_from_slice(&preset.ranges);
            }
        }
        Subset::Unicode { ranges } => {
            // One entry may hold a whole CSS value ("U+0000-00FF, U+0131").
            let values: Vec<&str> = ranges
                .iter()
                .flat_map(|r| r.split(','))
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .collect();
            if values.is_empty() {
                return Err(FmError::invalid(
                    "Enter at least one Unicode range to keep.",
                ));
            }
            for value in values {
                let (first, last) = unicode_range(value).ok_or_else(|| {
                    FmError::invalid(format!(
                        "\"{value}\" is not a Unicode range. Write it like U+0000-00FF, U+0131 or U+4??."
                    ))
                })?;
                out.push((first, last));
            }
        }
        Subset::Text { text } => {
            if text.is_empty() {
                return Err(FmError::invalid(
                    "Type or paste the text whose characters to keep.",
                ));
            }
            out.extend(text.chars().map(|c| (u32::from(c), u32::from(c))));
            out.push((0x20, 0x20));
            return Ok(Keep {
                space_added: !text.contains(' '),
                ..Keep::from_ranges(out)
            });
        }
    }
    Ok(Keep::from_ranges(out))
}

/// The tables dropped from every subset: HarfBuzz's default list. `skera`
/// cannot renumber the glyphs in them, so a copy would point at the wrong
/// glyphs.
const DROP_TABLES: [&[u8; 4]; 17] = [
    b"morx", b"mort", b"kerx", b"kern", b"JSTF", b"DSIG", b"EBDT", b"EBLC", b"EBSC", b"SVG ",
    b"PCLT", b"LTSH", b"Feat", b"Glat", b"Gloc", b"Silf", b"Sill",
];

/// `sfnt` cut down to the code points of `keep` it maps, with every glyph
/// layout rules reach from them. A face that maps none of them is
/// `InvalidRequest`; a CFF-based face is `Unsupported`; a face whose
/// license forbids subsetting is `PermissionDenied`.
pub fn cut(sfnt: &Sfnt, keep: &Keep) -> FmResult<Sfnt> {
    let bytes = web_font::sfnt_bytes(sfnt)?;
    let font = FontRef::new(&bytes).map_err(|e| {
        FmError::unsupported("The font is damaged and cannot be subset.").with_detail(e.to_string())
    })?;
    if font.table_data(Tag::new(b"CFF ")).is_some() || font.table_data(Tag::new(b"CFF2")).is_some()
    {
        return Err(FmError::unsupported(
            "Fonts with PostScript (CFF) outlines cannot be subset yet. Export them without a subset.",
        ));
    }
    if font.os2().is_ok_and(|os2| os2.fs_type() & 0x0100 != 0) {
        return Err(FmError::new(
            ErrorCode::PermissionDenied,
            "The font's license does not allow subsetting. Export it without a subset.",
        ));
    }
    let mapped: IntSet<u32> = skrifa::FontRef::new(&bytes)
        .map_err(|e| {
            FmError::unsupported("The font is damaged and cannot be subset.")
                .with_detail(e.to_string())
        })?
        .charmap()
        .mappings()
        .map(|(cp, _)| cp)
        .filter(|&cp| keep.contains(cp))
        .collect();
    let asked = |cp: u32| !(keep.space_added && cp == 0x20);
    if !mapped.iter().any(asked) {
        return Err(FmError::invalid(
            "The font has none of the characters to keep. Choose other characters, or export it without a subset.",
        ));
    }
    // The cmap must hold exactly the mapped characters: no mirrored
    // brackets pulled in. Glyph names stay for the user and the shaper.
    let flags = SubsetFlags::SUBSET_FLAGS_GLYPH_NAMES
        | SubsetFlags::SUBSET_FLAGS_NO_BIDI_CLOSURE
        | SubsetFlags::SUBSET_FLAGS_NOTDEF_OUTLINE;
    let drop: IntSet<Tag> = DROP_TABLES.iter().map(|t| Tag::new(t)).collect();
    let plan = Plan::new(
        &IntSet::<GlyphId>::empty(),
        &mapped,
        &font,
        flags,
        &drop,
        &everything::<Tag>(),
        &everything::<Tag>(),
        &everything::<NameId>(),
        &everything::<u16>(),
    );
    let out = skera::subset_font(&font, &plan).map_err(|e| {
        FmError::unsupported("The font could not be subset.").with_detail(e.to_string())
    })?;
    web_font::standalone(&out, 0)
}

/// The set of every value: keep all scripts, features, names, languages.
fn everything<T: read_fonts::collections::int_set::Domain>() -> IntSet<T> {
    let mut all = IntSet::empty();
    all.invert();
    all
}

#[cfg(test)]
mod tests;
