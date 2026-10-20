//! The glyph map of a face (E11.1): every character its `cmap` maps, with
//! the glyph id and the glyph name, grouped by Unicode block
//! ([`crate::blocks`]), one page at a time, optionally one block only or the
//! characters a search finds.
//!
//! - **Names** come from `post` (version 2) or the CFF charset, through
//!   skrifa's [`GlyphNames`]. A font with neither has no names: skrifa's
//!   synthesized `gidN` names are not reported.
//! - **Search** ([`Query`]) finds a glyph by the character itself (`Ж`), by
//!   code point (`U+0416`, `u+416`, `0x416`, `0416`), by an AGL name that
//!   spells the code point (`uni0416`, `u1F600`) and by glyph name (`Zhe-cy`,
//!   case-insensitive, a part of the name is enough).
//! - **Detail** of one character ([`glyph_detail`], E11.2): its advance
//!   width and the GSUB features that produce or replace its glyph.
//! - **Malformed data** is never a panic and never an error: a face whose
//!   header, `cmap`, `maxp` or `post` cannot be read is a page with
//!   [`Health::Broken`] and no characters. A file that cannot be read at all
//!   is an I/O error.

use std::borrow::Cow;
use std::path::Path;

use fm_api::common::{FontFormat, Health, Page};
use fm_api::library::{GlyphBlock, GlyphDetail, GlyphInfo, GlyphPage, GlyphsRequest};
use read_fonts::{FileRef, FontRef, ReadError, TableProvider};
use skrifa::instance::{LocationRef, Size};
use skrifa::{GlyphId, GlyphNames, MetadataProvider};

use crate::blocks::{self, BlockTable};
use crate::error::{FmError, FmResult};
use crate::glyph_features::{GlyphFeatures, glyph_features};
use crate::parser::decompress_web;

/// The most characters one page holds.
pub const MAX_PAGE: u32 = 500;

/// The block name of a code point outside every Unicode block.
pub const NO_BLOCK: &str = "No block";

/// Reads the font file at `path` (face `collection_index` of a collection)
/// and answers [`glyph_page`] for it.
pub fn glyph_page_file(
    path: &Path,
    collection_index: u32,
    req: &GlyphsRequest,
) -> FmResult<GlyphPage> {
    let data = std::fs::read(path)
        .map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    glyph_page(&data, collection_index, req)
}

/// One page of the glyph map of face `collection_index` in the font file
/// `data` (TrueType, OpenType, a collection, WOFF or WOFF2).
pub fn glyph_page(data: &[u8], collection_index: u32, req: &GlyphsRequest) -> FmResult<GlyphPage> {
    let table = blocks::load()?;
    let glyphs = match glyphs_of(data, collection_index) {
        Ok(glyphs) => glyphs,
        Err(reason) => return Ok(broken(table, reason)),
    };
    let query = Query::parse(req.query.as_deref().unwrap_or_default());
    let all: Vec<GlyphInfo> = glyphs
        .into_iter()
        .map(|(code_point, glyph_id, name)| GlyphInfo {
            code_point,
            glyph_id,
            name,
            block: table
                .find(code_point)
                .map_or(NO_BLOCK, |b| b.name.as_str())
                .to_owned(),
        })
        .filter(|g| req.block.as_ref().is_none_or(|b| *b == g.block))
        .filter(|g| query.matches(g))
        .collect();
    let blocks = blocks_of(&all, table);
    let Page { offset, limit } = req.page;
    Ok(GlyphPage {
        total: u32::try_from(all.len()).unwrap_or(u32::MAX),
        items: all
            .into_iter()
            .skip(offset as usize)
            .take(limit.min(MAX_PAGE) as usize)
            .collect(),
        blocks,
        health: Health::Ok,
        unicode_version: table.unicode_version.clone(),
    })
}

/// Reads the font file at `path` (face `collection_index` of a collection)
/// and answers [`glyph_detail`] for it.
pub fn glyph_detail_file(
    path: &Path,
    collection_index: u32,
    code_point: u32,
) -> FmResult<GlyphDetail> {
    let data = std::fs::read(path)
        .map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    glyph_detail(&data, collection_index, code_point)
}

/// The detail of character `code_point` of face `collection_index` in the
/// font file `data` (E11.2): glyph id and name, advance width at the default
/// location and the GSUB features that produce or replace the glyph
/// ([`glyph_features`]). `NotFound` when the face does not map the
/// character; a malformed font is a detail with [`Health::Broken`].
pub fn glyph_detail(data: &[u8], collection_index: u32, code_point: u32) -> FmResult<GlyphDetail> {
    let broken = |reason: String| GlyphDetail {
        code_point,
        glyph_id: 0,
        name: None,
        advance_width: None,
        produced_by: Vec::new(),
        replaced_by: Vec::new(),
        health: Health::Broken(reason),
    };
    let sfnt = match sfnt(data) {
        Ok(sfnt) => sfnt,
        Err(reason) => return Ok(broken(reason)),
    };
    let (font, num_glyphs) =
        match face(&sfnt, collection_index).and_then(|font| num_glyphs(&font).map(|n| (font, n))) {
            Ok(found) => found,
            Err(reason) => return Ok(broken(reason)),
        };
    let gid = font
        .charmap()
        .map(code_point)
        .filter(|&gid| draws(gid, num_glyphs))
        .ok_or_else(|| FmError::not_found(format_args!("Character U+{code_point:04X}")))?;
    // skrifa reads a missing hmtx as zero advances; that is no advance.
    let advance_width = font.hmtx().ok().and_then(|_| {
        font.glyph_metrics(Size::unscaled(), LocationRef::default())
            .advance_width(gid)
            .map(|w| w.max(0.0).round() as u32)
    });
    let GlyphFeatures {
        produced_by,
        replaced_by,
    } = glyph_features(&font, gid);
    Ok(GlyphDetail {
        code_point,
        glyph_id: gid.to_u32(),
        name: glyph_name(&font.glyph_names(), gid),
        advance_width,
        produced_by,
        replaced_by,
        health: Health::Ok,
    })
}

fn broken(table: &BlockTable, reason: String) -> GlyphPage {
    GlyphPage {
        items: Vec::new(),
        total: 0,
        blocks: Vec::new(),
        health: Health::Broken(reason),
        unicode_version: table.unicode_version.clone(),
    }
}

/// Every mapped character of the face: code point, glyph id and name, by
/// code point. A mapping to `.notdef` or past the glyph count is not a
/// character the face draws and is left out. `Err` is the reason the face
/// is broken.
fn glyphs_of(data: &[u8], index: u32) -> Result<Vec<(u32, u32, Option<String>)>, String> {
    let sfnt = sfnt(data)?;
    let font = face(&sfnt, index)?;
    let num_glyphs = num_glyphs(&font)?;
    let names = font.glyph_names();
    let mut out: Vec<(u32, u32, Option<String>)> = font
        .charmap()
        .mappings()
        .filter(|&(_, gid)| draws(gid, num_glyphs))
        .map(|(cp, gid)| (cp, gid.to_u32(), glyph_name(&names, gid)))
        .collect();
    out.sort_unstable_by_key(|&(cp, _, _)| cp);
    out.dedup_by_key(|&mut (cp, _, _)| cp);
    Ok(out)
}

/// Face `index` of the TrueType or OpenType data `sfnt`, with the tables
/// the glyph map reads checked. `Err` is the reason the face is broken.
pub(crate) fn face(sfnt: &[u8], index: u32) -> Result<FontRef<'_>, String> {
    let font = match FileRef::new(sfnt) {
        Ok(FileRef::Font(font)) if index == 0 => font,
        Ok(FileRef::Font(_)) => return Err(format!("The file has no face {index}.")),
        Ok(FileRef::Collection(c)) => c
            .get(index)
            .map_err(|e| format!("Face {index} of the collection is malformed: {e}."))?,
        Err(e) => return Err(format!("The font header is malformed: {e}.")),
    };
    check(&font)?;
    Ok(font)
}

pub(crate) fn num_glyphs(font: &FontRef<'_>) -> Result<u32, String> {
    Ok(u32::from(
        font.maxp()
            .map_err(|e| table_error("maxp", &e))?
            .num_glyphs(),
    ))
}

/// A mapping to `.notdef` or past the glyph count is not a glyph the face
/// draws.
pub(crate) fn draws(gid: GlyphId, num_glyphs: u32) -> bool {
    gid != GlyphId::NOTDEF && gid.to_u32() < num_glyphs
}

/// The glyph's name from `post` or the CFF charset; skrifa's synthesized
/// `gidN` names are not names.
fn glyph_name(names: &GlyphNames<'_>, gid: GlyphId) -> Option<String> {
    names
        .get(gid)
        .filter(|n| !n.is_synthesized())
        .map(|n| n.as_str().to_owned())
        .filter(|n| !n.is_empty())
}

/// The tables the glyph map reads are present and readable. An absent
/// `post` is no names, not damage; an absent `cmap` or `maxp` is broken, as
/// the scan says.
fn check(font: &FontRef<'_>) -> Result<(), String> {
    let len = font.data().len() as u64;
    for record in font.table_directory().table_records() {
        let tag = record.tag();
        if [b"cmap", b"maxp", b"post"].iter().any(|t| tag == **t)
            && u64::from(record.offset()) + u64::from(record.length()) > len
        {
            return Err(format!("The {tag} table is cut off."));
        }
    }
    // skrifa's charmap skips a subtable it cannot read; for the glyph map
    // that is damage, not an empty font.
    let cmap = font.cmap().map_err(|e| table_error("cmap", &e))?;
    for record in cmap.encoding_records() {
        record
            .subtable(cmap.offset_data())
            .map_err(|e| table_error("cmap", &e))?;
    }
    font.maxp().map_err(|e| table_error("maxp", &e))?;
    match font.post() {
        Ok(_) | Err(ReadError::TableIsMissing(_)) => Ok(()),
        Err(e) => Err(table_error("post", &e)),
    }
}

fn table_error(name: &str, e: &ReadError) -> String {
    match e {
        ReadError::TableIsMissing(_) => format!("The font has no {name} table."),
        e => format!("The {name} table is malformed: {e}."),
    }
}

/// The TrueType or OpenType data of a font file; WOFF and WOFF2 are
/// decompressed.
pub(crate) fn sfnt(data: &[u8]) -> Result<Cow<'_, [u8]>, String> {
    let (format, name) = match data.get(..4) {
        Some(b"wOFF") => (FontFormat::Woff, "WOFF"),
        Some(b"wOF2") => (FontFormat::Woff2, "WOFF2"),
        _ => return Ok(Cow::Borrowed(data)),
    };
    decompress_web(data, format)
        .map(Cow::Owned)
        .ok_or_else(|| format!("The {name} data is malformed and cannot be decompressed."))
}

/// The blocks of `glyphs` (sorted by code point) with their counts. A run of
/// code points outside every block is one [`NO_BLOCK`] entry spanning it.
fn blocks_of(glyphs: &[GlyphInfo], table: &BlockTable) -> Vec<GlyphBlock> {
    let mut out: Vec<GlyphBlock> = Vec::new();
    for g in glyphs {
        match out.last_mut() {
            Some(b) if b.name == g.block => {
                b.count += 1;
                b.last = b.last.max(g.code_point);
            }
            _ => {
                let (first, last) = table
                    .find(g.code_point)
                    .map_or((g.code_point, g.code_point), |b| (b.first, b.last));
                out.push(GlyphBlock {
                    name: g.block.clone(),
                    first,
                    last,
                    count: 1,
                });
            }
        }
    }
    out
}

/// What a glyph-map search looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Code points the query spells; a glyph mapped from one matches.
    code_points: Vec<u32>,
    /// Lower-case text a glyph name must contain; `None` for no name search.
    name: Option<String>,
    /// A blank query: everything matches.
    all: bool,
}

impl Query {
    /// Reads a search: blank is everything; one character is that character;
    /// longer text is a code point in any form it spells and a glyph name.
    pub fn parse(query: &str) -> Self {
        let q = query.trim();
        if q.is_empty() {
            return Self {
                code_points: Vec::new(),
                name: None,
                all: true,
            };
        }
        let mut chars = q.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Self {
                code_points: vec![c as u32],
                name: None,
                all: false,
            };
        }
        let mut code_points = Vec::new();
        let prefixed = ["U+", "u+", "0x", "0X"]
            .iter()
            .find_map(|p| q.strip_prefix(p))
            .and_then(|hex| hex_code_point(hex, 1..=6));
        // Bare hex needs 4 digits, so "ab" or "fe" stays a name search only.
        let bare = hex_code_point(q, 4..=6);
        // AGL glyph names spell code points: uni0416 (4 digits), u1F600 (4–6).
        let agl = q
            .strip_prefix("uni")
            .and_then(|hex| hex_code_point(hex, 4..=4))
            .or_else(|| {
                q.strip_prefix('u')
                    .and_then(|hex| hex_code_point(hex, 4..=6))
            });
        code_points.extend([prefixed, bare, agl].into_iter().flatten());
        code_points.sort_unstable();
        code_points.dedup();
        Self {
            code_points,
            name: Some(q.to_lowercase()),
            all: false,
        }
    }

    /// The glyph is one this search finds.
    pub fn matches(&self, g: &GlyphInfo) -> bool {
        self.all
            || self.code_points.contains(&g.code_point)
            || self.name.as_ref().is_some_and(|q| {
                g.name
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(q.as_str()))
            })
    }
}

/// `hex` as a code point when it is `digits` hex digits (either case) and at
/// most `10FFFF`.
fn hex_code_point(hex: &str, digits: std::ops::RangeInclusive<usize>) -> Option<u32> {
    if !digits.contains(&hex.len()) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16)
        .ok()
        .filter(|&cp| cp <= 0x10_FFFF)
}

#[cfg(test)]
mod tests;
