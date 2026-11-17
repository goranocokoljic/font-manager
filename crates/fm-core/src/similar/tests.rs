use fm_api::common::{Classification, Metrics};

use super::*;

/// A drawing of 64 cells: `ink` times a fixed pattern of letters and white.
/// The other pattern ([`unlike`]) correlates negatively with it.
fn drawing(ink: f32, unlike: bool) -> Vec<f32> {
    (0..64)
        .map(|i| {
            let cell = ((i * 7) % 5) as f32 / 4.0;
            ink * if unlike { 1.0 - cell } else { cell }
        })
        .collect()
}

/// The drawing that does not look like [`print`]'s.
fn unlike(ink: f32) -> Vec<f32> {
    drawing(ink, true)
}

/// A fingerprint whose drawing is the fixed pattern at `ink`.
fn print(ink: f32) -> Fingerprint {
    Fingerprint {
        vector: drawing(ink, false),
        x_height_ratio: 0.70,
        contrast: 0.20,
        stroke_width: 0.15,
        width_class: 5.0,
        advance_ratio: 0.60,
    }
}

fn profile(class: Classification, fingerprint: Option<Fingerprint>) -> Profile {
    Profile {
        classification: Some(class),
        weight: 400,
        width: 5,
        x_height_ratio: Some(0.70),
        fingerprint,
        shapes_only: false,
    }
}

/// `b` differs from `a` in every component but the one `keep` leaves equal,
/// so the reasons name that component alone.
fn only(keep: &str) -> (Profile, Profile) {
    let a = profile(Classification::Sans, Some(print(0.2)));
    let mut b = profile(Classification::Serif, Some(print(0.6)));
    let fp = b.fingerprint.as_mut().expect("fingerprint");
    fp.vector = unlike(0.6);
    fp.x_height_ratio = 0.30;
    fp.contrast = 0.90;
    fp.stroke_width = 0.01;
    fp.advance_ratio = 1.20;
    let theirs = a.fingerprint.as_ref().expect("fingerprint");
    match keep {
        SAME_CLASSIFICATION => b.classification = a.classification,
        MATCHING_X_HEIGHT => fp.x_height_ratio = theirs.x_height_ratio,
        SIMILAR_WIDTH => fp.advance_ratio = theirs.advance_ratio,
        SIMILAR_CONTRAST => fp.contrast = theirs.contrast,
        SIMILAR_STROKE_WEIGHT => fp.stroke_width = theirs.stroke_width,
        SIMILAR_SHAPES => fp.vector = theirs.vector.clone(),
        _ => unreachable!("a component"),
    }
    (a, b)
}

fn reasons_when_only(keep: &str) -> Vec<&'static str> {
    let (a, b) = only(keep);
    compare(&a, &b).reasons
}

#[test]
fn nothing_alike_gives_no_reason() {
    let a = profile(Classification::Sans, Some(print(0.2)));
    let (_, b) = only(SIMILAR_SHAPES);
    let mut b = b;
    b.fingerprint.as_mut().expect("fingerprint").vector = vec![0.6; 64];
    let found = compare(&a, &b);
    assert!(found.reasons.is_empty(), "{found:?}");
    assert!(found.score < 0.1, "{found:?}");
}

#[test]
fn same_classification_is_a_reason() {
    assert_eq!(
        reasons_when_only(SAME_CLASSIFICATION),
        [SAME_CLASSIFICATION]
    );
}

#[test]
fn matching_x_height_is_a_reason() {
    assert_eq!(reasons_when_only(MATCHING_X_HEIGHT), [MATCHING_X_HEIGHT]);
}

#[test]
fn similar_width_is_a_reason() {
    assert_eq!(reasons_when_only(SIMILAR_WIDTH), [SIMILAR_WIDTH]);
}

#[test]
fn similar_contrast_is_a_reason() {
    assert_eq!(reasons_when_only(SIMILAR_CONTRAST), [SIMILAR_CONTRAST]);
}

#[test]
fn similar_stroke_weight_is_a_reason() {
    assert_eq!(
        reasons_when_only(SIMILAR_STROKE_WEIGHT),
        [SIMILAR_STROKE_WEIGHT]
    );
}

#[test]
fn similar_letter_shapes_is_a_reason() {
    assert_eq!(reasons_when_only(SIMILAR_SHAPES), [SIMILAR_SHAPES]);
}

#[test]
fn identical_faces_score_one_with_every_reason() {
    let a = profile(Classification::Sans, Some(print(0.3)));
    let found = compare(&a, &a.clone());
    assert!((found.score - 1.0).abs() < 1e-6, "{found:?}");
    assert_eq!(
        found.reasons,
        [
            SAME_CLASSIFICATION,
            MATCHING_X_HEIGHT,
            SIMILAR_WIDTH,
            SIMILAR_CONTRAST,
            SIMILAR_STROKE_WEIGHT,
            SIMILAR_SHAPES,
        ]
    );
}

#[test]
fn closer_components_score_higher_and_stay_in_range() {
    let a = profile(Classification::Sans, Some(print(0.3)));
    let mut previous = 1.0f32;
    // `t` moves the drawing toward the other pattern and the stroke away.
    for t in [0.01f32, 0.03, 0.06, 0.1, 0.3, 1.0] {
        let mut b = profile(Classification::Sans, Some(print(0.3)));
        let fp = b.fingerprint.as_mut().expect("fingerprint");
        fp.vector = print(0.3)
            .vector
            .iter()
            .zip(unlike(0.3))
            .map(|(x, y)| x * (1.0 - t) + y * t)
            .collect();
        fp.stroke_width *= 1.0 + t;
        let score = compare(&a, &b).score;
        assert!((0.0..=1.0).contains(&score), "{t}: {score}");
        assert!(score < previous, "{t}: {score} >= {previous}");
        previous = score;
    }
}

/// E54.13: the drawing is compared by where the ink is, not how much: a
/// heavier drawing of the same pattern is alike, the other pattern is not.
#[test]
fn letter_shapes_compare_where_the_ink_is_not_how_much() {
    assert!((shapes(&drawing(0.2, false), &drawing(0.8, false)) - 1.0).abs() < 1e-5);
    assert!(shapes(&drawing(0.5, false), &unlike(0.5)) < f32::EPSILON);
    // An even drawing has no shape: it is alike only an equal one.
    assert!((shapes(&[0.3; 64], &[0.3; 64]) - 1.0).abs() < f32::EPSILON);
    assert!(shapes(&[0.3; 64], &[0.6; 64]) < f32::EPSILON);
    assert!(shapes(&[0.3; 64], &drawing(0.5, false)) < f32::EPSILON);
    // Other lengths and empty drawings do not compare.
    assert!(shapes(&[0.3; 64], &[0.3; 32]) < f32::EPSILON);
    assert!(shapes(&[], &[]) < f32::EPSILON);
}

/// E54.13: serif and slab, sans and mono are close classes: a pair of them
/// scores between a pair of one class and a pair of unrelated classes.
#[test]
fn close_classes_count_half() {
    use Classification::*;
    assert!((classes(Serif, Slab) - CLOSE_CLASSES).abs() < f32::EPSILON);
    assert!((classes(Mono, Sans) - CLOSE_CLASSES).abs() < f32::EPSILON);
    assert!((classes(Slab, Slab) - 1.0).abs() < f32::EPSILON);
    for (x, y) in [(Serif, Sans), (Slab, Mono), (Script, Serif), (Display, Sans)] {
        assert!(classes(x, y) < f32::EPSILON, "{x:?} {y:?}");
    }
    let serif = profile(Serif, Some(print(0.3)));
    let score = |c| compare(&serif, &profile(c, Some(print(0.3)))).score;
    assert!(score(Serif) > score(Slab) && score(Slab) > score(Sans));
    assert_eq!(score(Slab), compare(&profile(Slab, Some(print(0.3))), &serif).score);
}

#[test]
fn the_score_is_symmetric() {
    let (a, b) = only(SIMILAR_WIDTH);
    assert_eq!(compare(&a, &b), compare(&b, &a));
}

/// Criterion 4: without a fingerprint on either side, the pair is compared
/// by metrics and classification only, and the reasons say so.
#[test]
fn without_a_fingerprint_metrics_and_classification_decide() {
    let with = profile(Classification::Sans, Some(print(0.3)));
    let without = profile(Classification::Sans, None);
    for (a, b) in [(&with, &without), (&without, &with), (&without, &without)] {
        let found = compare(a, b);
        assert_eq!(
            found.reasons,
            [
                SAME_CLASSIFICATION,
                MATCHING_X_HEIGHT,
                SIMILAR_WIDTH,
                SIMILAR_STROKE_WEIGHT,
                METRICS_ONLY,
            ],
            "no contrast or shapes without a drawing"
        );
        // Everything it can compare is equal; what it cannot is unknown.
        assert!(found.score > 0.75 && found.score < 0.9, "{found:?}");
    }

    // The fingerprint's drawing plays no part: a different one changes
    // nothing when the other side has none.
    let other = profile(Classification::Sans, Some(print(0.9)));
    assert_eq!(compare(&with, &without), compare(&other, &without));

    // The metrics do: weight, width class and declared x-height.
    let mut bold_wide = profile(Classification::Serif, None);
    bold_wide.weight = 900;
    bold_wide.width = 9;
    bold_wide.x_height_ratio = Some(0.30);
    let found = compare(&without, &bold_wide);
    assert_eq!(found.reasons, [METRICS_ONLY]);
    assert!(found.score < 0.2, "{found:?}");
}

#[test]
fn a_missing_classification_or_x_height_is_left_out() {
    let mut a = profile(Classification::Sans, None);
    a.classification = None;
    a.x_height_ratio = None;
    let b = profile(Classification::Serif, None);
    let found = compare(&a, &b);
    assert_eq!(
        found.reasons,
        [SIMILAR_WIDTH, SIMILAR_STROKE_WEIGHT, METRICS_ONLY]
    );
    assert!(found.score < 0.7, "{found:?}");
}

#[test]
fn a_profile_takes_the_x_height_ratio_from_the_metrics() {
    let metrics = |x: Option<i16>, cap: Option<i16>| Metrics {
        units_per_em: 1000,
        ascender: 800,
        descender: -200,
        x_height: x,
        cap_height: cap,
    };
    let p = |m: &Metrics| Profile::new(None, 400, 5, Some(m), None).x_height_ratio;
    assert_eq!(p(&metrics(Some(500), Some(700))), Some(500.0 / 700.0));
    assert_eq!(p(&metrics(None, Some(700))), None);
    assert_eq!(p(&metrics(Some(500), Some(0))), None);
    assert_eq!(Profile::new(None, 400, 5, None, None).x_height_ratio, None);
}

/// A pair compared on little (Phetsarath in the fixtures: no class, no
/// fingerprint, the same weight and width) does not outrank a close pair
/// compared on everything.
#[test]
fn less_evidence_never_outranks_a_close_full_match() {
    let a = profile(Classification::Serif, Some(print(0.3)));
    let mut close = profile(Classification::Serif, Some(print(0.33)));
    close.fingerprint.as_mut().expect("fingerprint").contrast = 0.3;
    let mut unknown = profile(Classification::Serif, None);
    unknown.classification = None;
    unknown.x_height_ratio = None;
    assert!(compare(&a, &close).score > compare(&a, &unknown).score);
}

#[test]
fn not_a_number_is_not_close() {
    let a = profile(Classification::Sans, Some(print(0.3)));
    let mut b = a.clone();
    b.fingerprint.as_mut().expect("fingerprint").contrast = f32::NAN;
    let found = compare(&a, &b);
    assert!(!found.reasons.contains(&SIMILAR_CONTRAST), "{found:?}");
    assert!((0.0..=1.0).contains(&found.score), "{found:?}");
}

/// E33.1: a shapes-only profile is compared on the drawing and the width;
/// its other measures count as unknown and name no reason.
#[test]
fn a_shapes_only_profile_compares_drawing_and_width_only() {
    let a = profile(Classification::Sans, Some(print(0.2)));
    let mut b = a.clone();
    b.classification = None;
    let fp = b.fingerprint.as_mut().expect("fingerprint");
    fp.x_height_ratio = 0.0;
    fp.contrast = 0.0;
    fp.stroke_width = 0.0;
    let measured = compare(&a, &b);
    b.shapes_only = true;
    let found = compare(&a, &b);
    assert_eq!(found.reasons, [SIMILAR_WIDTH, SIMILAR_SHAPES], "{found:?}");
    assert!(found.score > measured.score, "{found:?} {measured:?}");
    // Unknown counts as half alike: the same shapes-only pair is below a
    // pair alike in everything.
    assert!(found.score < compare(&a, &a).score);
}

/// E54.11: only pairs that both have a fingerprint are candidates.
#[test]
fn a_face_without_a_fingerprint_is_no_candidate() {
    let measured = profile(Classification::Serif, Some(print(0.3)));
    let bare = profile(Classification::Serif, None);
    assert!(candidate(&measured, &measured.clone()));
    assert!(!candidate(&measured, &bare));
    assert!(!candidate(&bare, &measured));
    assert!(!candidate(&bare, &bare.clone()));
}

/// E54.11: symbol faces are candidates for symbol faces only.
#[test]
fn symbol_and_text_faces_are_no_candidates_for_each_other() {
    let text = profile(Classification::Serif, Some(print(0.3)));
    let icons = profile(Classification::Symbol, Some(print(0.3)));
    let unknown = Profile {
        classification: None,
        ..text.clone()
    };
    assert!(is_symbol(&icons) && !is_symbol(&text) && !is_symbol(&unknown));
    assert!(!candidate(&text, &icons));
    assert!(!candidate(&icons, &text));
    assert!(!candidate(&icons, &unknown));
    assert!(candidate(&icons, &icons.clone()));
    assert!(candidate(&text, &unknown));
}

/// E54.11: the floor is "neither alike nor unlike"; a pair that differs in
/// every component scores below it.
#[test]
fn the_floor_is_half_alike_and_unlike_pairs_fall_below_it() {
    assert!((MIN_SCORE - UNKNOWN).abs() < f32::EPSILON);
    let a = profile(Classification::Sans, Some(print(0.2)));
    let (_, mut b) = only(SIMILAR_SHAPES);
    b.fingerprint.as_mut().expect("fingerprint").vector = unlike(0.9);
    assert!(compare(&a, &b).score < MIN_SCORE);
    assert!(compare(&a, &a.clone()).score >= MIN_SCORE);
}

// ---- The agreed pairs: fixtures/similar/expected.json (E54.12) ----

/// One face of the agreed pairs: a file of `fixtures/fonts`.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AgreedFace {
    family: String,
    file: String,
}

/// One reference of the agreed pairs with the faces a person put near and far.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AgreedReference {
    family: String,
    file: String,
    class: String,
    note: String,
    near: Vec<AgreedFace>,
    far: Vec<AgreedFace>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AgreedPairs {
    #[serde(rename = "$schema")]
    schema: String,
    about: String,
    references: Vec<AgreedReference>,
}

const AGREED_CLASSES: [&str; 6] = ["display", "mono", "sans", "script", "serif", "slab"];

/// E54.12: the file a person wrote is one the ranking test (E54.13) can read:
/// the shape the schema gives, real fixture fonts, and no face on both sides.
#[test]
fn the_agreed_pairs_are_well_formed() {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read =
        |p: &str| std::fs::read_to_string(repo.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
    let pairs: AgreedPairs =
        serde_json::from_str(&read("fixtures/similar/expected.json")).expect("expected.json");

    // The schema and this reader agree on the shape.
    let schema: serde_json::Value =
        serde_json::from_str(&read("fixtures/similar/expected.schema.json")).expect("schema");
    let sorted = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v
            .as_array()
            .expect("array")
            .iter()
            .map(|s| s.as_str().expect("string").to_owned())
            .collect();
        k.sort();
        k
    };
    assert_eq!(schema["properties"]["$schema"]["const"], pairs.schema);
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        sorted(&schema["required"]),
        ["$schema", "about", "references"]
    );
    let min = schema["properties"]["references"]["minItems"]
        .as_u64()
        .expect("minItems");
    assert_eq!(min, 20);
    let face = &schema["$defs"]["face"];
    assert_eq!(face["additionalProperties"], false);
    assert_eq!(sorted(&face["required"]), ["family", "file"]);
    let reference = &schema["$defs"]["reference"];
    assert_eq!(reference["additionalProperties"], false);
    assert_eq!(
        sorted(&reference["required"]),
        ["class", "family", "far", "file", "near", "note"]
    );
    assert_eq!(
        sorted(&reference["properties"]["class"]["enum"]),
        AGREED_CLASSES
    );

    // The file itself.
    assert!(!pairs.about.trim().is_empty());
    assert!(
        pairs.references.len() as u64 >= min,
        "at least {min} references"
    );
    let manifest = read("fixtures/fonts/manifest.tsv");
    let known: BTreeSet<&str> = manifest
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').nth(4))
        .collect();
    let check = |f: &str, family: &str| {
        assert!(!family.trim().is_empty(), "{f}: no family name");
        assert!(
            known.contains(f),
            "{f} is not in fixtures/fonts/manifest.tsv"
        );
        let ext = f.rsplit('.').next().unwrap_or_default();
        assert!(
            ["ttf", "otf", "woff", "woff2"].contains(&ext),
            "{f}: not a format every reader of the file can load"
        );
    };
    let mut references = BTreeSet::new();
    let mut classes = BTreeSet::new();
    for r in &pairs.references {
        check(&r.file, &r.family);
        assert!(
            references.insert(r.file.as_str()),
            "{} is a reference twice",
            r.file
        );
        assert!(
            AGREED_CLASSES.contains(&r.class.as_str()),
            "{}: class {}",
            r.file,
            r.class
        );
        classes.insert(r.class.as_str());
        assert!(!r.note.trim().is_empty(), "{}: no note", r.file);
        assert!(
            !r.near.is_empty() && !r.far.is_empty(),
            "{}: near and far",
            r.file
        );
        let mut seen = BTreeSet::from([r.file.as_str()]);
        for f in r.near.iter().chain(&r.far) {
            check(&f.file, &f.family);
            assert!(
                seen.insert(f.file.as_str()),
                "{}: {} is listed twice or is the reference",
                r.file,
                f.file
            );
        }
    }
    assert_eq!(classes.into_iter().collect::<Vec<_>>(), AGREED_CLASSES);
}

// ---- Ranking the agreed pairs (E54.13) ----

/// The profile the index would hold for face 0 of a fixture file: the scan's
/// classification, weight, width, metrics and fingerprint.
fn agreed_profile(file: &str) -> Profile {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(file);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{file}: {e}"));
    let (mut parsed, sfnt) = crate::parser::parse_keeping_sfnt(&data);
    let sfnt = sfnt.into_owned();
    crate::fingerprint::fill_sfnt(&sfnt, &mut parsed);
    let face = parsed.faces.into_iter().next().unwrap_or_else(|| panic!("{file}: no face"));
    let fingerprint = match face.fingerprint {
        crate::fingerprint::FaceFingerprint::Taken(f) => Some(f),
        _ => None,
    };
    Profile::new(
        face.classification.map(|c| c.class),
        face.weight.unwrap_or(400),
        face.width.unwrap_or(5),
        Some(&face.metrics),
        fingerprint,
    )
}

fn agreed_pairs() -> AgreedPairs {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/similar/expected.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("expected.json"))
        .expect("expected.json")
}

/// E54.13: for every reference of the agreed pairs, every "near" face
/// scores above every "far" face. The faces of the reference's own family
/// are compared too: the app leaves them out of its list, the score does
/// not.
#[test]
fn every_near_face_ranks_above_every_far_face() {
    let mut wrong = Vec::new();
    for r in agreed_pairs().references {
        let reference = agreed_profile(&r.file);
        let scored = |faces: &[AgreedFace]| -> Vec<(f32, String)> {
            faces
                .iter()
                .map(|f| {
                    let face = agreed_profile(&f.file);
                    assert!(
                        candidate(&reference, &face),
                        "{} / {}: not ranked at all",
                        r.file,
                        f.file
                    );
                    (compare(&reference, &face).score, f.file.clone())
                })
                .collect()
        };
        let near = scored(&r.near);
        let far = scored(&r.far);
        for (n, nf) in &near {
            for (f, ff) in &far {
                if n <= f {
                    wrong.push(format!("{}: near {nf} {n:.3} <= far {ff} {f:.3}", r.file));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("
"));
}
