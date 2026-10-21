//! The Unicode block table: `data/unicode-blocks/blocks.json` (E11.1).
//!
//! The file is a copy of `Blocks.txt` from the Unicode Character Database; it
//! records the Unicode version it comes from. It is embedded at build time, so
//! the glyph map needs no file access and no network. [`load`] parses it and
//! checks every rule of `blocks.schema.json` plus the ones a schema cannot
//! say: blocks sorted, not overlapping, `first` ≤ `last`, names unique, and
//! `assigned` (from `DerivedGeneralCategory.txt`, see
//! `scripts/unicode-blocks-assigned.mjs`) at most the block's size.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./blocks.schema.json";

/// The data file, embedded at build time.
const FILE: &str = include_str!("../../../data/unicode-blocks/blocks.json");

/// One Unicode block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The name as `Blocks.txt` writes it (`Latin Extended-A`).
    pub name: String,
    /// Its first code point.
    pub first: u32,
    /// Its last code point.
    pub last: u32,
    /// Its assigned code points: those whose General_Category is not `Cn`.
    pub assigned: u32,
}

/// The block table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTable {
    /// The Unicode version it comes from (`17.0.0`).
    pub unicode_version: String,
    /// Every block, sorted by code point.
    pub blocks: Vec<Block>,
}

impl BlockTable {
    /// The block holding `cp`, or `None` for a code point outside every block.
    pub fn find(&self, cp: u32) -> Option<&Block> {
        let i = self.blocks.partition_point(|b| b.last < cp);
        self.blocks.get(i).filter(|b| b.first <= cp)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    unicode_version: String,
    source: String,
    blocks: Vec<RawBlock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBlock {
    first: String,
    last: String,
    assigned: u32,
    name: String,
}

static TABLE: LazyLock<FmResult<BlockTable>> = LazyLock::new(|| parse(FILE));

/// The embedded block table, parsed once.
pub fn load() -> FmResult<&'static BlockTable> {
    TABLE.as_ref().map_err(Clone::clone)
}

/// Parses and checks a block file.
fn parse(json: &str) -> FmResult<BlockTable> {
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
    if raw.blocks.is_empty() {
        return Err(damaged("blocks is empty"));
    }
    let mut blocks: Vec<Block> = Vec::with_capacity(raw.blocks.len());
    for b in raw.blocks {
        let first = code_point(&b.first).map_err(|why| damaged(&format!("{}: {why}", b.name)))?;
        let last = code_point(&b.last).map_err(|why| damaged(&format!("{}: {why}", b.name)))?;
        if b.name.trim().is_empty() {
            return Err(damaged(&format!("{}: name is empty", b.first)));
        }
        if first > last {
            return Err(damaged(&format!("{}: first is after last", b.name)));
        }
        if b.assigned > last - first + 1 {
            return Err(damaged(&format!(
                "{}: assigned is more than the block holds",
                b.name
            )));
        }
        if let Some(prev) = blocks.last()
            && prev.last >= first
        {
            return Err(damaged(&format!(
                "{}: overlaps or comes before {}",
                b.name, prev.name
            )));
        }
        if blocks.iter().any(|x| x.name == b.name) {
            return Err(damaged(&format!("{}: name is listed twice", b.name)));
        }
        blocks.push(Block {
            name: b.name,
            first,
            last,
            assigned: b.assigned,
        });
    }
    Ok(BlockTable {
        unicode_version: raw.unicode_version,
        blocks,
    })
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

fn is_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn damaged(detail: &str) -> FmError {
    FmError::internal(format!("data/unicode-blocks/blocks.json: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/unicode-blocks")
    }

    fn file(blocks: &str) -> String {
        format!(
            r#"{{"$schema": "./blocks.schema.json", "unicodeVersion": "17.0.0",
               "source": "https://www.unicode.org/Public/17.0.0/ucd/Blocks.txt",
               "blocks": [{blocks}]}}"#
        )
    }

    #[test]
    fn the_data_file_is_valid_and_records_its_unicode_version() {
        let table = load().expect("blocks.json is valid");
        assert_eq!(table.unicode_version, "17.0.0");
        assert_eq!(table.blocks.len(), 346);
        let v: Value = serde_json::from_str(FILE).expect("JSON");
        assert!(
            v["source"]
                .as_str()
                .is_some_and(|s| s.contains(&table.unicode_version)),
            "the source URL names the version"
        );
    }

    #[test]
    fn the_embedded_file_is_the_file_on_disk() {
        let disk = std::fs::read_to_string(data_dir().join("blocks.json")).expect("read");
        assert_eq!(disk.replace("\r\n", "\n"), FILE.replace("\r\n", "\n"));
    }

    #[test]
    fn spot_checks_on_the_data() {
        let table = load().expect("valid");
        let name = |cp| table.find(cp).map(|b| b.name.as_str());
        assert_eq!(name(0x41), Some("Basic Latin"));
        assert_eq!(name(0x416), Some("Cyrillic"));
        assert_eq!(name(0x4E2D), Some("CJK Unified Ideographs"));
        assert_eq!(name(0x1F600), Some("Emoticons"));
        assert_eq!(name(0x10_FFFF), Some("Supplementary Private Use Area-B"));
        // Between Hangul Jamo Extended-B and the surrogates there is a block;
        // past the BMP's planes 4–13 there is none.
        assert_eq!(name(0x5_0000), None);
        let assigned = |cp| table.find(cp).map(|b| b.assigned);
        assert_eq!(assigned(0x41), Some(128));
        assert_eq!(assigned(0x416), Some(256));
        // Greek and Coptic has holes (U+0378, U+0379, …).
        assert_eq!(assigned(0x3B1), Some(135));
        let total: u64 = table.blocks.iter().map(|b| u64::from(b.assigned)).sum();
        // Unicode 17.0: 159,801 graphic and format characters, 65 controls,
        // 2,048 surrogates and 137,468 private-use code points; every
        // assigned code point is in a block.
        assert_eq!(total, 159_801 + 65 + 2_048 + 137_468);
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let text = std::fs::read_to_string(data_dir().join("blocks.schema.json")).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(
            keys(&schema["properties"]),
            ["$schema", "blocks", "source", "unicodeVersion"]
        );
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        assert_eq!(schema["additionalProperties"], false);
        let block = &schema["$defs"]["block"];
        assert_eq!(
            keys(&block["properties"]),
            ["assigned", "first", "last", "name"]
        );
        assert_eq!(block["additionalProperties"], false);
        assert_eq!(schema["$defs"]["codePoint"]["pattern"], "^[0-9A-F]{4,6}$");
        // The pattern the schema gives is the one the loader applies.
        assert!(
            parse(&file(
                r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A"}"#
            ))
            .is_ok()
        );
        assert!(
            parse(&file(
                r#"{"first": "000", "last": "007F", "assigned": 1, "name": "A"}"#
            ))
            .is_err()
        );
        assert!(
            parse(&file(
                r#"{"first": "0000", "last": "007f", "assigned": 1, "name": "A"}"#
            ))
            .is_err()
        );
    }

    #[test]
    fn broken_tables_are_rejected() {
        let bad = [
            // overlapping
            r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A"},
               {"first": "0070", "last": "00FF", "assigned": 1, "name": "B"}"#,
            // out of order
            r#"{"first": "0080", "last": "00FF", "assigned": 1, "name": "A"},
               {"first": "0000", "last": "007F", "assigned": 1, "name": "B"}"#,
            // first after last
            r#"{"first": "00FF", "last": "0080", "assigned": 1, "name": "A"}"#,
            // name twice
            r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A"},
               {"first": "0080", "last": "00FF", "assigned": 1, "name": "A"}"#,
            // empty name
            r#"{"first": "0000", "last": "007F", "assigned": 1, "name": " "}"#,
            // past the last code point
            r#"{"first": "110000", "last": "110001", "assigned": 1, "name": "A"}"#,
            // more assigned than the block holds
            r#"{"first": "0000", "last": "000F", "assigned": 17, "name": "A"}"#,
            // assigned missing
            r#"{"first": "0000", "last": "007F", "name": "A"}"#,
            // unknown field
            r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A", "alias": "x"}"#,
        ];
        for blocks in bad {
            let err = parse(&file(blocks)).expect_err(blocks);
            assert!(
                err.detail().unwrap_or_default().contains("blocks.json"),
                "{err:?}"
            );
        }
        assert!(parse(&file("")).is_err(), "no blocks");
        let no_version = file(r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A"}"#)
            .replace("17.0.0\"", "seventeen\"");
        assert!(parse(&no_version).is_err());
    }

    #[test]
    fn find_answers_the_edges_of_a_block() {
        let table = parse(&file(
            r#"{"first": "0000", "last": "007F", "assigned": 1, "name": "A"},
               {"first": "0100", "last": "017F", "assigned": 1, "name": "B"}"#,
        ))
        .expect("valid");
        let name = |cp| table.find(cp).map(|b| b.name.as_str());
        assert_eq!(name(0), Some("A"));
        assert_eq!(name(0x7F), Some("A"));
        assert_eq!(name(0x80), None);
        assert_eq!(name(0x100), Some("B"));
        assert_eq!(name(0x17F), Some("B"));
        assert_eq!(name(0x180), None);
    }
}
