use std::path::PathBuf;

use fm_api::common::FontFormat;
use read_fonts::types::Tag;
use skrifa::MetadataProvider;

use super::*;

fn read(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel);
    std::fs::read(path).expect("fixture")
}

/// `liga` makes `ﬁ`; lower-case letters have `smcp` and `ss01` forms.
const EPILOGUE: &str = "google-repo/epilogue/Epilogue[wght].ttf";
/// `ﬁ` itself has a `ss01` alternate.
const MONTSERRAT: &str = "fontshare/montserrat/Montserrat-LightItalic.woff2";
/// `ccmp` decomposes `ﬁ` into `f` and `i`.
const CALSANS: &str = "google-repo/calsans/CalSans-Regular.ttf";
/// No GSUB table.
const BASIC: &str = "google-repo/basic/Basic-Regular.ttf";

fn sfnt(woff2: &[u8]) -> Vec<u8> {
    crate::parser::decompress_web(woff2, FontFormat::Woff2).expect("WOFF2")
}

fn features_of(data: &[u8], c: char) -> GlyphFeatures {
    let font = FontRef::new(data).expect("font");
    let gid = font.charmap().map(c).expect("mapped");
    glyph_features(&font, gid)
}

fn tags(features: &[FeatureInfo]) -> Vec<&str> {
    features.iter().map(|f| f.tag.as_str()).collect()
}

#[test]
fn liga_produces_the_fi_ligature_and_replaces_its_components() {
    let data = read(EPILOGUE);
    let fi = features_of(&data, '\u{FB01}');
    assert_eq!(tags(&fi.produced_by), ["liga"]);
    assert_eq!(
        fi.produced_by[0].name.as_deref(),
        Some("Standard ligatures")
    );
    assert!(fi.replaced_by.is_empty(), "{:?}", fi.replaced_by);
    for c in ['f', 'i'] {
        let component = features_of(&data, c);
        assert!(tags(&component.replaced_by).contains(&"liga"), "{c}");
        assert!(!tags(&component.produced_by).contains(&"liga"), "{c}");
    }
}

#[test]
fn small_caps_and_stylistic_sets_replace_a_letter_with_friendly_names() {
    let a = features_of(&read(EPILOGUE), 'a');
    assert_eq!(
        tags(&a.replaced_by),
        ["aalt", "ordn", "salt", "smcp", "ss01"]
    );
    let named: Vec<_> = a
        .replaced_by
        .iter()
        .map(|f| f.name.as_deref().unwrap_or_default())
        .collect();
    assert!(named.contains(&"Small capitals"), "{named:?}");
    assert!(named.contains(&"Stylistic set 1"), "{named:?}");
    assert!(a.produced_by.is_empty(), "{:?}", a.produced_by);
}

#[test]
fn an_alternate_of_a_ligature_replaces_it() {
    let fi = features_of(&sfnt(&read(MONTSERRAT)), '\u{FB01}');
    assert_eq!(tags(&fi.produced_by), ["liga"]);
    assert_eq!(tags(&fi.replaced_by), ["aalt", "salt", "ss01"]);
}

#[test]
fn a_decomposition_replaces_the_ligature_and_produces_its_parts() {
    let data = read(CALSANS);
    assert!(tags(&features_of(&data, '\u{FB01}').replaced_by).contains(&"ccmp"));
    let f = features_of(&data, 'f');
    assert_eq!(tags(&f.produced_by), ["ccmp"]);
    assert!(f.produced_by[0].automatic, "ccmp is applied by the shaper");
}

#[test]
fn a_font_without_gsub_has_no_features() {
    let data = read(BASIC);
    let font = FontRef::new(&data).expect("font");
    assert!(font.gsub().is_err());
    assert_eq!(features_of(&data, '\u{FB01}'), GlyphFeatures::default());
}

#[test]
fn a_glyph_id_past_16_bits_is_in_no_lookup() {
    let data = read(EPILOGUE);
    let font = FontRef::new(&data).expect("font");
    assert_eq!(
        glyph_features(&font, GlyphId::new(0x1_0000)),
        GlyphFeatures::default()
    );
}

/// The offset of `tag`'s table in `data`.
fn table(data: &[u8], tag: &[u8; 4]) -> usize {
    FontRef::new(data)
        .expect("font")
        .table_directory()
        .table_records()
        .iter()
        .find(|r| r.tag() == Tag::new(tag))
        .expect("table")
        .offset() as usize
}

fn put_u16(data: &mut [u8], at: usize, value: u16) {
    data[at..at + 2].copy_from_slice(&value.to_be_bytes());
}

fn u16_at(data: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([data[at], data[at + 1]]))
}

#[test]
fn a_malformed_lookup_list_gives_empty_lists() {
    let mut data = read(EPILOGUE);
    let gsub = table(&data, b"GSUB");
    // The lookup list offset points past the font.
    put_u16(&mut data, gsub + 8, u16::MAX);
    assert_eq!(features_of(&data, '\u{FB01}'), GlyphFeatures::default());
    assert_eq!(features_of(&data, 'a'), GlyphFeatures::default());
}

#[test]
fn a_malformed_lookup_gives_empty_lists() {
    let mut data = read(EPILOGUE);
    let gsub = table(&data, b"GSUB");
    let list = gsub + u16_at(&data, gsub + 8);
    // Every lookup's first subtable offset points past the font.
    for i in 0..u16_at(&data, list) {
        let lookup = list + u16_at(&data, list + 2 + 2 * i);
        put_u16(&mut data, lookup + 6, u16::MAX);
    }
    assert_eq!(features_of(&data, '\u{FB01}'), GlyphFeatures::default());
}

#[test]
fn a_malformed_feature_list_gives_empty_lists() {
    let mut data = read(EPILOGUE);
    let gsub = table(&data, b"GSUB");
    put_u16(&mut data, gsub + 6, u16::MAX);
    assert_eq!(features_of(&data, 'a'), GlyphFeatures::default());
}

#[test]
fn a_lookup_that_nests_itself_is_walked_once() {
    let mut data = read(EPILOGUE);
    let gsub = table(&data, b"GSUB");
    let list = gsub + u16_at(&data, gsub + 8);
    // Turn lookup 0 into a contextual format 3 lookup applying lookup 0.
    let lookup = list + u16_at(&data, list + 2);
    let sub = lookup + u16_at(&data, lookup + 6);
    put_u16(&mut data, lookup, 5);
    put_u16(&mut data, lookup + 4, 1);
    // Format 3, one glyph, one lookup record, the coverage at offset 0 of
    // itself is nonsense but the lookup record is what is walked.
    for (i, v) in [3u16, 1, 1, 10, 0, 0].into_iter().enumerate() {
        put_u16(&mut data, sub + 2 * i, v);
    }
    for c in ['a', 'f', '\u{FB01}'] {
        // No hang, no panic; the rest of the lookups still count.
        let _ = features_of(&data, c);
    }
}

#[test]
fn damaged_gsub_bytes_never_panic() {
    let clean = read(EPILOGUE);
    let gsub = table(&clean, b"GSUB");
    let font = FontRef::new(&clean).expect("font");
    let len = font
        .table_directory()
        .table_records()
        .iter()
        .find(|r| r.tag() == Tag::new(b"GSUB"))
        .expect("GSUB")
        .length() as usize;
    // A fixed pseudo-random walk: deterministic, no new dependency.
    let mut seed = 0x2545_F491_u32;
    for _ in 0..200 {
        let mut data = clean.clone();
        for _ in 0..8 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let at = gsub + (seed as usize % len);
            data[at] = (seed >> 24) as u8;
        }
        for c in ['a', 'f', '\u{FB01}'] {
            let _ = features_of(&data, c);
        }
    }
}
