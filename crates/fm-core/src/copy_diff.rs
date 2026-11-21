//! What differs between two copies of a duplicate set (E56.2, ADR 0027).
//!
//! [`compare`] reads both files on demand and parses them with fontations.
//! It answers in the words a user reads ([`CopyComparison::summary`]) and in
//! fields the UI lays out ([`CopyDifferences`]):
//!
//! - **Languages** gained and lost: the presets of `data/charsets` one copy
//!   covers in full and the other does not.
//! - **Glyphs** added and removed. Glyph ids are not stable between versions
//!   of a font, so a glyph is known by the character it maps (`cmap`), else
//!   by its glyph name (`post`, `CFF`); a glyph with neither only counts in
//!   the glyph total.
//! - **Features** and **axes** added and removed, by tag.
//! - **Metrics**: the vertical metrics (`hhea`, `OS/2` typographic and
//!   Windows) and the advance widths of the glyphs both copies have, each
//!   compared per em, and the units per em. A change in the vertical metrics
//!   or the advance widths means text set in one copy may reflow in the
//!   other.
//! - **Container only**: every table holds the same data (the outlines
//!   compared glyph by glyph when `glyf` is stored differently, as WOFF2
//!   does), so only the file format differs.
//!
//! A file that cannot be read gives [`Health::Broken`] for its side and no
//! differences; nothing here panics on malformed data.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use fm_api::FaceId;
use fm_api::common::{FontFormat, Health};
use fm_api::library::{ComparedCopy, CopyComparison, CopyDifferences, GlyphChange};
use read_fonts::{FontRef, TableProvider, types::Tag};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, GlyphNames, MetadataProvider};

use crate::coverage::{self, CodePointSet};
use crate::parser::{self, ParsedFace};

/// How many glyphs of each change a comparison names.
pub const FIRST_GLYPHS: usize = 10;

/// One copy to compare: a face of the library and where its file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The face.
    pub face: FaceId,
    /// Its file.
    pub path: PathBuf,
    /// Its index in a collection file; 0 otherwise.
    pub collection_index: u32,
}

/// Compares the copies `a` and `b`; the differences read from `a` to `b`.
pub fn compare(a: &Target, b: &Target) -> CopyComparison {
    let read_a = read(a);
    let read_b = read(b);
    let side = |t: &Target, r: &Read| {
        let (full, locl) = r
            .font
            .as_ref()
            .map(|l| {
                let langs = languages(&l.parsed);
                (langs.complete, langs.locl)
            })
            .unwrap_or_default();
        ComparedCopy {
            face: t.face,
            path: t.path.to_string_lossy().into_owned(),
            format: r.format,
            health: r.font.as_ref().err().cloned().unwrap_or(Health::Ok),
            languages: full,
            locl,
        }
    };
    let (sa, sb) = (side(a, &read_a), side(b, &read_b));
    let (differences, summary) = match (&read_a.font, &read_b.font) {
        (Ok(fa), Ok(fb)) => match (fa.font(), fb.font()) {
            (Ok(ra), Ok(rb)) => {
                let d = differences(fa, &ra, fb, &rb);
                let summary = summary(&d, &fa.parsed, &fb.parsed, read_a.format, read_b.format);
                (Some(d), summary)
            }
            // Parsed once already, so this does not happen; kept as an answer.
            (Err(e), _) | (_, Err(e)) => (None, vec![format!("A copy could not be read: {e}.")]),
        },
        _ => (None, unreadable(&sa, &sb)),
    };
    CopyComparison {
        a: sa,
        b: sb,
        differences,
        summary,
    }
}

/// The sentences for a comparison with a side that could not be read.
fn unreadable(a: &ComparedCopy, b: &ComparedCopy) -> Vec<String> {
    [("first", a), ("second", b)]
        .into_iter()
        .filter_map(|(which, side)| match &side.health {
            Health::Ok => None,
            Health::Broken(reason) | Health::Unsupported(reason) => Some(format!(
                "The {which} copy ({}) could not be read, so the copies are not compared: {reason}",
                side.path
            )),
        })
        .collect()
}

/// A file read for a comparison: its format, and the face or why it could
/// not be read.
struct Read {
    format: Option<FontFormat>,
    font: Result<Loaded, Health>,
}

/// One face, parsed, with the TrueType or OpenType data it was parsed from.
struct Loaded {
    sfnt: Vec<u8>,
    index: u32,
    parsed: ParsedFace,
}

impl Loaded {
    fn font(&self) -> Result<FontRef<'_>, read_fonts::ReadError> {
        FontRef::from_index(&self.sfnt, self.index)
    }
}

fn read(target: &Target) -> Read {
    let data = match fs::read(&target.path) {
        Ok(data) => data,
        Err(e) => {
            return Read {
                format: None,
                font: Err(Health::Broken(format!("The file could not be read: {e}."))),
            };
        }
    };
    let (file, sfnt) = parser::parse_keeping_sfnt(&data);
    let format = file.format;
    let font = match file
        .faces
        .into_iter()
        .find(|f| f.index == target.collection_index)
    {
        None => match file.health {
            Health::Ok => Err(Health::Broken(format!(
                "The file holds no face {}.",
                target.collection_index
            ))),
            other => Err(other),
        },
        Some(face) if face.health != Health::Ok => Err(face.health),
        Some(parsed) => {
            let sfnt = sfnt.into_owned();
            // A WOFF holding a collection keeps its faces' indices.
            let index = if matches!(sfnt.get(..4), Some(b"ttcf")) {
                target.collection_index
            } else {
                0
            };
            let loaded = Loaded {
                sfnt,
                index,
                parsed,
            };
            match loaded.font() {
                Ok(_) => Ok(loaded),
                Err(e) => Err(Health::Broken(format!("The font is malformed: {e}."))),
            }
        }
    };
    Read { format, font }
}

/// How a glyph is known across two versions of a font: the character it
/// maps, else its name. Characters sort first, in Unicode order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum GlyphKey {
    Char(u32),
    Name(String),
}

impl GlyphKey {
    fn label(&self) -> String {
        match self {
            GlyphKey::Char(cp) => match char::from_u32(*cp).filter(|c| !c.is_control()) {
                Some(c) => format!("{c} (U+{cp:04X})"),
                None => format!("U+{cp:04X}"),
            },
            GlyphKey::Name(name) => name.clone(),
        }
    }

    /// The character the glyph maps; `None` for a glyph known by name.
    fn code_point(&self) -> Option<u32> {
        match self {
            GlyphKey::Char(cp) => Some(*cp),
            GlyphKey::Name(_) => None,
        }
    }
}

/// The presets a face covers in full and those it has `locl` forms for,
/// each in preset order.
struct Languages {
    complete: Vec<String>,
    locl: Vec<String>,
}

fn languages(parsed: &ParsedFace) -> Languages {
    let set = CodePointSet::from_code_points(parsed.code_points.iter().copied());
    let all = coverage::languages(&set, &parsed.locl_languages);
    Languages {
        complete: all
            .iter()
            .filter(|l| l.is_complete())
            .map(|l| l.code.clone())
            .collect(),
        locl: all
            .iter()
            .filter(|l| l.has_locl)
            .map(|l| l.code.clone())
            .collect(),
    }
}

/// What one face holds that a comparison looks at.
struct Facts {
    glyphs: BTreeMap<GlyphKey, GlyphId>,
    /// The name of every glyph that has one, mapped or not: a glyph known
    /// by its name here may be mapped in the other copy.
    names: BTreeSet<String>,
    glyph_count: u32,
    upem: u16,
    /// hhea ascender, descender, line gap; OS/2 typographic ascender,
    /// descender, line gap; Windows ascent and descent. `None` for a value
    /// of a table the font lacks.
    vertical: [Option<i32>; 8],
    advances: BTreeMap<GlyphId, f32>,
    complete: BTreeSet<String>,
    locl: BTreeSet<String>,
}

fn facts(font: &FontRef<'_>, parsed: &ParsedFace) -> Facts {
    let mut glyphs = BTreeMap::new();
    let mut mapped = BTreeSet::new();
    for (cp, gid) in font.charmap().mappings() {
        glyphs.entry(GlyphKey::Char(cp)).or_insert(gid);
        mapped.insert(gid);
    }
    let glyph_count = font.maxp().map(|m| u32::from(m.num_glyphs())).unwrap_or(0);
    let mut names = BTreeSet::new();
    for (gid, name) in GlyphNames::new(font).iter() {
        if name.is_synthesized() {
            continue;
        }
        if !mapped.contains(&gid) {
            glyphs
                .entry(GlyphKey::Name(name.as_str().to_owned()))
                .or_insert(gid);
        }
        names.insert(name.as_str().to_owned());
    }
    let metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let advances = glyphs
        .values()
        .filter_map(|&gid| metrics.advance_width(gid).map(|w| (gid, w)))
        .collect();
    let hhea = font.hhea().ok();
    let os2 = font.os2().ok();
    let vertical = [
        hhea.as_ref().map(|h| i32::from(h.ascender().to_i16())),
        hhea.as_ref().map(|h| i32::from(h.descender().to_i16())),
        hhea.as_ref().map(|h| i32::from(h.line_gap().to_i16())),
        os2.as_ref().map(|o| i32::from(o.s_typo_ascender())),
        os2.as_ref().map(|o| i32::from(o.s_typo_descender())),
        os2.as_ref().map(|o| i32::from(o.s_typo_line_gap())),
        os2.as_ref().map(|o| i32::from(o.us_win_ascent())),
        os2.as_ref().map(|o| i32::from(o.us_win_descent())),
    ];
    let langs = languages(parsed);
    Facts {
        glyphs,
        names,
        glyph_count,
        upem: font.head().map(|h| h.units_per_em()).unwrap_or(0),
        vertical,
        advances,
        complete: langs.complete.into_iter().collect(),
        locl: langs.locl.into_iter().collect(),
    }
}

/// `x` of a font with `x_upem` and `y` of one with `y_upem`, compared per em.
fn same_per_em(x: f64, x_upem: u16, y: f64, y_upem: u16) -> bool {
    (x * f64::from(y_upem) - y * f64::from(x_upem)).abs() < 1e-6
}

fn differences(la: &Loaded, fa: &FontRef<'_>, lb: &Loaded, fb: &FontRef<'_>) -> CopyDifferences {
    let a = facts(fa, &la.parsed);
    let b = facts(fb, &lb.parsed);
    let only = |x: &Facts, y: &Facts| -> GlyphChange {
        // A glyph known by name is the other copy's when that copy has a
        // glyph of the name, mapped or not.
        let keys: Vec<&GlyphKey> = x
            .glyphs
            .keys()
            .filter(|k| match k {
                GlyphKey::Char(_) => !y.glyphs.contains_key(*k),
                GlyphKey::Name(name) => !y.names.contains(name),
            })
            .collect();
        GlyphChange {
            count: u32::try_from(keys.len()).unwrap_or(u32::MAX),
            first: keys.iter().take(FIRST_GLYPHS).map(|k| k.label()).collect(),
            code_points: keys
                .iter()
                .take(FIRST_GLYPHS)
                .map(|k| k.code_point())
                .collect(),
        }
    };
    let tags = |x: &[String], y: &[String]| -> Vec<String> {
        let mut out: Vec<String> = x.iter().filter(|t| !y.contains(t)).cloned().collect();
        out.dedup();
        out
    };
    let axes = |p: &ParsedFace| p.axes.iter().map(|x| x.tag.clone()).collect::<Vec<_>>();
    let vertical_metrics_changed = a
        .vertical
        .iter()
        .zip(&b.vertical)
        .any(|(x, y)| match (x, y) {
            (Some(x), Some(y)) => !same_per_em(f64::from(*x), a.upem, f64::from(*y), b.upem),
            (None, None) => false,
            _ => true,
        });
    let advance_widths_changed = a
        .glyphs
        .iter()
        .filter_map(|(key, ga)| Some((a.advances.get(ga)?, b.advances.get(b.glyphs.get(key)?)?)))
        .filter(|(x, y)| !same_per_em(f64::from(**x), a.upem, f64::from(**y), b.upem))
        .count();
    let advance_widths_changed = u32::try_from(advance_widths_changed).unwrap_or(u32::MAX);
    let tables_changed = tables_changed(fa, fb);
    let container_only =
        tables_changed.is_empty() && la.sfnt != lb.sfnt && a.glyph_count == b.glyph_count;
    CopyDifferences {
        languages_gained: b.complete.difference(&a.complete).cloned().collect(),
        languages_lost: a.complete.difference(&b.complete).cloned().collect(),
        locl_gained: b.locl.difference(&a.locl).cloned().collect(),
        locl_lost: a.locl.difference(&b.locl).cloned().collect(),
        glyphs_added: only(&b, &a),
        glyphs_removed: only(&a, &b),
        features_added: tags(&lb.parsed.features, &la.parsed.features),
        features_removed: tags(&la.parsed.features, &lb.parsed.features),
        axes_added: tags(&axes(&lb.parsed), &axes(&la.parsed)),
        axes_removed: tags(&axes(&la.parsed), &axes(&lb.parsed)),
        vertical_metrics_changed,
        units_per_em_changed: a.upem != b.upem,
        advance_widths_changed,
        may_reflow: vertical_metrics_changed || advance_widths_changed > 0,
        container_only,
        tables_changed,
    }
}

/// Tables a container may rewrite without changing the font: a signature,
/// and `loca`, which follows `glyf` (compared glyph by glyph).
const CONTAINER_TABLES: [&[u8; 4]; 2] = [b"DSIG", b"loca"];

/// The tables whose data differs between `a` and `b`, by tag. `head` is
/// compared without its checksum adjustment and the WOFF2 "transformed" flag;
/// `glyf`, when its bytes differ, by drawing every glyph.
fn tables_changed(a: &FontRef<'_>, b: &FontRef<'_>) -> Vec<String> {
    let tags = |f: &FontRef<'_>| -> BTreeSet<Tag> {
        f.table_directory()
            .table_records()
            .iter()
            .map(|r| r.tag())
            .filter(|t| !CONTAINER_TABLES.iter().any(|c| t.to_be_bytes() == **c))
            .collect()
    };
    let all: BTreeSet<Tag> = tags(a).union(&tags(b)).copied().collect();
    all.into_iter()
        .filter(|&tag| {
            let x = a.table_data(tag).map(|d| d.as_bytes());
            let y = b.table_data(tag).map(|d| d.as_bytes());
            match (x, y) {
                (Some(x), Some(y)) if tag == Tag::new(b"head") => head_key(x) != head_key(y),
                (Some(x), Some(y)) if tag == Tag::new(b"glyf") => x != y && !same_outlines(a, b),
                (x, y) => x != y,
            }
        })
        .map(|tag| tag.to_string().trim_end().to_owned())
        .collect()
}

/// `head` without `checkSumAdjustment` (bytes 8–11) and bit 11 of `flags`
/// (bytes 16–17), which WOFF2 sets when it transforms `glyf`.
fn head_key(head: &[u8]) -> Vec<u8> {
    let mut key = head.to_vec();
    if let Some(sum) = key.get_mut(8..12) {
        sum.fill(0);
    }
    if let Some(flags) = key.get_mut(16) {
        *flags &= !0x08;
    }
    key
}

/// Records the path commands of a glyph exactly.
#[derive(Default, PartialEq)]
struct Commands(Vec<u32>);

impl Commands {
    fn push(&mut self, op: u32, points: &[f32]) {
        self.0.push(op);
        self.0.extend(points.iter().map(|p| p.to_bits()));
    }
}

impl OutlinePen for Commands {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push(0, &[x, y]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(1, &[x, y]);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.push(2, &[cx0, cy0, x, y]);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.push(3, &[cx0, cy0, cx1, cy1, x, y]);
    }
    fn close(&mut self) {
        self.push(4, &[]);
    }
}

/// Whether every glyph of `a` draws exactly as the glyph with its id in `b`.
fn same_outlines(a: &FontRef<'_>, b: &FontRef<'_>) -> bool {
    let (oa, ob) = (a.outline_glyphs(), b.outline_glyphs());
    let count = |f: &FontRef<'_>| f.maxp().map(|m| m.num_glyphs()).ok();
    let Some(n) = count(a).filter(|n| Some(*n) == count(b)) else {
        return false;
    };
    (0..u32::from(n)).all(|gid| {
        let gid = GlyphId::new(gid);
        let draw = |o: &skrifa::outline::OutlineGlyphCollection<'_>| {
            let mut pen = Commands::default();
            let drawn = o.get(gid).map(|g| {
                g.draw(
                    DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                    &mut pen,
                )
                .is_ok()
            });
            (drawn, pen)
        };
        draw(&oa) == draw(&ob)
    })
}

/// A language preset by its name ("Vietnamese"), else its code.
fn language(code: &str) -> &str {
    coverage::language_name(code).unwrap_or(code)
}

fn list(items: &[String]) -> String {
    items.join(", ")
}

fn format_name(format: Option<FontFormat>) -> &'static str {
    match format {
        Some(FontFormat::Ttf) => "TTF",
        Some(FontFormat::Otf) => "OTF",
        Some(FontFormat::Woff) => "WOFF",
        Some(FontFormat::Woff2) => "WOFF2",
        Some(FontFormat::Ttc) => "TTC",
        Some(FontFormat::Otc) => "OTC",
        Some(_) | None => "another format",
    }
}

/// The answer in sentences, most important first; the first copy is the
/// one the differences read from.
fn summary(
    d: &CopyDifferences,
    a: &ParsedFace,
    b: &ParsedFace,
    format_a: Option<FontFormat>,
    format_b: Option<FontFormat>,
) -> Vec<String> {
    let mut out = Vec::new();
    if d.container_only {
        out.push(format!(
            "Only the container differs: both files hold the same font ({} and {}).",
            format_name(format_a),
            format_name(format_b)
        ));
        return out;
    }
    let names = |codes: &[String]| {
        codes
            .iter()
            .map(|c| language(c).to_owned())
            .collect::<Vec<_>>()
    };
    if !d.languages_gained.is_empty() {
        out.push(format!(
            "The second copy covers {} in full; the first does not.",
            list(&names(&d.languages_gained))
        ));
    }
    if !d.languages_lost.is_empty() {
        out.push(format!(
            "The first copy covers {} in full; the second does not.",
            list(&names(&d.languages_lost))
        ));
    }
    if !d.locl_gained.is_empty() {
        out.push(format!(
            "Only the second copy has localized forms (locl) for {}.",
            list(&names(&d.locl_gained))
        ));
    }
    if !d.locl_lost.is_empty() {
        out.push(format!(
            "Only the first copy has localized forms (locl) for {}.",
            list(&names(&d.locl_lost))
        ));
    }
    let glyphs = |c: &GlyphChange, verb: &str| {
        let noun = if c.count == 1 { "glyph" } else { "glyphs" };
        format!(
            "The second copy {verb} {} {noun}, the first: {}.",
            c.count,
            list(&c.first)
        )
    };
    if d.glyphs_added.count > 0 {
        out.push(glyphs(&d.glyphs_added, "adds"));
    }
    if d.glyphs_removed.count > 0 {
        out.push(glyphs(&d.glyphs_removed, "lacks"));
    }
    if a.glyph_count != b.glyph_count && d.glyphs_added.count == 0 && d.glyphs_removed.count == 0 {
        out.push(format!(
            "The first copy has {} glyphs, the second {}.",
            a.glyph_count, b.glyph_count
        ));
    }
    for (items, what) in [
        (
            &d.features_added,
            "OpenType features only the second copy has",
        ),
        (
            &d.features_removed,
            "OpenType features only the first copy has",
        ),
        (&d.axes_added, "Variation axes only the second copy has"),
        (&d.axes_removed, "Variation axes only the first copy has"),
    ] {
        if !items.is_empty() {
            out.push(format!("{what}: {}.", list(items)));
        }
    }
    if d.units_per_em_changed {
        out.push(format!(
            "The units per em differ: {} in the first copy, {} in the second.",
            a.metrics.units_per_em, b.metrics.units_per_em
        ));
    }
    if d.vertical_metrics_changed {
        out.push("The vertical metrics differ, so the line spacing changes.".to_owned());
    }
    if d.advance_widths_changed > 0 {
        let noun = if d.advance_widths_changed == 1 {
            "glyph"
        } else {
            "glyphs"
        };
        out.push(format!(
            "The advance widths of {} {noun} both copies have differ.",
            d.advance_widths_changed
        ));
    }
    if d.may_reflow {
        out.push("Text set in one copy may reflow in the other.".to_owned());
    }
    if out.is_empty() {
        out.push(if d.tables_changed.is_empty() {
            "The two copies hold the same font data.".to_owned()
        } else {
            format!(
                "The glyphs, metrics, features, axes and languages are the same; other font data differs ({}).",
                list(&d.tables_changed)
            )
        });
    }
    out
}

#[cfg(test)]
mod tests;
