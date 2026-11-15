//! The Unicode character names: `data/unicode-names/names.json` (E50.9).
//!
//! The file is generated from `UnicodeData.txt` of the Unicode Character
//! Database (`scripts/unicode-names.mjs`); it records the Unicode version it
//! comes from. It is embedded at build time, so the glyph detail needs no file
//! access and no network, and parsed on the first lookup. [`load`] checks every
//! rule of `names.schema.json` plus the ones a schema cannot say: ranges
//! sorted and not overlapping, `first` ≤ `last`, no name inside a range.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./names.schema.json";

/// The data file, embedded at build time.
const FILE: &str = include_str!("../../../data/unicode-names/names.json");

/// How the names of a range are made from the code point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeRule {
    /// The prefix and the code point in upper-case hex (`CJK UNIFIED IDEOGRAPH-4E00`).
    Prefix(String),
    /// The Hangul syllable names of the Unicode Standard, section 3.12.
    Hangul,
}

/// A range of code points whose names are derived from the code point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRange {
    pub first: u32,
    pub last: u32,
    pub rule: RangeRule,
}

/// The name table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameTable {
    /// The Unicode version it comes from (`17.0.0`).
    pub unicode_version: String,
    /// The derived ranges, sorted by code point.
    pub ranges: Vec<NameRange>,
    /// Every other named code point, sorted by code point.
    names: Vec<(u32, Box<str>)>,
}

impl NameTable {
    /// The Unicode name of `cp` (`LATIN SMALL LETTER G`), or `None` for a
    /// control, a surrogate, private use or an unassigned code point.
    pub fn name(&self, cp: u32) -> Option<String> {
        if let Ok(i) = self.names.binary_search_by_key(&cp, |(c, _)| *c) {
            return self.names.get(i).map(|(_, n)| n.to_string());
        }
        let i = self.ranges.partition_point(|r| r.last < cp);
        let range = self.ranges.get(i).filter(|r| r.first <= cp)?;
        match &range.rule {
            RangeRule::Prefix(prefix) => Some(format!("{prefix}{cp:04X}")),
            RangeRule::Hangul => hangul(cp),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    unicode_version: String,
    source: String,
    ranges: Vec<RawRange>,
    names: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRange {
    first: String,
    last: String,
    prefix: Option<String>,
    rule: Option<String>,
}

static TABLE: LazyLock<FmResult<NameTable>> = LazyLock::new(|| parse(FILE));

/// The embedded name table, parsed once.
pub fn load() -> FmResult<&'static NameTable> {
    TABLE.as_ref().map_err(Clone::clone)
}

/// The Unicode name of `cp` from the embedded table; `None` when it has no
/// name or the table cannot be read (the glyph detail then shows none).
pub fn unicode_name(cp: u32) -> Option<String> {
    load().ok().and_then(|t| t.name(cp))
}

/// Parses and checks a name file.
fn parse(json: &str) -> FmResult<NameTable> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(&e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(&format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if !is_version(&raw.unicode_version) {
        return Err(damaged("unicodeVersion is not a version like 17.0.0"));
    }
    if !raw.source.starts_with("https://www.unicode.org/") {
        return Err(damaged("source is not a unicode.org URL"));
    }
    if raw.names.is_empty() {
        return Err(damaged("names is empty"));
    }
    let mut ranges: Vec<NameRange> = Vec::with_capacity(raw.ranges.len());
    for r in raw.ranges {
        let first = code_point(&r.first).map_err(|why| damaged(&why))?;
        let last = code_point(&r.last).map_err(|why| damaged(&why))?;
        if first > last {
            return Err(damaged(&format!("range {}: first is after last", r.first)));
        }
        if ranges.last().is_some_and(|prev| prev.last >= first) {
            return Err(damaged(&format!(
                "range {}: overlaps or comes before the one above it",
                r.first
            )));
        }
        let rule = match (r.prefix, r.rule.as_deref()) {
            (Some(p), None) if is_prefix(&p) => RangeRule::Prefix(p),
            (None, Some("hangul")) => RangeRule::Hangul,
            _ => {
                return Err(damaged(&format!(
                    "range {}: needs a prefix like \"CJK UNIFIED IDEOGRAPH-\" or the rule \"hangul\"",
                    r.first
                )));
            }
        };
        ranges.push(NameRange { first, last, rule });
    }
    let mut names: Vec<(u32, Box<str>)> = Vec::with_capacity(raw.names.len());
    for (hex, name) in raw.names {
        let cp = code_point(&hex).map_err(|why| damaged(&why))?;
        if !is_name(&name) {
            return Err(damaged(&format!("{hex}: {name:?} is not a Unicode name")));
        }
        if ranges.iter().any(|r| (r.first..=r.last).contains(&cp)) {
            return Err(damaged(&format!("{hex}: is inside a derived range")));
        }
        names.push((cp, name.into_boxed_str()));
    }
    // The map is sorted by its hex keys, which is not code point order across lengths.
    names.sort_unstable_by_key(|(cp, _)| *cp);
    Ok(NameTable {
        unicode_version: raw.unicode_version,
        ranges,
        names,
    })
}

/// The Hangul syllable name of `cp` (Unicode Standard, section 3.12).
fn hangul(cp: u32) -> Option<String> {
    const L: [&str; 19] = [
        "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T",
        "P", "H",
    ];
    const V: [&str; 21] = [
        "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO",
        "WE", "WI", "YU", "EU", "YI", "I",
    ];
    const T: [&str; 28] = [
        "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH",
        "M", "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
    ];
    let s = cp.checked_sub(0xAC00)? as usize;
    let (l, v, t) = (s / (21 * 28), s % (21 * 28) / 28, s % 28);
    Some(format!(
        "HANGUL SYLLABLE {}{}{}",
        L.get(l)?,
        V.get(v)?,
        T.get(t)?
    ))
}

/// Upper-case hex, 4 to 6 digits, at most `10FFFF`.
fn code_point(hex: &str) -> Result<u32, String> {
    if !(4..=6).contains(&hex.len())
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    {
        return Err(format!("{hex:?} is not 4–6 upper-case hex digits"));
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|&cp| cp <= 0x10_FFFF)
        .ok_or_else(|| format!("{hex} is past U+10FFFF"))
}

/// `^[A-Z0-9][A-Z0-9 -]*$`, the schema's name pattern.
fn is_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b' ' || b == b'-')
}

/// `^[A-Z][A-Z -]*-$`, the schema's prefix pattern.
fn is_prefix(prefix: &str) -> bool {
    let b = prefix.as_bytes();
    b.len() >= 2
        && b[0].is_ascii_uppercase()
        && b[b.len() - 1] == b'-'
        && b.iter()
            .all(|&c| c.is_ascii_uppercase() || c == b' ' || c == b'-')
}

fn is_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn damaged(detail: &str) -> FmError {
    FmError::internal(format!("data/unicode-names/names.json: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn file(ranges: &str, names: &str) -> String {
        format!(
            r#"{{"$schema": "./names.schema.json", "unicodeVersion": "17.0.0",
                "source": "https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt",
                "ranges": [{ranges}], "names": {{{names}}}}}"#
        )
    }

    #[test]
    fn the_data_file_is_valid_and_matches_the_block_table_version() {
        let table = load().expect("valid");
        assert_eq!(
            table.unicode_version,
            crate::blocks::load().expect("blocks").unicode_version
        );
    }

    #[test]
    fn spot_checks_on_the_data() {
        let name = |cp| unicode_name(cp);
        assert_eq!(name(0x20).as_deref(), Some("SPACE"));
        assert_eq!(name(0x67).as_deref(), Some("LATIN SMALL LETTER G"));
        assert_eq!(name(0x416).as_deref(), Some("CYRILLIC CAPITAL LETTER ZHE"));
        assert_eq!(name(0xFB01).as_deref(), Some("LATIN SMALL LIGATURE FI"));
        assert_eq!(name(0x1F600).as_deref(), Some("GRINNING FACE"));
        // Derived ranges.
        assert_eq!(name(0x4E2D).as_deref(), Some("CJK UNIFIED IDEOGRAPH-4E2D"));
        assert_eq!(
            name(0x20000).as_deref(),
            Some("CJK UNIFIED IDEOGRAPH-20000")
        );
        assert_eq!(name(0x17000).as_deref(), Some("TANGUT IDEOGRAPH-17000"));
        assert_eq!(name(0xAC00).as_deref(), Some("HANGUL SYLLABLE GA"));
        assert_eq!(name(0xD4DB).as_deref(), Some("HANGUL SYLLABLE PWILH"));
        assert_eq!(name(0xD7A3).as_deref(), Some("HANGUL SYLLABLE HIH"));
        // No name: a control, a surrogate, private use, unassigned, past the ranges.
        assert_eq!(name(0x0A), None);
        assert_eq!(name(0xD800), None);
        assert_eq!(name(0xE000), None);
        assert_eq!(name(0x378), None);
        assert_eq!(name(0x10_FFFF), None);
    }

    #[test]
    fn the_loader_rejects_what_the_schema_rejects() {
        let ok = file(
            r#"{"first": "AC00", "last": "D7A3", "rule": "hangul"}"#,
            r#""0041": "LATIN CAPITAL LETTER A""#,
        );
        assert!(parse(&ok).is_ok());
        for (ranges, names) in [
            // A bad code point, a lower-case name, an empty table.
            ("", r#""041": "A""#),
            ("", r#""0041": "latin a""#),
            ("", ""),
            // A range with neither or both of prefix and rule, or reversed.
            (r#"{"first": "4E00", "last": "9FFF"}"#, r#""0041": "A""#),
            (
                r#"{"first": "4E00", "last": "9FFF", "prefix": "CJK-", "rule": "hangul"}"#,
                r#""0041": "A""#,
            ),
            (
                r#"{"first": "9FFF", "last": "4E00", "prefix": "CJK-"}"#,
                r#""0041": "A""#,
            ),
            // Overlapping ranges, a name inside a range.
            (
                r#"{"first": "4E00", "last": "9FFF", "prefix": "CJK-"},
                   {"first": "9000", "last": "A000", "prefix": "CJK-"}"#,
                r#""0041": "A""#,
            ),
            (
                r#"{"first": "4E00", "last": "9FFF", "prefix": "CJK-"}"#,
                r#""4E00": "A""#,
            ),
        ] {
            assert!(parse(&file(ranges, names)).is_err(), "{ranges} {names}");
        }
        assert!(parse("not json").is_err());
    }

    #[test]
    fn names_are_ordered_by_code_point_not_by_hex_text() {
        let t = parse(&file(
            "",
            r#""10000": "LINEAR B SYLLABLE B008 A", "FFFD": "REPLACEMENT CHARACTER""#,
        ))
        .expect("valid");
        assert_eq!(t.name(0xFFFD).as_deref(), Some("REPLACEMENT CHARACTER"));
        assert_eq!(t.name(0x10000).as_deref(), Some("LINEAR B SYLLABLE B008 A"));
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/unicode-names");
        let text = std::fs::read_to_string(dir.join("names.schema.json")).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
        let mut keys: Vec<&String> = schema["properties"]
            .as_object()
            .expect("object")
            .keys()
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            ["$schema", "names", "ranges", "source", "unicodeVersion"]
        );
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["$defs"]["codePoint"]["pattern"], "^[0-9A-F]{4,6}$");
        assert_eq!(
            schema["properties"]["names"]["additionalProperties"]["pattern"],
            "^[A-Z0-9][A-Z0-9 -]*$"
        );
        assert_eq!(
            schema["$defs"]["range"]["properties"]["prefix"]["pattern"],
            "^[A-Z][A-Z -]*-$"
        );
    }
}
