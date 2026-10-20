use std::path::PathBuf;

use fm_api::common::AxisValue;
use fm_api::{ErrorCode, FaceId};
use read_fonts::TableProvider;
use read_fonts::tables::glyf::Glyph;
use skrifa::GlyphId;
use skrifa::instance::LocationRef;
use skrifa::metrics::BoundingBox;
use skrifa::outline::OutlineGlyphFormat;

use super::*;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).expect("fixture")
}

/// Static TrueType.
const ROBOTO_SLAB: &str = "google-static/robotoslab/RobotoSlab-ExtraBold.ttf";
/// CFF.
const NUNITO_OTF: &str = "fontshare/nunito/Nunito-ExtraLightItalic.otf";
/// Variable TrueType, `wght` only.
const EPILOGUE: &str = "google-repo/epilogue/Epilogue[wght].ttf";
const WOFF2: &str = "fontshare/jet-brains-mono/JetBrainsMono-Variable.woff2";

fn req(c: char, variations: &[(&str, f32)]) -> GlyphSvgRequest {
    GlyphSvgRequest {
        face: FaceId(1),
        code_point: c as u32,
        variations: variations
            .iter()
            .map(|&(tag, value)| AxisValue {
                tag: tag.into(),
                value,
            })
            .collect(),
    }
}

fn svg(data: &[u8], c: char, variations: &[(&str, f32)]) -> String {
    glyph_svg(data, 0, &req(c, variations)).expect("an svg")
}

/// The `d` attribute of the one path.
fn path(svg: &str) -> &str {
    let start = svg.find("<path d=\"").expect("a path") + "<path d=\"".len();
    &svg[start..start + svg[start..].find('"').expect("closing quote")]
}

/// The four viewBox numbers.
fn view_box(svg: &str) -> [f32; 4] {
    let start = svg.find("viewBox=\"").expect("a viewBox") + "viewBox=\"".len();
    let end = start + svg[start..].find('"').expect("closing quote");
    let v: Vec<f32> = svg[start..end]
        .split(' ')
        .map(|n| n.parse().expect("a number"))
        .collect();
    v.try_into().expect("four numbers")
}

/// Every point of the path, in SVG coordinates.
fn points(d: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = d
        .split(|c: char| c.is_ascii_alphabetic() || c == ' ')
        .filter(|s| !s.is_empty())
        .map(|n| n.parse().expect("a number"))
        .collect();
    assert_eq!(nums.len() % 2, 0, "{d}");
    nums.chunks(2).map(|p| (p[0], p[1])).collect()
}

/// The control box of the path, back in font coordinates (y up).
fn font_bounds(svg: &str, ascender: f32) -> BoundingBox {
    let pts = points(path(svg));
    let fold = |f: fn(f32, f32) -> f32, init: f32, sel: fn(&(f32, f32)) -> f32| {
        pts.iter().map(sel).fold(init, f)
    };
    BoundingBox {
        x_min: fold(f32::min, f32::MAX, |p| p.0),
        x_max: fold(f32::max, f32::MIN, |p| p.0),
        y_min: ascender - fold(f32::max, f32::MIN, |p| p.1),
        y_max: ascender - fold(f32::min, f32::MAX, |p| p.1),
    }
}

fn font(data: &[u8]) -> FontRef<'_> {
    FontRef::new(data).expect("font")
}

fn gid(font: &FontRef<'_>, c: char) -> GlyphId {
    font.charmap().map(c).expect("mapped")
}

/// The path's bounds match skrifa's bounds of the glyph, and the viewBox is
/// the advance by ascender to descender.
fn assert_matches_skrifa(data: &[u8], c: char) {
    let font = font(data);
    let gid = gid(&font, c);
    let metrics = font.metrics(Size::unscaled(), LocationRef::default());
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let expected = glyph_metrics.bounds(gid).expect("skrifa bounds");
    let svg = svg(data, c, &[]);
    let got = font_bounds(&svg, metrics.ascent);
    for (g, e) in [
        (got.x_min, expected.x_min),
        (got.x_max, expected.x_max),
        (got.y_min, expected.y_min),
        (got.y_max, expected.y_max),
    ] {
        assert!((g - e).abs() <= 0.01, "{c}: {got:?} vs {expected:?}");
    }
    let advance = glyph_metrics.advance_width(gid).expect("advance");
    assert_eq!(
        view_box(&svg),
        [0.0, 0.0, advance, metrics.ascent - metrics.descent]
    );
}

#[test]
fn a_truetype_glyph_is_quadratic_and_matches_skrifa_bounds() {
    let data = read(ROBOTO_SLAB);
    assert_eq!(
        font(&data).outline_glyphs().format(),
        Some(OutlineGlyphFormat::Glyf)
    );
    for c in ['A', 'g', 'O'] {
        assert_matches_skrifa(&data, c);
    }
    let d = path(&svg(&data, 'O', &[])).to_owned();
    assert!(d.starts_with('M') && d.ends_with('Z'), "{d}");
    assert!(d.contains('Q') && !d.contains('C'), "{d}");
}

#[test]
fn a_cff_glyph_is_cubic_and_matches_skrifa_bounds() {
    let data = read(NUNITO_OTF);
    assert_eq!(
        font(&data).outline_glyphs().format(),
        Some(OutlineGlyphFormat::Cff)
    );
    for c in ['A', 'g', 'O'] {
        assert_matches_skrifa(&data, c);
    }
    let d = path(&svg(&data, 'O', &[])).to_owned();
    assert!(d.contains('C') && !d.contains('Q'), "{d}");
}

#[test]
fn a_composite_glyph_draws_its_components() {
    let data = read(ROBOTO_SLAB);
    let f = font(&data);
    let aring = gid(&f, 'Å');
    let glyf = f.glyf().expect("glyf");
    let glyph = f.loca(None).expect("loca").get_glyf(aring, &glyf);
    assert!(
        matches!(glyph, Ok(Some(Glyph::Composite(_)))),
        "Å should be a composite in the fixture"
    );
    assert_matches_skrifa(&data, 'Å');
    // The ring sits above A: Å reaches higher and has more contours.
    let a = svg(&data, 'A', &[]);
    let ring = svg(&data, 'Å', &[]);
    let asc = f.metrics(Size::unscaled(), LocationRef::default()).ascent;
    assert!(font_bounds(&ring, asc).y_max > font_bounds(&a, asc).y_max);
    assert!(path(&ring).matches('M').count() > path(&a).matches('M').count());
}

#[test]
fn the_weight_axis_changes_the_path_and_matches_skrifa_bounds_there() {
    let data = read(EPILOGUE);
    let f = font(&data);
    let wght = f
        .axes()
        .iter()
        .find(|a| a.tag() == Tag::new(b"wght"))
        .expect("wght axis");
    let (min, max) = (wght.min_value(), wght.max_value());
    let thin = svg(&data, 'o', &[("wght", min)]);
    let black = svg(&data, 'o', &[("wght", max)]);
    assert_ne!(path(&thin), path(&black));
    assert_ne!(path(&thin), path(&svg(&data, 'o', &[])));

    // At wght max the path is skrifa's outline at wght max.
    let location = f.axes().location([("wght", max)]);
    let expected = f
        .glyph_metrics(Size::unscaled(), &location)
        .bounds(gid(&f, 'o'))
        .expect("bounds");
    let asc = f.metrics(Size::unscaled(), &location).ascent;
    let got = font_bounds(&black, asc);
    assert!(
        (got.x_max - expected.x_max).abs() <= 0.01,
        "{got:?} vs {expected:?}"
    );
    assert!(
        (got.y_min - expected.y_min).abs() <= 0.01,
        "{got:?} vs {expected:?}"
    );
    // Black `o` is wider than thin `o`.
    assert!(view_box(&black)[2] > view_box(&thin)[2]);
}

#[test]
fn an_unknown_axis_is_ignored() {
    let data = read(ROBOTO_SLAB);
    assert_eq!(svg(&data, 'A', &[("wdth", 50.0)]), svg(&data, 'A', &[]));
}

#[test]
fn a_space_is_a_valid_svg_with_an_empty_path() {
    for rel in [ROBOTO_SLAB, NUNITO_OTF] {
        let s = svg(&read(rel), ' ', &[]);
        assert!(
            s.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
            "{s}"
        );
        assert!(s.ends_with("<path d=\"\"/></svg>"), "{s}");
        let [_, _, w, h] = view_box(&s);
        assert!(w > 0.0 && h > 0.0, "{s}");
    }
}

#[test]
fn a_web_font_exports_too() {
    let s = svg(&read(WOFF2), 'a', &[]);
    assert!(path(&s).starts_with('M'), "{s}");
}

#[test]
fn an_unmapped_character_is_not_found_with_a_message() {
    let err = glyph_svg(&read(ROBOTO_SLAB), 0, &req('\u{10FFFD}', &[])).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert_eq!(err.message(), "Character U+10FFFD was not found.");
}

#[test]
fn a_bad_axis_value_or_tag_is_an_invalid_request() {
    let data = read(EPILOGUE);
    for variations in [&[("wght", f32::NAN)][..], &[("weight", 400.0)]] {
        let err = glyph_svg(&data, 0, &req('o', variations)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }
}

#[test]
fn a_malformed_outline_is_an_error_not_a_panic() {
    let mut data = read(ROBOTO_SLAB);
    let (start, len) = {
        let f = font(&data);
        let a = gid(&f, 'A');
        let loca = f.loca(None).expect("loca");
        let glyf = f
            .table_directory()
            .table_records()
            .iter()
            .find(|r| r.tag() == Tag::new(b"glyf"))
            .expect("glyf")
            .offset() as usize;
        let start = loca.get_raw(a.to_u32() as usize).expect("start") as usize;
        let end = loca.get_raw(a.to_u32() as usize + 1).expect("end") as usize;
        (glyf + start, end - start)
    };
    assert!(len > 12, "A has an outline");
    // The first contour claims 65,535 points: far past the glyph's data.
    data[start + 10..start + 12].copy_from_slice(&0xFFFEu16.to_be_bytes());
    let err = glyph_svg(&data, 0, &req('A', &[])).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Unsupported);
    assert_eq!(
        err.message(),
        "This glyph cannot be exported: the font data is malformed."
    );
    // The face itself reads fine: drawing the outline is what failed.
    let detail = err.detail().expect("detail");
    assert!(
        detail.starts_with(&format!("glyph {}:", gid(&font(&data), 'A').to_u32())),
        "{detail}"
    );
}

#[test]
fn a_broken_font_is_an_error_not_a_panic() {
    for data in [
        read("derived/agdasima/agdasima-truncated.ttf"),
        b"not a font at all".to_vec(),
        b"wOF2garbage-garbage".to_vec(),
    ] {
        let err = glyph_svg(&data, 0, &req('A', &[])).unwrap_err();
        assert_eq!(err.code(), ErrorCode::Unsupported, "{err:?}");
    }
}

#[test]
fn the_svg_is_returned_and_nothing_is_written_next_to_the_font() {
    let dir = crate::test_support::TempDir::new("glyph-svg");
    let font_path = dir.path().join("RobotoSlab.ttf");
    std::fs::copy(fixture(ROBOTO_SLAB), &font_path).expect("copy");
    let s = glyph_svg_file(&font_path, 0, &req('A', &[])).expect("an svg");
    assert!(s.starts_with("<svg"));
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, ["RobotoSlab.ttf"]);
}

#[test]
fn a_missing_file_is_an_io_error() {
    let err = glyph_svg_file(&fixture("no/such.ttf"), 0, &req('A', &[])).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
}

#[test]
fn numbers_are_short() {
    assert_eq!(num(12.0), "12");
    assert_eq!(num(12.5), "12.5");
    assert_eq!(num(-0.004), "0");
    assert_eq!(num(1.234_56), "1.23");
}
