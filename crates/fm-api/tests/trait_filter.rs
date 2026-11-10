//! `TraitFilter` (E49.5): its text form and its wire form.

use fm_api::search::{SearchRequest, TraitFilter};

fn value(t: &str, v: &str) -> TraitFilter {
    TraitFilter::Value {
        trait_id: t.into(),
        value: v.into(),
    }
}

fn range(t: &str, measure: Option<&str>, min: Option<f64>, max: Option<f64>) -> TraitFilter {
    TraitFilter::Range {
        trait_id: t.into(),
        measure: measure.map(Into::into),
        min,
        max,
    }
}

#[test]
fn the_text_form_parses_values_and_bounds() {
    let parse = |s: &str| s.parse::<TraitFilter>().expect(s);
    assert_eq!(parse("R.leg=straight"), value("R.leg", "straight"));
    assert_eq!(parse(" R.leg = straight "), value("R.leg", "straight"));
    assert_eq!(
        parse("O.roundness>=0.95"),
        range("O.roundness", None, Some(0.95), None)
    );
    assert_eq!(
        parse("O.roundness<=0.9"),
        range("O.roundness", None, None, Some(0.9))
    );
    assert_eq!(
        parse("O.roundness:corners>=0.028"),
        range("O.roundness", Some("corners"), Some(0.028), None)
    );
}

#[test]
fn a_malformed_text_form_says_why() {
    let err = |s: &str| s.parse::<TraitFilter>().expect_err(s);
    assert!(err("R.leg").contains("is not <trait>=<value>"));
    assert!(err("=straight").contains("names no trait"));
    assert!(err("O.roundness:>=1").contains("names no trait"));
    assert!(err("R.leg=").contains("names no value"));
    assert!(err("R.leg==straight").contains("names no value"));
    assert!(err("O.roundness>0.9").contains("use >= or <="));
    assert!(err("O.roundness<0.9").contains("use >= or <="));
    assert!(err("O.roundness>=round").contains("is not a number"));
    assert!(err("O.roundness>=inf").contains("is not a number"));
    assert!(err("O.roundness:corners=round").contains("not ="));
}

#[test]
fn the_wire_form_is_tagged_by_kind() {
    let json = serde_json::to_value(value("R.leg", "straight")).expect("json");
    assert_eq!(
        json,
        serde_json::json!({ "kind": "value", "traitId": "R.leg", "value": "straight" })
    );
    let json = serde_json::to_value(range("O.roundness", None, Some(0.95), None)).expect("json");
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "range", "traitId": "O.roundness", "measure": null, "min": 0.95, "max": null
        })
    );
    // Bounds and measure may be left out.
    let back: TraitFilter = serde_json::from_value(
        serde_json::json!({ "kind": "range", "traitId": "O.roundness", "max": 0.9 }),
    )
    .expect("parse");
    assert_eq!(back, range("O.roundness", None, None, Some(0.9)));
}

/// Criterion 1: additive. A request without `glyphTraits` still reads, and
/// one with them round-trips.
#[test]
fn glyph_traits_is_an_additive_request_field() {
    let old: SearchRequest =
        serde_json::from_value(serde_json::json!({ "query": "sans" })).expect("old request");
    assert_eq!(old.glyph_traits, None);
    let req = SearchRequest {
        glyph_traits: Some(vec![
            value("R.leg", "straight"),
            range("O.roundness", None, Some(0.95), None),
        ]),
        ..SearchRequest::default()
    };
    let back: SearchRequest =
        serde_json::from_value(serde_json::to_value(&req).expect("json")).expect("parse");
    assert_eq!(back, req);
}

#[test]
fn filters_with_other_bounds_differ() {
    assert_ne!(
        range("O.roundness", None, Some(0.95), None),
        range("O.roundness", None, Some(0.96), None)
    );
    assert_ne!(
        range("O.roundness", None, Some(0.95), None),
        range("O.roundness", None, None, Some(0.95))
    );
    assert_ne!(value("R.leg", "straight"), value("R.leg", "curved"));
}
