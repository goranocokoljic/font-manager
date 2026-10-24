use fm_api::common::{Classification, Metrics};

use super::*;

/// A fingerprint whose drawing is `ink` in every cell.
fn print(ink: f32) -> Fingerprint {
    Fingerprint {
        vector: vec![ink; 64],
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
    }
}

/// `b` differs from `a` in every component but the one `keep` leaves equal,
/// so the reasons name that component alone.
fn only(keep: &str) -> (Profile, Profile) {
    let a = profile(Classification::Sans, Some(print(0.2)));
    let mut b = profile(Classification::Serif, Some(print(0.6)));
    let fp = b.fingerprint.as_mut().expect("fingerprint");
    fp.x_height_ratio = 0.45;
    fp.contrast = 0.90;
    fp.stroke_width = 0.04;
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
    for ink in [0.31, 0.34, 0.38, 0.45, 0.6, 1.0] {
        let b = profile(Classification::Sans, Some(print(ink)));
        let score = compare(&a, &b).score;
        assert!((0.0..=1.0).contains(&score), "{ink}: {score}");
        assert!(score <= previous, "{ink}: {score} > {previous}");
        previous = score;
    }
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
    bold_wide.x_height_ratio = Some(0.45);
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
