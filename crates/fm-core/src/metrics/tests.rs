use std::path::{Path, PathBuf};

use fm_api::common::Metrics;
use fm_api::preview::SizeBasis;
use read_fonts::{FileRef, FontRef, TableProvider};

use super::*;
use crate::parser;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

/// The fixture paths of `manifest.tsv`, relative to `fixtures/fonts`.
fn fixture_paths() -> Vec<String> {
    let manifest = std::fs::read_to_string(fixtures().join("manifest.tsv")).expect("manifest");
    manifest
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').nth(4))
        .map(str::to_owned)
        .collect()
}

/// Every TrueType or OpenType fixture's bytes (WOFF, WOFF2 and Type 1 left
/// out: `read` takes the sfnt the parser hands it).
fn sfnt_fixtures() -> Vec<(String, Vec<u8>)> {
    fixture_paths()
        .into_iter()
        .filter(|p| [".ttf", ".otf", ".ttc"].iter().any(|e| p.ends_with(e)))
        .filter_map(|p| Some((p.clone(), std::fs::read(fixtures().join(&p)).ok()?)))
        .collect()
}

fn fonts(data: &[u8]) -> Vec<FontRef<'_>> {
    match FileRef::new(data) {
        Ok(FileRef::Font(f)) => vec![f],
        Ok(FileRef::Collection(c)) => c.iter().filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn metrics(upem: u16, x: Option<i16>, cap: Option<i16>) -> Metrics {
    Metrics {
        units_per_em: upem,
        ascender: 0,
        descender: 0,
        x_height: x,
        cap_height: cap,
    }
}

/// Criterion 1: a face that declares its x-height and cap height in `OS/2`
/// gets exactly those; one that does not gets the top of its `x` and `H`.
#[test]
fn reads_os2_heights_and_measures_the_rest_on_fixtures() {
    let (mut declared, mut measured) = (0, 0);
    for (path, data) in sfnt_fixtures() {
        for font in fonts(&data) {
            let m = read(&font);
            let os2 = font.os2().ok();
            let checks = [
                ('x', m.x_height, os2.as_ref().and_then(|o| o.sx_height())),
                (
                    'H',
                    m.cap_height,
                    os2.as_ref().and_then(|o| o.s_cap_height()),
                ),
            ];
            for (c, got, os2_value) in checks {
                match os2_value.filter(|&v| v > 0) {
                    Some(v) => {
                        declared += 1;
                        assert_eq!(got, Some(v), "{path}: {c} as declared");
                    }
                    None => {
                        measured += 1;
                        assert_eq!(got, outline_top(&font, c), "{path}: {c} measured");
                    }
                }
            }
            assert_eq!(
                m.units_per_em,
                font.head().unwrap().units_per_em(),
                "{path}"
            );
        }
    }
    assert!(declared > 100, "{declared} declared heights checked");
    assert!(measured > 0, "{measured} measured heights checked");
}

/// Criterion 1: the outline measure agrees with what fonts declare: on the
/// fixtures that declare both, the top of `x` and `H` is within 3 % of the
/// em of `OS/2` for at least 90 % of the heights (overshoot and rounding are
/// smaller than that; some fonts declare another script's heights, such as
/// Karma's Devanagari x-height, or values that fit no letter).
#[test]
fn measured_heights_match_declared_ones_on_fixtures() {
    let (mut compared, mut close) = (0_u32, 0_u32);
    let mut far = Vec::new();
    for (path, data) in sfnt_fixtures() {
        for font in fonts(&data) {
            let Ok(os2) = font.os2() else { continue };
            let upem = f32::from(font.head().unwrap().units_per_em());
            for (c, v) in [('x', os2.sx_height()), ('H', os2.s_cap_height())] {
                let (Some(v), Some(top)) = (v.filter(|&v| v > 0), outline_top(&font, c)) else {
                    continue;
                };
                compared += 1;
                let off = (f32::from(top) - f32::from(v)).abs() / upem;
                if off <= 0.03 {
                    close += 1;
                } else {
                    far.push(format!("{path}: {c} top {top}, OS/2 {v}"));
                }
            }
        }
    }
    assert!(compared > 100, "{compared} heights compared");
    assert!(
        close * 100 >= compared * 90,
        "{close}/{compared} close; far: {far:#?}"
    );
}

/// Criterion 1: the scan stores the metrics on the parsed face (and with it
/// `face.metrics`), the same as reading the font directly.
#[test]
fn parse_carries_the_metrics() {
    let path = fixtures().join("google-static/robotoslab/RobotoSlab-ExtraBold.ttf");
    let parsed = parser::parse_file(&path).unwrap();
    let data = std::fs::read(&path).unwrap();
    let font = FontRef::new(&data).unwrap();
    assert_eq!(parsed.faces[0].metrics, read(&font));
    assert!(parsed.faces[0].metrics.x_height.is_some());
}

/// Criterion 3: at every toolbar size, every fixture face with a usable
/// metric has the same x-height (and cap height) in pixels within 1 px.
#[test]
fn auto_size_matches_within_one_px_on_fixtures() {
    let all: Vec<Metrics> = fixture_paths()
        .iter()
        .filter_map(|p| parser::parse_file(&fixtures().join(p)).ok())
        .flat_map(|f| f.faces)
        .map(|f| f.metrics)
        .collect();
    for basis in [SizeBasis::XHeight, SizeBasis::CapHeight] {
        let value = |m: &Metrics| match basis {
            SizeBasis::XHeight => m.x_height,
            SizeBasis::CapHeight => m.cap_height,
        };
        for size in [8.0_f32, 28.0, 72.0, 300.0] {
            let px: Vec<f32> = all
                .iter()
                .filter(|m| size_factor(m, basis) != 1.0)
                .map(|m| {
                    let em = f32::from(value(m).unwrap()) / f32::from(m.units_per_em);
                    size * size_factor(m, basis) * em
                })
                .collect();
            assert!(px.len() > 100, "{basis:?}: {} faces scaled", px.len());
            let lo = px.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = px.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(hi - lo <= 1.0, "{basis:?} at {size}px: {lo}..{hi}");
        }
    }
}

/// Criterion 3: a face with no usable metric keeps factor 1: none, zero,
/// no units per em, or outside the usable range.
#[test]
fn unusable_metric_keeps_factor_one() {
    let x = SizeBasis::XHeight;
    let cap = SizeBasis::CapHeight;
    assert_eq!(size_factor(&metrics(1000, None, None), x), 1.0);
    assert_eq!(size_factor(&metrics(1000, None, None), cap), 1.0);
    assert_eq!(size_factor(&metrics(1000, Some(0), Some(0)), x), 1.0);
    assert_eq!(
        size_factor(&metrics(1000, Some(-400), Some(-700)), cap),
        1.0
    );
    assert_eq!(size_factor(&metrics(0, Some(500), Some(700)), x), 1.0);
    assert_eq!(size_factor(&metrics(1000, Some(50), Some(50)), x), 1.0);
    assert_eq!(
        size_factor(&metrics(1000, Some(5000), Some(5000)), cap),
        1.0
    );
    // A broken fixture parses to a face without metrics: factor 1.
    let broken = parser::parse_file(&fixtures().join("derived/agdasima/agdasima-truncated.ttf"));
    if let Ok(file) = broken {
        for face in file.faces {
            assert_eq!(size_factor(&face.metrics, x), 1.0);
        }
    }
}

/// Criterion 2: the factor brings the face to the target, per basis.
#[test]
fn factor_scales_to_the_target() {
    let m = metrics(2000, Some(1000), Some(1750));
    assert!((size_factor(&m, SizeBasis::XHeight) - X_HEIGHT_EM / 0.5).abs() < 1e-6);
    assert!((size_factor(&m, SizeBasis::CapHeight) - CAP_HEIGHT_EM / 0.875).abs() < 1e-6);
}

/// Malformed data never panics: a face whose tables are cut off reads
/// whatever it can.
#[test]
fn truncated_font_does_not_panic() {
    let data = std::fs::read(fixtures().join("google-static/robotoslab/RobotoSlab-ExtraBold.ttf"))
        .unwrap();
    for len in [64, 512, 4096, data.len() / 2] {
        if let Ok(font) = FontRef::new(&data[..len]) {
            let _ = read(&font);
        }
    }
}
