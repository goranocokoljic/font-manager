//! Pairs derived from `fixtures/fonts` at test time: the corpus holds no two
//! versions of one font, so an "older version" is a subset of a fixture, a
//! version with other metrics is a fixture with patched tables, and another
//! format is the fixture packed as WOFF, WOFF2 or a collection member.

use std::path::{Path, PathBuf};

use fm_api::export::WebFormat;

use super::*;
use crate::subset::{Keep, cut};
use crate::test_support::TempDir;
use crate::web_font::{self, Sfnt};

/// A TrueType fixture that covers Vietnamese, Turkish and more in full.
const FONT: &str = "google-static/plusjakartasans/PlusJakartaSans-Medium.ttf";
const TTC: &str = "derived/agdasima/agdasima.ttc";
const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

fn source(rel: &str) -> Sfnt {
    web_font::standalone(&fs::read(fixture(rel)).unwrap(), 0).unwrap()
}

fn target(face: u32, path: &Path, collection_index: u32) -> Target {
    Target {
        face: FaceId(face),
        path: path.to_owned(),
        collection_index,
    }
}

fn write(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn write_sfnt(dir: &TempDir, name: &str, sfnt: &Sfnt) -> PathBuf {
    write(dir, name, &web_font::sfnt_bytes(sfnt).unwrap())
}

/// The fixture cut down to ASCII and Latin-1: the "older version".
fn older(dir: &TempDir) -> PathBuf {
    let keep = Keep::from_ranges([(0x00, 0x7E), (0xA0, 0xFF)]);
    write_sfnt(dir, "older.ttf", &cut(&source(FONT), &keep).unwrap())
}

fn diff(c: &CopyComparison) -> &CopyDifferences {
    c.differences.as_ref().expect("both copies read")
}

fn table<'a>(sfnt: &'a mut Sfnt, tag: &[u8; 4]) -> &'a mut Vec<u8> {
    &mut sfnt
        .tables
        .iter_mut()
        .find(|t| &t.tag == tag)
        .expect("table")
        .data
}

fn add_i16(data: &mut [u8], at: usize, by: i16) {
    let v = i16::from_be_bytes([data[at], data[at + 1]]) + by;
    data[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

#[test]
fn the_newer_version_that_adds_a_language_says_which() {
    let dir = TempDir::new("copy-diff-language");
    let old = older(&dir);
    let new = fixture(FONT);
    let c = compare(&target(1, &old, 0), &target(2, &new, 0));
    assert_eq!(c.a.health, Health::Ok);
    assert_eq!(c.b.health, Health::Ok);
    let d = diff(&c);
    assert!(
        d.languages_gained.iter().any(|l| l == "vi"),
        "Vietnamese is gained: {:?}",
        d.languages_gained
    );
    assert!(d.languages_lost.is_empty(), "{:?}", d.languages_lost);
    assert!(
        c.summary
            .iter()
            .any(|s| s.contains("Vietnamese") && s.starts_with("The second copy covers")),
        "{:?}",
        c.summary
    );
    assert!(!d.container_only);
}

#[test]
fn the_newer_version_that_loses_glyphs_counts_them_and_names_the_first() {
    let dir = TempDir::new("copy-diff-glyphs");
    let old = fixture(FONT);
    let new = older(&dir);
    let c = compare(&target(1, &old, 0), &target(2, &new, 0));
    let d = diff(&c);
    assert!(d.glyphs_removed.count > 50, "{:?}", d.glyphs_removed);
    assert_eq!(d.glyphs_removed.first.len(), FIRST_GLYPHS);
    // Characters first, in Unicode order: the first past Latin-1.
    assert_eq!(d.glyphs_removed.first[0], "Ā (U+0100)");
    assert_eq!(d.glyphs_added.count, 0, "{:?}", d.glyphs_added);
    assert!(d.languages_lost.iter().any(|l| l == "vi"));
    assert!(
        c.summary.iter().any(|s| s.starts_with(&format!(
            "The second copy lacks {} glyphs, the first: Ā (U+0100)",
            d.glyphs_removed.count
        ))),
        "{:?}",
        c.summary
    );
}

#[test]
fn changed_vertical_metrics_say_text_may_reflow() {
    let dir = TempDir::new("copy-diff-metrics");
    let mut sfnt = source(FONT);
    add_i16(table(&mut sfnt, b"hhea"), 4, 120); // ascender
    let taller = write_sfnt(&dir, "taller.ttf", &sfnt);
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &taller, 0));
    let d = diff(&c);
    assert!(d.vertical_metrics_changed);
    assert!(!d.units_per_em_changed);
    assert_eq!(d.advance_widths_changed, 0);
    assert!(d.may_reflow);
    assert!(!d.container_only);
    assert_eq!(d.tables_changed, ["hhea"]);
    assert!(d.glyphs_added.count == 0 && d.glyphs_removed.count == 0);
    assert!(c.summary.iter().any(|s| s.contains("vertical metrics")));
    assert!(
        c.summary.iter().any(|s| s.contains("may reflow")),
        "{:?}",
        c.summary
    );
}

#[test]
fn changed_advance_widths_say_text_may_reflow() {
    let dir = TempDir::new("copy-diff-advances");
    let mut sfnt = source(FONT);
    let metrics = {
        let hhea = table(&mut sfnt, b"hhea");
        usize::from(u16::from_be_bytes([hhea[34], hhea[35]]))
    };
    let hmtx = table(&mut sfnt, b"hmtx");
    for i in 0..metrics {
        add_i16(hmtx, i * 4, 10);
    }
    let wider = write_sfnt(&dir, "wider.ttf", &sfnt);
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &wider, 0));
    let d = diff(&c);
    assert!(
        d.advance_widths_changed > 100,
        "{}",
        d.advance_widths_changed
    );
    assert!(!d.vertical_metrics_changed);
    assert!(d.may_reflow);
    assert!(c.summary.iter().any(|s| s.contains("advance widths")));
    assert!(c.summary.iter().any(|s| s.contains("may reflow")));
}

#[test]
fn metrics_scaled_with_the_units_per_em_are_the_same_per_em() {
    // The same font at twice the units per em, every vertical metric and
    // advance doubled: the units per em changed, nothing reflows.
    let dir = TempDir::new("copy-diff-upem");
    let mut sfnt = source(FONT);
    let double = |data: &mut Vec<u8>, at: usize| {
        let v = i16::from_be_bytes([data[at], data[at + 1]]);
        data[at..at + 2].copy_from_slice(&(v * 2).to_be_bytes());
    };
    let head = table(&mut sfnt, b"head");
    double(head, 18);
    let hhea = table(&mut sfnt, b"hhea");
    let metrics = usize::from(u16::from_be_bytes([hhea[34], hhea[35]]));
    for at in [4, 6, 8] {
        double(hhea, at);
    }
    let os2 = table(&mut sfnt, b"OS/2");
    for at in [68, 70, 72, 74, 76] {
        double(os2, at);
    }
    let hmtx = table(&mut sfnt, b"hmtx");
    for i in 0..metrics {
        double(hmtx, i * 4);
    }
    let path = write_sfnt(&dir, "double.ttf", &sfnt);
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &path, 0));
    let d = diff(&c);
    assert!(d.units_per_em_changed);
    assert!(!d.vertical_metrics_changed);
    assert_eq!(d.advance_widths_changed, 0);
    assert!(!d.may_reflow);
}

#[test]
fn a_ttf_and_its_woff_and_woff2_differ_only_in_the_container() {
    let dir = TempDir::new("copy-diff-container");
    let sfnt = source(FONT);
    let ttf = fixture(FONT);
    for (format, name, label) in [
        (WebFormat::Woff, "font.woff", "WOFF"),
        (WebFormat::Woff2, "font.woff2", "WOFF2"),
    ] {
        let web = write(&dir, name, &web_font::encode(&sfnt, format).unwrap());
        let c = compare(&target(1, &ttf, 0), &target(2, &web, 0));
        let d = diff(&c);
        assert!(d.container_only, "{label}: {:?}", d.tables_changed);
        assert!(d.tables_changed.is_empty());
        assert!(!d.may_reflow);
        assert_eq!(
            c.summary,
            [format!(
                "Only the container differs: both files hold the same font (TTF and {label})."
            )]
        );
    }
}

#[test]
fn a_collection_member_and_the_same_font_alone_differ_only_in_the_container() {
    let dir = TempDir::new("copy-diff-ttc");
    let ttc = fixture(TTC);
    let alone = write_sfnt(
        &dir,
        "alone.ttf",
        &web_font::standalone(&fs::read(&ttc).unwrap(), 1).unwrap(),
    );
    let c = compare(&target(1, &ttc, 1), &target(2, &alone, 0));
    assert!(diff(&c).container_only, "{:?}", diff(&c).tables_changed);
    // The other member is another font.
    let c = compare(&target(1, &ttc, 0), &target(2, &alone, 0));
    assert!(!diff(&c).container_only);
}

#[test]
fn the_same_file_twice_holds_the_same_font_data() {
    let dir = TempDir::new("copy-diff-same");
    let copy = write(&dir, "copy.ttf", &fs::read(fixture(FONT)).unwrap());
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &copy, 0));
    let d = diff(&c);
    assert_eq!(*d, CopyDifferences::default());
    assert_eq!(c.summary, ["The two copies hold the same font data."]);
}

#[test]
fn other_font_data_alone_is_named_by_table() {
    let dir = TempDir::new("copy-diff-tables");
    let mut sfnt = source(FONT);
    let post = table(&mut sfnt, b"post");
    post[9] ^= 1; // the underline position
    let path = write_sfnt(&dir, "post.ttf", &sfnt);
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &path, 0));
    assert_eq!(diff(&c).tables_changed, ["post"]);
    assert!(!diff(&c).container_only);
    assert!(
        c.summary[0].contains("other font data differs (post)"),
        "{:?}",
        c.summary
    );
}

#[test]
fn a_side_that_does_not_parse_is_broken_and_named() {
    let ok = fixture(FONT);
    let bad = fixture(TRUNCATED);
    let c = compare(&target(1, &ok, 0), &target(2, &bad, 0));
    assert_eq!(c.a.health, Health::Ok);
    assert!(matches!(c.b.health, Health::Broken(_)), "{:?}", c.b.health);
    assert!(c.differences.is_none());
    assert_eq!(c.summary.len(), 1);
    assert!(
        c.summary[0].starts_with("The second copy ("),
        "{:?}",
        c.summary
    );
    let c = compare(&target(2, &bad, 0), &target(1, &ok, 0));
    assert!(c.summary[0].starts_with("The first copy ("));
}

#[test]
fn a_missing_file_junk_and_a_missing_face_are_broken_never_a_panic() {
    let dir = TempDir::new("copy-diff-unreadable");
    let ok = fixture(FONT);
    let junk = write(&dir, "junk.ttf", b"\0\x01\0\0 not a font at all");
    let empty = write(&dir, "empty.otf", b"");
    let gone = dir.path().join("gone.ttf");
    for path in [&junk, &empty, &gone] {
        let c = compare(&target(1, path, 0), &target(2, &ok, 0));
        assert!(
            matches!(c.a.health, Health::Broken(_)),
            "{path:?}: {:?}",
            c.a.health
        );
        assert!(c.differences.is_none());
        assert!(c.summary[0].starts_with("The first copy"));
    }
    // A collection index the file does not have.
    let c = compare(&target(1, &fixture(TTC), 7), &target(2, &ok, 0));
    assert!(matches!(c.a.health, Health::Broken(_)));
}

#[test]
fn a_non_utf8_or_non_ascii_path_is_read() {
    let dir = TempDir::new("copy-diff-path");
    let path = write(&dir, "Schrift-ü 字.ttf", &fs::read(fixture(FONT)).unwrap());
    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &path, 0));
    assert_eq!(c.b.health, Health::Ok);
    assert!(c.b.path.ends_with("Schrift-ü 字.ttf"));
}

#[test]
fn glyph_keys_sort_characters_before_names_and_label_them() {
    assert!(GlyphKey::Char(0x10_FFFF) < GlyphKey::Name("a".into()));
    assert_eq!(GlyphKey::Char(0xE4).label(), "ä (U+00E4)");
    assert_eq!(GlyphKey::Char(0x7).label(), "U+0007");
    assert_eq!(GlyphKey::Name("f_f_i".into()).label(), "f_f_i");
}

#[test]
fn head_ignores_the_checksum_and_the_woff2_transform_flag_only() {
    let mut a = vec![0u8; 54];
    let mut b = a.clone();
    b[8..12].copy_from_slice(&[1, 2, 3, 4]);
    b[16] |= 0x08;
    assert_eq!(head_key(&a), head_key(&b));
    a[18] = 1; // units per em
    assert_ne!(head_key(&a), head_key(&b));
}

/// The variable fixture, for axis changes.
const VARIABLE: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";

fn u16_at(data: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([data[at], data[at + 1]]))
}

#[test]
fn features_added_and_removed_are_named_both_ways() {
    // The copy renames one feature of GPOS that no other feature record
    // carries: the original tag is removed, `zzzz` is added.
    let dir = TempDir::new("copy-diff-features");
    let mut sfnt = source(FONT);
    let parsed = |sfnt: &Sfnt| {
        let bytes = web_font::sfnt_bytes(sfnt).unwrap();
        parser::parse(&bytes).faces.remove(0).features
    };
    let before = parsed(&sfnt);
    let gpos = table(&mut sfnt, b"GPOS");
    let list = u16_at(gpos, 6);
    let count = u16_at(gpos, list);
    let tag_at = |i: usize| list + 2 + i * 6;
    let tags: Vec<[u8; 4]> = (0..count)
        .map(|i| gpos[tag_at(i)..tag_at(i) + 4].try_into().unwrap())
        .collect();
    let (i, renamed) = tags
        .iter()
        .enumerate()
        .find(|(_, t)| tags.iter().filter(|x| x == t).count() == 1)
        .map(|(i, t)| (i, String::from_utf8(t.to_vec()).unwrap()))
        .expect("a feature tag GPOS lists once");
    gpos[tag_at(i)..tag_at(i) + 4].copy_from_slice(b"zzzz");
    let still_there = parsed(&sfnt).contains(&renamed);
    assert!(before.contains(&renamed));
    let path = write_sfnt(&dir, "features.ttf", &sfnt);

    let c = compare(&target(1, &fixture(FONT), 0), &target(2, &path, 0));
    let d = diff(&c);
    assert_eq!(d.features_added, ["zzzz"]);
    if still_there {
        // GSUB has the tag too: nothing of it is removed.
        assert!(d.features_removed.is_empty(), "{:?}", d.features_removed);
    } else {
        assert_eq!(d.features_removed, std::slice::from_ref(&renamed));
        assert!(
            c.summary.contains(&format!(
                "OpenType features only the first copy has: {renamed}."
            )),
            "{:?}",
            c.summary
        );
    }
    assert!(
        c.summary
            .contains(&"OpenType features only the second copy has: zzzz.".to_owned()),
        "{:?}",
        c.summary
    );
    assert!(d.axes_added.is_empty() && d.axes_removed.is_empty());

    // The other way round, added and removed swap.
    let back = compare(&target(2, &path, 0), &target(1, &fixture(FONT), 0));
    assert_eq!(diff(&back).features_removed, ["zzzz"]);
    assert_eq!(diff(&back).features_added, d.features_removed);
}

#[test]
fn axes_added_and_removed_are_named_both_ways() {
    // The copy renames the first axis in `fvar`.
    let dir = TempDir::new("copy-diff-axes");
    let mut sfnt = source(VARIABLE);
    let fvar = table(&mut sfnt, b"fvar");
    let axes = u16_at(fvar, 4);
    let first = String::from_utf8(fvar[axes..axes + 4].to_vec()).unwrap();
    fvar[axes..axes + 4].copy_from_slice(b"TEST");
    let path = write_sfnt(&dir, "axes.ttf", &sfnt);

    let c = compare(&target(1, &fixture(VARIABLE), 0), &target(2, &path, 0));
    let d = diff(&c);
    assert_eq!(first, "wght");
    assert_eq!(d.axes_added, ["TEST"]);
    assert_eq!(d.axes_removed, ["wght"]);
    assert!(d.features_added.is_empty() && d.features_removed.is_empty());
    assert!(
        c.summary
            .contains(&"Variation axes only the second copy has: TEST.".to_owned()),
        "{:?}",
        c.summary
    );
    assert!(
        c.summary
            .contains(&"Variation axes only the first copy has: wght.".to_owned()),
        "{:?}",
        c.summary
    );
    let back = compare(&target(2, &path, 0), &target(1, &fixture(VARIABLE), 0));
    assert_eq!(diff(&back).axes_added, ["wght"]);
    assert_eq!(diff(&back).axes_removed, ["TEST"]);
}
