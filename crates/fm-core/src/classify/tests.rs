use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fm_api::common::{Classification, Health};

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

fn class_named(name: &str) -> Classification {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).expect("a class name")
}

/// The hand-labeled class of every fixture folder.
fn labels() -> BTreeMap<String, Classification> {
    include_str!("labels.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .map(|l| {
            let (folder, class) = l.split_once('\t').expect("folder<TAB>class");
            (folder.to_owned(), class_named(class))
        })
        .collect()
}

fn folder(path: &str) -> String {
    path.rsplit_once('/').map_or(path, |(f, _)| f).to_owned()
}

/// Every readable face of the fixtures: (path, family, guess).
fn classified() -> Vec<(String, Option<String>, Option<ClassGuess>)> {
    let mut out = Vec::new();
    for path in fixture_paths() {
        let parsed = parser::parse_file(&fixtures().join(&path)).expect("read fixture");
        for face in parsed.faces {
            if matches!(face.health, Health::Broken(_)) {
                continue;
            }
            out.push((path.clone(), face.family.clone(), face.classification));
        }
    }
    out
}

#[test]
fn every_fixture_folder_with_faces_has_one_label() {
    let labels = labels();
    for (path, _, _) in classified() {
        assert!(labels.contains_key(&folder(&path)), "no label for {path}");
    }
    let lines = include_str!("labels.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .count();
    assert_eq!(lines - 1, labels.len(), "a folder is labeled twice");
}

#[test]
fn the_fixture_fonts_are_classed_against_the_hand_labels() {
    let labels = labels();
    let faces = classified();
    let mut wrong = Vec::new();
    for (path, family, guess) in &faces {
        let want = labels[&folder(path)];
        if guess.map(|g| g.class) != Some(want) {
            wrong.push(format!("{path} ({family:?}): want {want:?}, got {guess:?}"));
        }
    }
    let right = faces.len() - wrong.len();
    let share = right as f32 / faces.len() as f32;
    // The fixtures hold 200 files; all but the truncated copy and the Type 1
    // font have readable faces (a TTC holds two).
    assert!(faces.len() >= 195, "only {} faces", faces.len());
    assert!(
        share >= 0.9,
        "{right}/{} = {:.1} % classed right:\n{}",
        faces.len(),
        share * 100.0,
        wrong.join("\n")
    );
}

#[test]
fn every_guess_has_a_confidence_in_range() {
    for (path, _, guess) in classified() {
        if let Some(g) = guess {
            assert!((0.0..=1.0).contains(&g.confidence), "{path}: {g:?}");
        }
    }
}

/// Fixtures that declare no class (`sFamilyClass` 0, PANOSE all 0, not fixed
/// pitch), one per class the outlines must find by themselves.
#[test]
fn outlines_class_faces_that_declare_nothing() {
    let cases = [
        ("fontshare/archivo/Archivo-Bold.woff2", Classification::Sans),
        (
            "fontshare/azeret-mono/AzeretMono-ExtraBoldItalic.woff",
            Classification::Mono,
        ),
        (
            "google-repo/castoro/Castoro[wght].ttf",
            Classification::Serif,
        ),
        (
            "google-static/robotoslab/RobotoSlab-ExtraBold.ttf",
            Classification::Slab,
        ),
        (
            "google-repo/solitreo/Solitreo-Regular.ttf",
            Classification::Script,
        ),
        (
            "google-repo/micro5/Micro5-Regular.ttf",
            Classification::Display,
        ),
    ];
    for (path, want) in cases {
        let parsed = parser::parse_file(&fixtures().join(path)).expect("read fixture");
        let guess = parsed.faces[0].classification.expect("a class");
        assert_eq!(
            guess.source,
            ClassSource::Outlines,
            "{path} declares a class"
        );
        assert_eq!(guess.class, want, "{path}: {guess:?}");
        assert!(guess.confidence >= 0.5, "{path}: {guess:?}");
    }
}

/// Italic sans faces lean but are not scripts.
#[test]
fn an_italic_sans_stays_a_sans() {
    for path in [
        "fontshare/sora/Sora-Italic.woff2",
        "google-repo/jost/Jost-Italic[wght].ttf",
        "google-static/monasans/MonaSans-BlackItalic.ttf",
    ] {
        let parsed = parser::parse_file(&fixtures().join(path)).expect("read fixture");
        let guess = parsed.faces[0].classification.expect("a class");
        assert_eq!(guess.class, Classification::Sans, "{path}: {guess:?}");
    }
}

#[test]
fn declared_classes() {
    use Classification::*;
    let class =
        |fc: i16, panose: &[u8], fixed| from_declared(fc, panose, fixed).map(|d| d.guess.class);
    assert_eq!(class(0, &[0; 10], false), None, "blank says nothing");
    assert_eq!(class(0, &[], false), None, "no OS/2 says nothing");
    assert_eq!(class(0, &[0; 10], true), Some(Mono), "post.isFixedPitch");
    assert_eq!(
        class(0, &[2, 11, 5, 9, 0, 0, 0, 0, 0, 0], false),
        Some(Mono),
        "PANOSE monospaced"
    );
    assert_eq!(
        class(8 << 8, &[2, 11, 5, 9, 0, 0, 0, 0, 0, 0], false),
        Some(Mono),
        "mono first"
    );
    assert_eq!(class(1 << 8, &[0; 10], false), Some(Serif));
    assert_eq!(class(5 << 8, &[0; 10], false), Some(Slab));
    assert_eq!(class(8 << 8 | 2, &[0; 10], false), Some(Sans));
    assert_eq!(class(9 << 8, &[0; 10], false), Some(Display));
    assert_eq!(class(10 << 8, &[0; 10], false), Some(Script));
    assert_eq!(class(12 << 8, &[0; 10], false), Some(Symbol));
    assert_eq!(
        class(8 << 8, &[2, 2, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Sans),
        "family class first"
    );
    assert_eq!(
        class(0, &[2, 2, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Serif),
        "cove"
    );
    assert_eq!(
        class(0, &[2, 6, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Slab),
        "square"
    );
    assert_eq!(
        class(0, &[2, 11, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Sans)
    );
    assert_eq!(
        class(0, &[2, 0, 5, 3, 0, 0, 0, 0, 0, 0], false),
        None,
        "any serif style"
    );
    assert_eq!(
        class(0, &[3, 0, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Script)
    );
    assert_eq!(
        class(0, &[4, 0, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Display)
    );
    assert_eq!(
        class(0, &[5, 0, 0, 0, 0, 0, 0, 0, 0, 0], false),
        Some(Symbol)
    );
    assert_eq!(
        class(0, &[3, 0, 0, 3, 0, 0, 0, 0, 0, 0], false),
        Some(Mono),
        "hand written, monospaced"
    );
    assert_eq!(
        class(0x7fff, &[99, 99], false),
        None,
        "out-of-range values say nothing"
    );
    let d = from_declared(8 << 8, &[], false).expect("a class");
    assert!(!d.serif_style_only);
    let g = d.guess;
    assert_eq!(g.source, ClassSource::Declared);
}

#[test]
fn name_words() {
    use Classification::*;
    let class = |name| from_name(name).map(|g| g.class);
    assert_eq!(class("Noto Sans Khmer"), Some(Sans));
    assert_eq!(class("Noto Serif Toto"), Some(Serif));
    assert_eq!(class("Spline Sans Mono"), Some(Mono));
    assert_eq!(class("Roboto Slab"), Some(Slab));
    assert_eq!(class("Clicker Script"), Some(Script));
    assert_eq!(class("Sans Serif Pro"), Some(Sans));
    assert_eq!(class("Sansita"), None, "whole words only");
    assert_eq!(
        class("Red Hat Display"),
        None,
        "display in a name says nothing"
    );
    assert_eq!(class(""), None);
}

/// A face without Latin letters falls back to its family name, then to none.
#[test]
fn faces_without_latin_letters() {
    let parsed = parser::parse_file(
        &fixtures().join("google-repo/notosansoriyaui/NotoSansOriyaUI-Bold.ttf"),
    )
    .expect("read fixture");
    let guess = parsed.faces[0]
        .classification
        .expect("a class from the name");
    assert_eq!(guess.class, Classification::Sans);
    assert_eq!(guess.source, ClassSource::Name);

    let parsed =
        parser::parse_file(&fixtures().join("google-repo/phetsarath/Phetsarath-Regular.ttf"))
            .expect("read fixture");
    assert_eq!(
        parsed.faces[0].classification, None,
        "no letters, no words: no class"
    );
}

/// Malformed font data is never a panic: a truncated file is broken, and its
/// faces, if any, carry whatever class could be read.
#[test]
fn malformed_and_empty_data() {
    let parsed = parser::parse(&[]);
    assert!(parsed.faces.is_empty());
    let data =
        std::fs::read(fixtures().join("google-repo/castoro/Castoro[wght].ttf")).expect("read");
    for len in [12, 200, 1_000, 5_000, data.len() / 2] {
        let parsed = parser::parse(&data[..len]);
        for face in parsed.faces {
            if let Some(g) = face.classification {
                assert!((0.0..=1.0).contains(&g.confidence));
            }
        }
    }
}

#[test]
fn a_parsed_face_without_a_class_reads_back_from_old_json() {
    let face = parser::ParsedFace::empty(0, Health::Ok);
    let mut json = serde_json::to_value(&face).expect("json");
    json.as_object_mut()
        .expect("object")
        .remove("classification");
    let back: parser::ParsedFace = serde_json::from_value(json).expect("old JSON parses");
    assert_eq!(back.classification, None);
}

#[test]
fn runs_count_overlapping_contours_once() {
    let square = |x0: f32, x1: f32| vec![(x0, 0.0), (x1, 0.0), (x1, 1.0), (x0, 1.0)];
    assert_eq!(
        runs_at(&[square(0.0, 2.0), square(1.0, 3.0)], 0.5, false),
        vec![(0.0, 3.0)]
    );
    assert_eq!(
        runs_at(&[square(0.0, 1.0), square(2.0, 3.0)], 0.5, false),
        vec![(0.0, 1.0), (2.0, 3.0)]
    );
    assert!(runs_at(&[square(0.0, 1.0)], 2.0, false).is_empty());
    assert_eq!(runs_at(&[square(0.0, 1.0)], 0.5, true), vec![(0.0, 1.0)]);
}

#[test]
#[ignore = "prints the measurements of every fixture face"]
fn dump_fixture_measures() {
    use read_fonts::FileRef;
    let labels = labels();
    for path in fixture_paths() {
        let data = std::fs::read(fixtures().join(&path)).expect("read");
        let data = if path.ends_with(".woff2") {
            parser::decompress_web(&data, fm_api::common::FontFormat::Woff2).unwrap_or_default()
        } else if path.ends_with(".woff") {
            parser::decompress_web(&data, fm_api::common::FontFormat::Woff).unwrap_or_default()
        } else {
            data
        };
        let fonts: Vec<FontRef<'_>> = match FileRef::new(&data) {
            Ok(FileRef::Font(f)) => vec![f],
            Ok(FileRef::Collection(c)) => c.iter().filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        };
        for font in fonts {
            let want = labels.get(&folder(&path));
            let cps: Vec<u32> = font.charmap().mappings().map(|(c, _)| c).collect();
            let got = classify(&font, None, &cps);
            let m = Probe::new(&font).and_then(|p| p.measure());
            let ok = if got.map(|g| g.class) == want.copied() {
                "  "
            } else {
                "XX"
            };
            let m = m.unwrap_or_default();
            println!(
                "{ok} {:<36.36} {:<8} {:<20} st/cap={:.2} fix={} fr={:.2} fh={:.2} sq={:.2} con={:.2} ov={:.2} wob={:.4} px={} uni={} split={}",
                path.rsplit('/').next().unwrap_or(&path),
                want.map_or("-".to_owned(), |w| format!("{w:?}")),
                got.map_or("-".to_owned(), |g| format!("{:?}/{:?}", g.class, g.source)),
                m.stem / m.cap_height.max(f32::EPSILON),
                m.fixed_advance as u8,
                m.foot_ratio,
                m.foot_height,
                m.foot_squareness,
                m.contrast,
                m.overhang,
                m.wobble,
                m.pixel as u8,
                m.unicase as u8,
                m.split_stem as u8,
            );
        }
    }
}
