//! Coverage at scan time (E3.7): what a face's `cmap` covers, kept compact,
//! and how much of every language preset of [`crate::charsets`] it covers.
//!
//! A [`CodePointSet`] is a two-level bitset: one 256-bit block per Unicode
//! page (`cp >> 8`) that has at least one mapped code point. A Latin face is
//! a handful of blocks, a CJK face a few hundred; membership is one binary
//! search and one bit test, so any text can be checked against it later. The
//! index stores it as [`CodePointSet::to_bytes`] in `face.cmap`.
//!
//! [`languages`] counts, for every preset, how many of its base characters a
//! face maps and whether the face has `locl` forms for the language. The scan
//! stores the result per face (`face_language`); a preset the face covers
//! nothing of and has no `locl` for has no row (`covered = 0`).
//!
//! [`face_coverage`] is the coverage list of the Glyphs tab (E11.3): per
//! Unicode block ([`crate::blocks`]) the characters a face maps against the
//! block's assigned code points, and per preset what [`languages`] counts,
//! complete presets first.
//!
//! A [`Need`] is what the `supports` search filter (E3.8) asks: the base
//! characters of some presets, or the characters of a pasted text. Checking
//! a face against it is one merge over the pages of its set.

use std::sync::LazyLock;

use fm_api::FaceId;
use fm_api::search::{
    BlockCoverage, CoverageReport, FaceCoverage, FaceLanguageCoverage, LanguageCoverage, Supports,
};

use crate::blocks::{self, BlockTable};
use crate::charsets::{self, Charset};
use crate::error::{FmError, FmResult};

/// The highest Unicode code point.
const MAX_CODE_POINT: u32 = 0x10_FFFF;

/// Bytes of one stored block: the page as little-endian `u16`, then the 256
/// bits as four little-endian `u64` words.
const BLOCK_BYTES: usize = 2 + 4 * 8;

/// The code points a face maps, as one 256-bit block per used Unicode page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodePointSet {
    /// `(page, bits)`, pages strictly ascending, no block without a bit set.
    blocks: Vec<(u16, [u64; 4])>,
}

impl CodePointSet {
    /// The set of `code_points`, in any order; values past U+10FFFF are dropped.
    pub fn from_code_points(code_points: impl IntoIterator<Item = u32>) -> Self {
        let mut points: Vec<u32> = code_points
            .into_iter()
            .filter(|&cp| cp <= MAX_CODE_POINT)
            .collect();
        points.sort_unstable();
        let mut blocks: Vec<(u16, [u64; 4])> = Vec::new();
        for cp in points {
            let (page, word, bit) = split(cp);
            match blocks.last_mut() {
                Some((last, bits)) if *last == page => bits[word] |= bit,
                _ => {
                    let mut bits = [0; 4];
                    bits[word] = bit;
                    blocks.push((page, bits));
                }
            }
        }
        Self { blocks }
    }

    /// Whether the set holds `cp`.
    pub fn contains(&self, cp: u32) -> bool {
        if cp > MAX_CODE_POINT {
            return false;
        }
        let (page, word, bit) = split(cp);
        self.blocks
            .binary_search_by_key(&page, |(p, _)| *p)
            .is_ok_and(|i| self.blocks[i].1[word] & bit != 0)
    }

    /// The number of code points in the set.
    pub fn len(&self) -> usize {
        self.blocks
            .iter()
            .map(|(_, bits)| bits.iter().map(|w| w.count_ones() as usize).sum::<usize>())
            .sum()
    }

    /// Whether the set holds no code point.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// The code points, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.blocks.iter().flat_map(|(page, bits)| {
            let base = u32::from(*page) << 8;
            (0u32..).zip(bits).flat_map(move |(i, &word)| {
                let mut rest = word;
                std::iter::from_fn(move || {
                    (rest != 0).then(|| {
                        let bit = rest.trailing_zeros();
                        rest &= rest - 1;
                        base | (i * 64) | bit
                    })
                })
            })
        })
    }

    /// The number of code points in both sets: one merge over the pages.
    pub fn intersection_len(&self, other: &Self) -> usize {
        let (mut a, mut b) = (
            self.blocks.iter().peekable(),
            other.blocks.iter().peekable(),
        );
        let mut out = 0;
        while let (Some((pa, ba)), Some((pb, bb))) = (a.peek(), b.peek()) {
            match pa.cmp(pb) {
                std::cmp::Ordering::Less => _ = a.next(),
                std::cmp::Ordering::Greater => _ = b.next(),
                std::cmp::Ordering::Equal => {
                    out += ba
                        .iter()
                        .zip(bb)
                        .map(|(x, y)| (x & y).count_ones() as usize)
                        .sum::<usize>();
                    a.next();
                    b.next();
                }
            }
        }
        out
    }

    /// The stored form: [`BLOCK_BYTES`] per block, in page order.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.blocks.len() * BLOCK_BYTES);
        for (page, bits) in &self.blocks {
            out.extend_from_slice(&page.to_le_bytes());
            for word in bits {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
        out
    }

    /// The set [`CodePointSet::to_bytes`] wrote; `None` for bytes it cannot
    /// have written (a cut-off block, pages out of order or past U+10FFFF,
    /// an empty block).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (chunks, rest) = bytes.as_chunks::<BLOCK_BYTES>();
        if !rest.is_empty() {
            return None;
        }
        let mut blocks: Vec<(u16, [u64; 4])> = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let (page, words) = chunk.split_at(2);
            let page = u16::from_le_bytes([page[0], page[1]]);
            let (words, _) = words.as_chunks::<8>();
            let mut bits = [0u64; 4];
            for (slot, word) in bits.iter_mut().zip(words) {
                *slot = u64::from_le_bytes(*word);
            }
            let ordered = blocks.last().is_none_or(|(last, _)| *last < page);
            if !ordered || u32::from(page) > MAX_CODE_POINT >> 8 || bits == [0; 4] {
                return None;
            }
            blocks.push((page, bits));
        }
        Some(Self { blocks })
    }
}

/// `cp` as (page, word in the block, bit in the word).
fn split(cp: u32) -> (u16, usize, u64) {
    let page = u16::try_from(cp >> 8).unwrap_or(u16::MAX);
    let low = cp & 0xFF;
    (page, (low / 64) as usize, 1 << (low % 64))
}

/// How much of one language preset a face covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLanguage {
    /// Preset code (`sr-Cyrl`).
    pub code: String,
    /// Base characters of the preset the face maps.
    pub covered: u32,
    /// Base characters the preset needs.
    pub total: u32,
    /// The face has `locl` forms for the preset's OpenType language system.
    pub has_locl: bool,
}

impl FaceLanguage {
    /// Every base character is covered.
    pub fn is_complete(&self) -> bool {
        self.covered == self.total
    }
}

/// A preset prepared for counting.
#[derive(Debug)]
struct Preset {
    code: String,
    /// English name ("Serbian (Cyrillic)").
    name: String,
    base: CodePointSet,
    /// Characters in `base`.
    total: u32,
    /// OpenType script tags of the preset's ISO 15924 script.
    ot_scripts: Vec<String>,
    /// OpenType language tag, trimmed (`SRB`).
    ot_language: Option<String>,
}

impl Preset {
    fn new(charset: &Charset) -> Self {
        Self {
            code: charset.code.clone(),
            name: charset.name.clone(),
            base: CodePointSet::from_code_points(charset.base.iter().map(|&c| u32::from(c))),
            total: u32::try_from(charset.base.len()).unwrap_or(u32::MAX),
            ot_scripts: ot_scripts(&charset.script),
            ot_language: charset
                .ot_language
                .as_deref()
                .map(|t| t.trim_end().to_owned()),
        }
    }

    /// Whether `locl` (the face's `script/LANG` entries) names this preset's
    /// language under one of its scripts: Serbian `locl` under `cyrl` is not
    /// Serbian Latin.
    fn has_locl(&self, locl: &[String]) -> bool {
        let Some(lang) = &self.ot_language else {
            return false;
        };
        locl.iter().any(|entry| {
            entry
                .split_once('/')
                .is_some_and(|(script, l)| l == lang && self.ot_scripts.iter().any(|s| s == script))
        })
    }
}

/// The OpenType script tags fonts use for an ISO 15924 script. Most are the
/// code in lower case; the exceptions are the Indic v2 tags and the CJK
/// scripts, which OpenType splits differently.
fn ot_scripts(iso: &str) -> Vec<String> {
    let tags: &[&str] = match iso {
        "Deva" => &["deva", "dev2"],
        "Hans" | "Hant" => &["hani"],
        "Jpan" => &["kana", "hani"],
        "Kore" => &["hang", "hani"],
        _ => return vec![iso.to_ascii_lowercase()],
    };
    tags.iter().map(|&t| t.to_owned()).collect()
}

/// The presets of `data/charsets`, loaded once. The data is embedded and its
/// tests reject a broken file, so a load error is a build defect: it is
/// logged and coverage is computed against no presets.
static PRESETS: LazyLock<Vec<Preset>> = LazyLock::new(|| match charsets::load() {
    Ok(all) => all.iter().map(Preset::new).collect(),
    Err(e) => {
        tracing::error!(error = %e, "language presets could not be loaded");
        Vec::new()
    }
});

/// The coverage of every preset by a face that maps `set` and has `locl`
/// forms for `locl` (`script/LANG` entries, see `ParsedFace::locl_languages`),
/// in preset order. Presets with nothing covered and no `locl` are left out.
pub fn languages(set: &CodePointSet, locl: &[String]) -> Vec<FaceLanguage> {
    languages_of(&PRESETS, set, locl)
}

fn languages_of(presets: &[Preset], set: &CodePointSet, locl: &[String]) -> Vec<FaceLanguage> {
    if set.is_empty() && locl.is_empty() {
        return Vec::new();
    }
    presets
        .iter()
        .filter_map(|p| {
            let covered = set.intersection_len(&p.base);
            let has_locl = p.has_locl(locl);
            (covered > 0 || has_locl).then(|| FaceLanguage {
                code: p.code.clone(),
                covered: u32::try_from(covered).unwrap_or(u32::MAX),
                total: p.total,
                has_locl,
            })
        })
        .collect()
}

/// The coverage list of one face (`face_coverage`) that maps `set` and has
/// `locl` forms for `locl`: the blocks it maps anything of, in Unicode order,
/// and the presets [`languages`] reports, complete ones first.
pub fn face_coverage(face: FaceId, set: &CodePointSet, locl: &[String]) -> FmResult<FaceCoverage> {
    Ok(face_coverage_of(&PRESETS, blocks::load()?, face, set, locl))
}

fn face_coverage_of(
    presets: &[Preset],
    table: &BlockTable,
    face: FaceId,
    set: &CodePointSet,
    locl: &[String],
) -> FaceCoverage {
    let mut languages: Vec<FaceLanguageCoverage> = languages_of(presets, set, locl)
        .into_iter()
        .map(|l| FaceLanguageCoverage {
            name: presets
                .iter()
                .find(|p| p.code == l.code)
                .map_or_else(|| l.code.clone(), |p| p.name.clone()),
            code: l.code,
            covered: l.covered,
            total: l.total,
            has_locl: l.has_locl,
        })
        .collect();
    // Stable: complete ones keep preset order, partial ones go by the share
    // covered (`a/b > c/d` as `a*d > c*b`), ties in preset order.
    languages.sort_by(|a, b| {
        let complete = |l: &FaceLanguageCoverage| l.covered >= l.total;
        let share = |x: &FaceLanguageCoverage, y: &FaceLanguageCoverage| {
            u64::from(x.covered) * u64::from(y.total)
        };
        complete(b)
            .cmp(&complete(a))
            .then_with(|| share(b, a).cmp(&share(a, b)))
    });
    FaceCoverage {
        face,
        blocks: block_coverage(table, set),
        languages,
    }
}

/// Per block of `table` that holds a code point of `set`, how many it holds,
/// in Unicode order. Code points outside every block are not counted. A
/// font that maps unassigned code points cannot cover more than `assigned`.
fn block_coverage(table: &BlockTable, set: &CodePointSet) -> Vec<BlockCoverage> {
    let mut out: Vec<BlockCoverage> = Vec::new();
    for cp in set.iter() {
        let Some(block) = table.find(cp) else {
            continue;
        };
        match out.last_mut() {
            Some(last) if last.first == block.first => last.covered += 1,
            _ => out.push(BlockCoverage {
                name: block.name.clone(),
                first: block.first,
                last: block.last,
                covered: 1,
                assigned: block.assigned,
            }),
        }
    }
    for b in &mut out {
        b.covered = b.covered.min(b.assigned);
    }
    out
}

/// A face that lacks at most this many characters of a [`Need`] is a near
/// miss: the `supports` filter keeps it and lists what it lacks.
pub const NEAR_MISS: usize = 5;

/// What a `supports` filter (E3.8) asks of a face: the characters it must
/// map, and the languages whose `locl` forms are reported.
#[derive(Debug, Clone)]
pub struct Need<'a> {
    chars: CodePointSet,
    /// `chars.len()`.
    len: usize,
    /// The chosen presets; empty for pasted text.
    presets: Vec<&'a Preset>,
}

/// How one face meets a [`Need`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fit {
    /// Characters of the need the face does not map.
    pub missing: usize,
    /// Those characters, ascending, when there are at most [`NEAR_MISS`];
    /// empty otherwise.
    pub chars: Vec<char>,
    /// The face has `locl` forms for one of the chosen languages.
    pub has_locl: bool,
}

impl Fit {
    /// Complete, or a near miss.
    pub fn passes(&self) -> bool {
        self.missing <= NEAR_MISS
    }
}

impl Need<'static> {
    /// Every base character of every preset in `codes` (AND). An unknown
    /// code is an `InvalidRequest` error.
    pub fn languages(codes: &[String]) -> FmResult<Self> {
        Need::languages_of(&PRESETS, codes)
    }
}

impl<'a> Need<'a> {
    fn languages_of(all: &'a [Preset], codes: &[String]) -> FmResult<Self> {
        let presets = codes
            .iter()
            .map(|code| {
                all.iter()
                    .find(|p| p.code == *code)
                    .ok_or_else(|| FmError::invalid(format!("Unknown language preset \"{code}\".")))
            })
            .collect::<FmResult<Vec<_>>>()?;
        let chars = CodePointSet::from_code_points(presets.iter().flat_map(|p| p.base.iter()));
        Ok(Self::new(chars, presets))
    }

    /// Every character of `text` a font draws a glyph for: whitespace,
    /// controls and invisible format characters are left out.
    pub fn text(text: &str) -> Self {
        let chars = CodePointSet::from_code_points(
            text.chars().filter(|&c| !is_ignorable(c)).map(u32::from),
        );
        Self::new(chars, Vec::new())
    }

    fn new(chars: CodePointSet, presets: Vec<&'a Preset>) -> Self {
        Self {
            len: chars.len(),
            chars,
            presets,
        }
    }

    /// Asks for no character: every face meets it.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How a face that maps `set` and has `locl` forms for `locl`
    /// (`script/LANG` entries) meets the need.
    pub fn check(&self, set: &CodePointSet, locl: &[String]) -> Fit {
        let missing = self.len - self.chars.intersection_len(set);
        let chars = if missing <= NEAR_MISS {
            self.chars
                .iter()
                .filter(|&cp| !set.contains(cp))
                .filter_map(char::from_u32)
                .collect()
        } else {
            Vec::new()
        };
        Fit {
            missing,
            chars,
            has_locl: !locl.is_empty() && self.presets.iter().any(|p| p.has_locl(locl)),
        }
    }
}

/// Characters a font needs no glyph for: whitespace, controls, the soft
/// hyphen, zero-width and joiner characters, the BOM and variation selectors.
pub(crate) fn is_ignorable(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(
            c,
            '\u{AD}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2060}'..='\u{2064}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FEFF}'
                | '\u{E0100}'..='\u{E01EF}'
        )
}

/// The coverage check of one face (`check_coverage`): which characters of
/// `target` the face, mapping `set` with `locl` forms for `locl`, lacks, and
/// per chosen language how much of it the face covers. Unlike a [`Need`], it
/// lists every missing character. An unknown preset is `InvalidRequest`.
pub fn report(
    face: FaceId,
    set: &CodePointSet,
    locl: &[String],
    target: &Supports,
) -> FmResult<CoverageReport> {
    report_of(&PRESETS, face, set, locl, target)
}

fn report_of(
    all: &[Preset],
    face: FaceId,
    set: &CodePointSet,
    locl: &[String],
    target: &Supports,
) -> FmResult<CoverageReport> {
    let mut missing: Vec<String> = Vec::new();
    let add = |c: char, missing: &mut Vec<String>| {
        let c = c.to_string();
        if !missing.contains(&c) {
            missing.push(c);
        }
    };
    let mut languages = Vec::new();
    match target {
        Supports::Text { text } => {
            for c in text.chars().filter(|&c| !is_ignorable(c)) {
                if !set.contains(u32::from(c)) {
                    add(c, &mut missing);
                }
            }
        }
        Supports::Languages { codes } => {
            for code in codes {
                let preset = all.iter().find(|p| p.code == *code).ok_or_else(|| {
                    FmError::invalid(format!("Unknown language preset \"{code}\"."))
                })?;
                let lacks: Vec<char> = preset
                    .base
                    .iter()
                    .filter(|&cp| !set.contains(cp))
                    .filter_map(char::from_u32)
                    .collect();
                for &c in &lacks {
                    add(c, &mut missing);
                }
                languages.push(LanguageCoverage {
                    code: preset.code.clone(),
                    covered: preset
                        .total
                        .saturating_sub(u32::try_from(lacks.len()).unwrap_or(u32::MAX)),
                    total: preset.total,
                    missing: lacks.iter().map(char::to_string).collect(),
                    has_locl: preset.has_locl(locl),
                });
            }
        }
    }
    Ok(CoverageReport {
        face,
        complete: missing.is_empty(),
        missing,
        languages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_lists_every_missing_character_per_language() {
        let presets = [
            preset("de", "Latn", Some("DEU"), "aäbß"),
            preset("sr-Cyrl", "Cyrl", Some("SRB"), "бгд"),
        ];
        let target = Supports::Languages {
            codes: vec!["de".into(), "sr-Cyrl".into()],
        };
        let got = report_of(
            &presets,
            FaceId(4),
            &set("abб"),
            &["cyrl/SRB".into()],
            &target,
        )
        .expect("report");
        assert_eq!(got.face, FaceId(4));
        assert!(!got.complete);
        assert_eq!(got.missing, ["ß", "ä", "г", "д"], "code point order");
        assert_eq!(got.languages.len(), 2);
        assert_eq!((got.languages[0].covered, got.languages[0].total), (2, 4));
        assert_eq!(got.languages[0].missing, ["ß", "ä"]);
        assert!(!got.languages[0].has_locl);
        assert_eq!(got.languages[1].covered, 1);
        assert!(got.languages[1].has_locl);
    }

    #[test]
    fn a_text_report_ignores_spaces_and_repeats() {
        let target = Supports::Text {
            text: "ab ab\u{200B}c".into(),
        };
        let got = report_of(&[], FaceId(1), &set("ab"), &[], &target).expect("report");
        assert_eq!(got.missing, ["c"]);
        assert!(got.languages.is_empty());
        let full = report_of(&[], FaceId(1), &set("abc"), &[], &target).expect("report");
        assert!(full.complete);
    }

    #[test]
    fn a_report_on_an_unknown_preset_is_invalid() {
        let target = Supports::Languages {
            codes: vec!["xx".into()],
        };
        let err = report_of(&[], FaceId(1), &set("a"), &[], &target).expect_err("unknown");
        assert_eq!(err.code(), fm_api::ErrorCode::InvalidRequest);
    }

    fn set(text: &str) -> CodePointSet {
        CodePointSet::from_code_points(text.chars().map(u32::from))
    }

    fn table(blocks: &[(&str, u32, u32, u32)]) -> BlockTable {
        BlockTable {
            unicode_version: "17.0.0".into(),
            blocks: blocks
                .iter()
                .map(|&(name, first, last, assigned)| crate::blocks::Block {
                    name: name.into(),
                    first,
                    last,
                    assigned,
                })
                .collect(),
        }
    }

    #[test]
    fn blocks_are_listed_in_unicode_order_with_covered_and_assigned() {
        let t = table(&[
            ("Basic Latin", 0x00, 0x7F, 128),
            ("Latin-1 Supplement", 0x80, 0xFF, 128),
            ("Greek and Coptic", 0x370, 0x3FF, 135),
            ("Cyrillic", 0x400, 0x4FF, 256),
        ]);
        // Cyrillic before Latin in the text; U+0378 is unassigned in Greek;
        // U+0500 is outside every block of the table.
        let got = block_coverage(&t, &set("\u{436}\u{437}AB\u{378}\u{500}"));
        let rows: Vec<(&str, u32, u32)> = got
            .iter()
            .map(|b| (b.name.as_str(), b.covered, b.assigned))
            .collect();
        assert_eq!(
            rows,
            [
                ("Basic Latin", 2, 128),
                ("Greek and Coptic", 1, 135),
                ("Cyrillic", 2, 256),
            ],
            "only blocks with a covered character, in Unicode order"
        );
        assert_eq!((got[2].first, got[2].last), (0x400, 0x4FF));
        assert!(block_coverage(&t, &set("")).is_empty());
    }

    #[test]
    fn a_block_is_never_covered_past_its_assigned_code_points() {
        let t = table(&[("Tiny", 0x00, 0x0F, 2)]);
        let got = block_coverage(&t, &CodePointSet::from_code_points(0..16));
        assert_eq!((got[0].covered, got[0].assigned), (2, 2));
    }

    #[test]
    fn languages_list_complete_first_then_partial_by_share() {
        let presets = [
            preset("de", "Latn", Some("DEU"), "abcd"),
            preset("en", "Latn", None, "ab"),
            preset("pl", "Latn", Some("PLK"), "abcdefgh"),
            preset("sr-Cyrl", "Cyrl", Some("SRB"), "\u{431}\u{433}\u{434}"),
            preset("fr", "Latn", None, "ab"),
            preset("ru", "Cyrl", Some("RUS"), "\u{436}\u{437}"),
        ];
        let t = table(&[]);
        // de 3/4, en 2/2, pl 3/8, sr-Cyrl 0/3 with locl, fr 2/2, ru nothing.
        let got = face_coverage_of(&presets, &t, FaceId(7), &set("abc"), &["cyrl/SRB".into()]);
        assert_eq!(got.face, FaceId(7));
        let rows: Vec<(&str, u32, u32, bool)> = got
            .languages
            .iter()
            .map(|l| (l.code.as_str(), l.covered, l.total, l.has_locl))
            .collect();
        assert_eq!(
            rows,
            [
                ("en", 2, 2, false),
                ("fr", 2, 2, false),
                ("de", 3, 4, false),
                ("pl", 3, 8, false),
                ("sr-Cyrl", 0, 3, true),
            ]
        );
        assert!(
            got.languages.iter().all(|l| l.name == l.code),
            "names from the preset"
        );
    }

    #[test]
    fn the_embedded_tables_give_a_coverage_list() {
        let got =
            face_coverage(FaceId(1), &set("abcdefghijklmnopqrstuvwxyz\u{436}"), &[]).expect("list");
        assert_eq!(got.blocks[0].name, "Basic Latin");
        assert_eq!((got.blocks[0].covered, got.blocks[0].assigned), (26, 128));
        assert_eq!(got.blocks[1].name, "Cyrillic");
        let en = got.languages.iter().find(|l| l.code == "en").expect("en");
        assert_eq!(en.name, "English");
    }

    fn preset(code: &str, script: &str, ot: Option<&str>, base: &str) -> Preset {
        Preset::new(&Charset {
            code: code.into(),
            name: code.into(),
            script: script.into(),
            ot_language: ot.map(|t| format!("{t:<4}")),
            base: base.chars().collect(),
            auxiliary: Vec::new(),
            source: charsets::Source::Project,
        })
    }

    #[test]
    fn a_set_holds_exactly_its_code_points() {
        let points = [0x41, 0x42, 0x43, 0xFF, 0x100, 0x4E00, 0x1F600, 0x10_FFFF];
        let s = CodePointSet::from_code_points(points.iter().rev().copied().chain([0x41]));
        assert_eq!(s.len(), points.len());
        assert_eq!(s.iter().collect::<Vec<_>>(), points);
        for cp in points {
            assert!(s.contains(cp), "U+{cp:04X}");
        }
        for cp in [
            0x40,
            0x44,
            0xFE,
            0x101,
            0x4DFF,
            0x1F601,
            0x11_0000,
            u32::MAX,
        ] {
            assert!(!s.contains(cp), "U+{cp:04X}");
        }
        assert!(CodePointSet::from_code_points([0x11_0000]).is_empty());
    }

    #[test]
    fn intersection_counts_shared_code_points_across_pages() {
        let a = CodePointSet::from_code_points([0x41, 0x42, 0x141, 0x4E00, 0x1F600]);
        let b = CodePointSet::from_code_points([0x42, 0x43, 0x141, 0x1F600, 0x10_FFFF]);
        assert_eq!(a.intersection_len(&b), 3);
        assert_eq!(b.intersection_len(&a), 3);
        assert_eq!(a.intersection_len(&a), a.len());
        assert_eq!(a.intersection_len(&CodePointSet::default()), 0);
    }

    #[test]
    fn a_set_round_trips_through_bytes_and_stays_compact() {
        let latin: Vec<u32> = (0x20..0x7F).chain(0xA0..0x180).collect();
        let s = CodePointSet::from_code_points(latin.iter().copied());
        let bytes = s.to_bytes();
        // Two pages (U+00xx, U+01xx): two blocks, whatever the number of points.
        assert_eq!(bytes.len(), 2 * BLOCK_BYTES);
        assert_eq!(CodePointSet::from_bytes(&bytes), Some(s));
        assert_eq!(CodePointSet::from_bytes(&[]), Some(CodePointSet::default()));
        assert!(CodePointSet::default().to_bytes().is_empty());
    }

    #[test]
    fn damaged_bytes_are_refused() {
        let two = CodePointSet::from_code_points([0x41, 0x141]).to_bytes();
        assert_eq!(
            CodePointSet::from_bytes(&two[..BLOCK_BYTES + 1]),
            None,
            "cut off"
        );
        let mut swapped = two[BLOCK_BYTES..].to_vec();
        swapped.extend_from_slice(&two[..BLOCK_BYTES]);
        assert_eq!(
            CodePointSet::from_bytes(&swapped),
            None,
            "pages out of order"
        );
        let mut repeated = two[..BLOCK_BYTES].to_vec();
        repeated.extend_from_slice(&two[..BLOCK_BYTES]);
        assert_eq!(CodePointSet::from_bytes(&repeated), None, "page twice");
        let mut empty = vec![0u8; BLOCK_BYTES];
        empty[0] = 1;
        assert_eq!(CodePointSet::from_bytes(&empty), None, "empty block");
        let mut past = two[..BLOCK_BYTES].to_vec();
        past[..2].copy_from_slice(&0x1100u16.to_le_bytes());
        assert_eq!(CodePointSet::from_bytes(&past), None, "past U+10FFFF");
    }

    #[test]
    fn coverage_counts_base_characters_per_preset() {
        let presets = [
            preset("xa", "Latn", None, "abc"),
            preset("xb", "Latn", None, "abž"),
            preset("xc", "Cyrl", None, "бв"),
        ];
        let got = languages_of(&presets, &set("abcdž"), &[]);
        let row = |code: &str| got.iter().find(|l| l.code == code).cloned();
        assert_eq!(
            row("xa"),
            Some(FaceLanguage {
                code: "xa".into(),
                covered: 3,
                total: 3,
                has_locl: false
            })
        );
        assert!(row("xa").is_some_and(|l| l.is_complete()));
        assert_eq!(row("xb").map(|l| (l.covered, l.total)), Some((3, 3)));
        assert_eq!(row("xc"), None, "nothing covered, no row");
        let partial = languages_of(&presets, &set("ab"), &[]);
        assert_eq!(
            partial
                .iter()
                .map(|l| (l.code.as_str(), l.covered))
                .collect::<Vec<_>>(),
            [("xa", 2), ("xb", 2)]
        );
        assert!(!partial[1].is_complete());
        assert!(languages_of(&presets, &CodePointSet::default(), &[]).is_empty());
    }

    #[test]
    fn locl_matches_language_and_script() {
        let presets = [
            preset("sr-Cyrl", "Cyrl", Some("SRB"), "бв"),
            preset("sr-Latn", "Latn", Some("SRB"), "ab"),
            preset("hi", "Deva", Some("HIN"), "क"),
            preset("en", "Latn", None, "ab"),
        ];
        let locl = ["cyrl/SRB".to_owned(), "dev2/HIN".to_owned()];
        let got = languages_of(&presets, &set("ab"), &locl);
        let locl_of = |code: &str| got.iter().find(|l| l.code == code).map(|l| l.has_locl);
        assert_eq!(
            locl_of("sr-Cyrl"),
            Some(true),
            "a locl row even with nothing covered"
        );
        assert_eq!(
            locl_of("sr-Latn"),
            Some(false),
            "SRB under cyrl is not Serbian Latin"
        );
        assert_eq!(locl_of("hi"), Some(true), "dev2 is a Devanagari tag");
        assert_eq!(locl_of("en"), Some(false), "no OpenType language, no locl");
    }

    #[test]
    fn the_embedded_presets_load() {
        assert!(PRESETS.len() >= 50);
        let got = languages(&set("Serbia"), &[]);
        assert!(got.iter().any(|l| l.code == "en" && l.covered == 6));
    }

    fn codes(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|&c| c.to_owned()).collect()
    }

    #[test]
    fn a_need_for_languages_asks_for_every_preset() {
        let presets = [
            preset("xa", "Latn", None, "abc"),
            preset("xb", "Latn", None, "cdž"),
        ];
        let both = Need::languages_of(&presets, &codes(&["xa", "xb"])).expect("known");
        let one = Need::languages_of(&presets, &codes(&["xa"])).expect("known");
        let face = set("abcd");
        assert_eq!(one.check(&face, &[]).missing, 0, "xa alone is covered");
        let fit = both.check(&face, &[]);
        assert!(fit.passes());
        assert_eq!((fit.missing, fit.chars), (1, vec!['ž']), "AND: xb's ž too");
        let err = Need::languages_of(&presets, &codes(&["xa", "zz"])).expect_err("unknown");
        assert_eq!(err.code(), fm_api::ErrorCode::InvalidRequest);
        assert!(err.message().contains("zz"));
        assert!(Need::languages_of(&presets, &[]).expect("none").is_empty());
    }

    #[test]
    fn a_near_miss_lists_its_exact_characters_up_to_five() {
        let need = Need::text("abcdefghij");
        let five = need.check(&set("abcde"), &[]);
        assert_eq!(five.missing, 5);
        assert_eq!(five.chars, vec!['f', 'g', 'h', 'i', 'j']);
        assert!(five.passes());
        let six = need.check(&set("abcd"), &[]);
        assert_eq!(six.missing, 6);
        assert!(six.chars.is_empty(), "past a near miss: no list");
        assert!(!six.passes());
        let full = need.check(&set("abcdefghijk"), &[]);
        assert_eq!((full.missing, full.chars.is_empty()), (0, true));
    }

    #[test]
    fn pasted_text_skips_characters_without_glyphs() {
        let need = Need::text("Š Đ\n\tš\u{200D}đ\u{FE0F}\u{AD}Š");
        let fit = need.check(&CodePointSet::default(), &[]);
        assert_eq!(fit.chars, vec!['Đ', 'đ', 'Š', 'š'], "once each, ascending");
        assert!(Need::text(" \n\u{200B}").is_empty());
        assert!(
            !need.check(&set("ŠšĐđ"), &[]).has_locl,
            "text has no language"
        );
    }

    #[test]
    fn locl_is_reported_for_any_chosen_language() {
        let presets = [
            preset("sr-Cyrl", "Cyrl", Some("SRB"), "бв"),
            preset("en", "Latn", None, "ab"),
        ];
        let locl = ["cyrl/SRB".to_owned()];
        let face = set("абвг");
        let sr = Need::languages_of(&presets, &codes(&["sr-Cyrl"])).expect("known");
        assert!(sr.check(&face, &locl).has_locl);
        assert!(!sr.check(&face, &[]).has_locl);
        let mixed = Need::languages_of(&presets, &codes(&["en", "sr-Cyrl"])).expect("known");
        assert!(mixed.check(&face, &locl).has_locl);
        let en = Need::languages_of(&presets, &codes(&["en"])).expect("known");
        assert!(!en.check(&face, &locl).has_locl);
    }

    #[test]
    fn the_embedded_presets_make_a_need() {
        let need = Need::languages(&codes(&["sr-Cyrl", "sr-Latn"])).expect("known");
        assert!(!need.is_empty());
        assert!(Need::languages(&codes(&["xx-Nope"])).is_err());
    }
}
