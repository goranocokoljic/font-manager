use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use fm_api::common::Health;

use super::*;
use crate::scanner::{discover, hash_and_parse};

const STATIC: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
const VARIABLE: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";
const COLLECTION: &str = "derived/agdasima/agdasima.ttc";
const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";
/// A Lao face without the Latin letters of the sample.
const LAO: &str = "google-repo/phetsarath/Phetsarath-Regular.ttf";
const WOFF: &str = "fontshare/public-sans/PublicSans-Variable.woff";

/// The tolerance of the snapshot: one vector on every OS.
const TOLERANCE: f32 = 1e-6;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn read(rel: &str) -> Vec<u8> {
    fs::read(fixtures().join(rel)).expect("read fixture")
}

fn covers_sample(code_points: &[u32]) -> bool {
    SAMPLE
        .chars()
        .all(|c| code_points.binary_search(&u32::from(c)).is_ok())
}

#[test]
fn every_latin_fixture_face_gets_a_vector_of_the_right_length() {
    let root = fixtures();
    let discovery = discover(&root).expect("discover");
    let mut latin = 0;
    for file in &discovery.files {
        let (_, parsed) = hash_and_parse(&file.path).expect("read");
        for face in &parsed.faces {
            if face.health != Health::Ok || !covers_sample(&face.code_points) {
                continue;
            }
            latin += 1;
            let FaceFingerprint::Taken(fp) = &face.fingerprint else {
                unreachable!(
                    "{} face {} has no fingerprint",
                    file.path.display(),
                    face.index
                )
            };
            assert_eq!(
                fp.vector.len(),
                Params::default().length,
                "{}",
                file.path.display()
            );
            assert!(fp.vector.iter().all(|v| (0.0..=1.0).contains(v)));
            assert!(fp.vector.iter().any(|v| *v > 0.0), "the sample has ink");
            assert!(fp.x_height_ratio > 0.2 && fp.x_height_ratio < 1.5, "{fp:?}");
            assert!((0.0..=1.0).contains(&fp.contrast), "{fp:?}");
            assert!(fp.stroke_width > 0.0 && fp.stroke_width < 1.0, "{fp:?}");
            assert!((1.0..=9.0).contains(&fp.width_class), "{fp:?}");
            assert!(fp.advance_ratio > 0.0, "{fp:?}");
        }
    }
    assert!(latin > 100, "the corpus has Latin faces: {latin}");
}

#[test]
fn a_non_latin_face_gets_no_fingerprint() {
    let data = read(LAO);
    let mut parsed = crate::parser::parse(&data);
    fill(&data, &mut parsed);
    assert!(!parsed.faces.is_empty());
    for face in &parsed.faces {
        assert_eq!(face.fingerprint, FaceFingerprint::Absent);
    }
}

#[test]
fn a_broken_file_gets_no_fingerprint_and_no_panic() {
    let data = read(TRUNCATED);
    let mut parsed = crate::parser::parse(&data);
    fill(&data, &mut parsed);
    assert!(
        parsed
            .faces
            .iter()
            .all(|f| f.fingerprint == FaceFingerprint::Absent)
    );
    assert_eq!(fingerprint(&data, 0, Params::default()), None);
    assert_eq!(
        fingerprint(b"not a font at all", 0, Params::default()),
        None
    );
    assert_eq!(fingerprint(&[], 0, Params::default()), None);
    // Every cut of a good font: no panic, and a fingerprint only when whole.
    let good = read(STATIC);
    for len in (0..good.len()).step_by(997) {
        let _ = fingerprint(&good[..len], 0, Params::default());
    }
    // A face index past the file.
    assert_eq!(fingerprint(&good, 3, Params::default()), None);
}

#[test]
fn a_web_font_is_fingerprinted_like_its_sfnt() {
    let data = read(WOFF);
    let mut parsed = crate::parser::parse(&data);
    fill(&data, &mut parsed);
    assert!(matches!(
        parsed.faces[0].fingerprint,
        FaceFingerprint::Taken(_)
    ));
}

#[test]
fn the_length_and_size_are_parameters() {
    let data = read(STATIC);
    let long = fingerprint(
        &data,
        0,
        Params {
            x_height_px: 48,
            length: 128,
        },
    )
    .expect("fingerprint");
    assert_eq!(long.vector.len(), 128);
    let bad = Params {
        x_height_px: 32,
        length: 63,
    };
    assert_eq!(
        fingerprint(&data, 0, bad),
        None,
        "not a multiple of the rows"
    );
}

#[test]
fn different_designs_give_different_vectors() {
    let a = fingerprint(&read(STATIC), 0, Params::default()).expect("static");
    let b = fingerprint(&read(COLLECTION), 0, Params::default()).expect("agdasima");
    assert_ne!(a.vector, b.vector);
}

#[test]
fn vectors_round_trip_through_bytes() {
    let v = vec![0.0, 0.25, 1.0, 1e-7];
    let bytes = vector_to_bytes(&v);
    assert_eq!(bytes.len(), 16);
    assert_eq!(vector_from_bytes(&bytes), Some(v));
    assert_eq!(vector_from_bytes(&[1, 2, 3]), None);
}

/// Three fixture faces (a static, a variable at its default instance, the
/// second face of a collection) against the snapshot every OS shares.
#[test]
fn the_same_file_gives_the_same_vector_on_every_os() {
    let mut actual = String::new();
    for (rel, index) in [(STATIC, 0), (VARIABLE, 0), (COLLECTION, 1)] {
        let fp = fingerprint(&read(rel), index, Params::default()).expect("fingerprint");
        let values: Vec<f32> = fp
            .vector
            .iter()
            .copied()
            .chain([
                fp.x_height_ratio,
                fp.contrast,
                fp.stroke_width,
                fp.width_class,
                fp.advance_ratio,
            ])
            .collect();
        let line: Vec<String> = values.iter().map(|v| format!("{v:.9}")).collect();
        let _ = writeln!(actual, "{rel}#{index}\t{}", line.join(" "));
    }
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/snapshots/fingerprints.txt");
    if std::env::var_os("FM_UPDATE_SNAPSHOTS").is_some() {
        fs::write(&snapshot, &actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&snapshot).expect("read snapshot");
    let parse = |text: &str| -> Vec<(String, Vec<f32>)> {
        text.lines()
            .map(|l| {
                let (name, values) = l.split_once('\t').expect("a tab");
                let values = values
                    .split(' ')
                    .map(|v| v.parse::<f32>().expect("a number"))
                    .collect();
                (name.to_owned(), values)
            })
            .collect()
    };
    let (want, got) = (parse(&expected), parse(&actual));
    assert_eq!(
        want.len(),
        got.len(),
        "rerun with FM_UPDATE_SNAPSHOTS=1\n{actual}"
    );
    for ((name, w), (_, g)) in want.iter().zip(&got) {
        assert_eq!(w.len(), g.len(), "{name}");
        for (i, (w, g)) in w.iter().zip(g).enumerate() {
            assert!(
                (w - g).abs() <= TOLERANCE,
                "{name} value {i}: snapshot {w}, now {g}\n{actual}"
            );
        }
    }
}

#[test]
fn a_variable_face_is_drawn_at_its_default_instance() {
    let data = read(VARIABLE);
    let first = fingerprint(&data, 0, Params::default()).expect("fingerprint");
    let again = fingerprint(&data, 0, Params::default()).expect("fingerprint");
    assert_eq!(first, again, "no state carries over between faces");
}
