use super::*;
use PairingRole::{Body, Caption, Headline};

/// Nothing to say: no contrast, x-height or era; both of normal width.
const BLANK: RoleFacts = RoleFacts {
    stroke_contrast: None,
    x_height: None,
    width: NORMAL_WIDTH,
    year: None,
};

fn contrast(value: f32) -> RoleFacts {
    RoleFacts {
        stroke_contrast: Some(value),
        ..BLANK
    }
}

fn x_height(value: f32) -> RoleFacts {
    RoleFacts {
        x_height: Some(value),
        ..BLANK
    }
}

fn width(value: u16) -> RoleFacts {
    RoleFacts {
        width: value,
        ..BLANK
    }
}

fn year(value: u16) -> RoleFacts {
    RoleFacts {
        year: Some(value),
        ..BLANK
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn every_role_weighs_the_facts_to_one_and_differently() {
    let all = [Headline, Body, Caption].map(weights);
    for w in all {
        assert!(
            close(w.contrast + w.x_height + w.width + w.era, 1.0),
            "{w:?}"
        );
    }
    let [headline, body, caption] = all;
    // No fact is worth the same for all three roles.
    let differs =
        |f: fn(&Weights) -> f32| !(close(f(&headline), f(&body)) && close(f(&body), f(&caption)));
    assert!(differs(|w| w.contrast));
    assert!(differs(|w| w.x_height));
    assert!(differs(|w| w.width));
    assert!(differs(|w| w.era));
    // A headline leads on contrast, running text and captions on x-height.
    assert!(headline.contrast > body.contrast && body.contrast > caption.contrast);
    assert!(caption.x_height > body.x_height && body.x_height > headline.x_height);
}

#[test]
fn a_headline_wants_more_contrast_body_and_caption_less() {
    let (low, high) = (contrast(0.2), contrast(0.5));
    let got = fit(Headline, &low, &high);
    assert_eq!(got.reasons, ["higher contrast for headline"]);
    // A gap of the full span earns the whole contrast share; widths are equal.
    assert!(close(got.score, weights(Headline).contrast));
    assert_eq!(fit(Body, &high, &low).reasons, ["lower contrast for body"]);
    assert_eq!(
        fit(Caption, &high, &low).reasons,
        ["lower contrast for caption"]
    );

    // The other way round earns nothing and gives no contrast reason.
    let says_contrast = |role, base: &RoleFacts, other: &RoleFacts| {
        fit(role, base, other)
            .reasons
            .iter()
            .any(|r| r.contains("contrast"))
    };
    assert!(!says_contrast(Headline, &low, &contrast(0.2)));
    assert!(!says_contrast(Headline, &high, &low));
    assert!(!says_contrast(Body, &low, &high));
    assert!(!says_contrast(Caption, &low, &high));
    assert!(fit(Headline, &high, &low).score < fit(Headline, &low, &high).score);
    // A gap under the margin scores a little, but says nothing.
    assert!(!says_contrast(Body, &contrast(0.30), &contrast(0.27)));
    // A face with no fingerprint: nothing either way.
    assert!(!says_contrast(Body, &contrast(0.5), &BLANK));
    assert!(!says_contrast(Body, &BLANK, &contrast(0.5)));
}

#[test]
fn the_x_height_reason_names_the_gap_only_when_it_is_close() {
    let reasons = |role, a, b| fit(role, &x_height(a), &x_height(b)).reasons;
    assert_eq!(reasons(Body, 0.50, 0.52), ["x-height within 4 %"]);
    assert_eq!(reasons(Body, 0.52, 0.50), ["x-height within 4 %"]);
    assert_eq!(reasons(Headline, 0.50, 0.5125), ["x-height within 3 %"]);
    // Equal is still "within" something.
    assert_eq!(reasons(Body, 0.50, 0.50), ["x-height within 1 %"]);
    // Just past the rules' tolerance: no reason.
    assert!(reasons(Body, 0.50, 0.53).is_empty());
    assert!(reasons(Headline, 0.53, 0.50).is_empty());
    // Unknown or broken: nothing, and no score.
    let unknown = fit(Body, &x_height(0.5), &BLANK);
    assert!(unknown.reasons.iter().all(|r| !r.contains("x-height")));
    let zero = fit(Body, &x_height(0.0), &x_height(0.5));
    assert!(zero.reasons.iter().all(|r| !r.contains("x-height")));
    assert!(close(zero.score, unknown.score));
}

#[test]
fn a_larger_x_height_helps_a_caption_only() {
    let (small, large) = (x_height(0.40), x_height(0.55));
    let caption = fit(Caption, &small, &large);
    assert_eq!(caption.reasons, ["larger x-height for caption"]);
    assert!(close(
        caption.score,
        weights(Caption).x_height + weights(Caption).width
    ));
    // A smaller one is no help, and neither role but caption says "larger".
    assert!(fit(Caption, &large, &small).reasons.is_empty());
    assert!(fit(Caption, &large, &small).score < caption.score);
    assert!(fit(Body, &small, &large).reasons.is_empty());
    assert!(fit(Headline, &small, &large).reasons.is_empty());
}

#[test]
fn width_goes_by_role_and_says_nothing_when_it_is_the_same() {
    let reasons = |role, a, b| fit(role, &width(a), &width(b)).reasons;
    assert_eq!(reasons(Headline, 5, 3), ["narrower for headline"]);
    assert_eq!(reasons(Headline, 5, 7), ["wider for headline"]);
    assert!(reasons(Headline, 5, 5).is_empty());
    assert_eq!(reasons(Body, 3, 5), ["normal width for body"]);
    assert_eq!(reasons(Caption, 3, 5), ["normal width for caption"]);
    // Normal next to normal holds for almost every font: noise.
    assert!(reasons(Body, 5, 5).is_empty());
    assert!(reasons(Body, 5, 3).is_empty());

    // A headline earns by differing, running text by staying near normal.
    assert!(fit(Headline, &width(5), &width(3)).score > fit(Headline, &width(5), &width(5)).score);
    assert!(fit(Body, &width(5), &width(5)).score > fit(Body, &width(5), &width(3)).score);
    // A width class off the scale does not break the score.
    let wild = fit(Headline, &width(1), &width(1000));
    assert!((0.0..=1.0).contains(&wild.score), "{wild:?}");
    let wild = fit(Caption, &width(1), &width(1000));
    assert!((0.0..=1.0).contains(&wild.score), "{wild:?}");
}

#[test]
fn years_close_together_are_the_same_era() {
    let got = fit(Body, &year(2010), &year(2015));
    assert_eq!(got.reasons, ["same era"]);
    assert!(fit(Body, &year(2010), &year(2016)).reasons.is_empty());
    assert!(fit(Body, &year(2010), &year(2015)).score > fit(Body, &year(2010), &year(2035)).score);
    // Far apart: no reason and no share, not a negative one.
    let far = fit(Body, &year(1800), &year(2020));
    assert!(far.reasons.is_empty());
    assert!(close(far.score, fit(Body, &BLANK, &BLANK).score));
    assert!(fit(Body, &year(2010), &BLANK).reasons.is_empty());
}

#[test]
fn every_fact_at_its_best_scores_one() {
    let base = RoleFacts {
        stroke_contrast: Some(0.0),
        x_height: Some(0.5),
        width: 5,
        year: Some(2010),
    };
    let headline = RoleFacts {
        stroke_contrast: Some(0.9),
        width: 1,
        ..base
    };
    assert!(close(fit(Headline, &base, &headline).score, 1.0));
    let (from, body) = (
        RoleFacts {
            stroke_contrast: Some(0.9),
            ..base
        },
        base,
    );
    assert!(close(fit(Body, &from, &body).score, 1.0));
    assert!(close(fit(Caption, &from, &body).score, 1.0));
}

#[test]
fn blending_shares_the_score_and_puts_the_role_reasons_first() {
    let plain = Pairing {
        score: 0.4,
        reasons: vec![
            "contrast: serif with sans serif".to_owned(),
            X_HEIGHT_REASON.to_owned(),
        ],
    };
    let role = Pairing {
        score: 0.8,
        reasons: vec!["x-height within 2 %".to_owned()],
    };
    let got = blend(plain, role);
    assert!(close(got.score, 0.6));
    assert_eq!(
        got.reasons,
        ["x-height within 2 %", "contrast: serif with sans serif"]
    );
    // A plain score over one (every rule plus the extras) stays in range.
    let high = blend(
        Pairing {
            score: 1.4,
            reasons: vec![],
        },
        Pairing {
            score: 1.0,
            reasons: vec![],
        },
    );
    assert!(close(high.score, 1.0));
}

#[test]
fn the_earliest_year_of_a_copyright_is_its_era() {
    assert_eq!(copyright_year("Copyright 2011 Cyreal"), Some(2011));
    assert_eq!(copyright_year("(c) 2010-2015 Someone, 1998"), Some(1998));
    assert_eq!(
        copyright_year("Copyright © 1996, The Monotype Corp."),
        Some(1996)
    );
    // Not years: too long, too short, out of range, inside a version.
    assert_eq!(copyright_year("Serial 12345 v1.1"), None);
    assert_eq!(copyright_year("Part 0042 and 9999"), None);
    assert_eq!(copyright_year("No date"), None);
    assert_eq!(copyright_year(""), None);
    assert_eq!(copyright_year("٢٠١١"), None);
}
