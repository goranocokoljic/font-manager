//! CSS for a face's axis coordinates and feature switches (E10.5): the
//! `font-variation-settings` and `font-feature-settings` declarations the
//! inspector's "Copy CSS" buttons copy, and the CLI and MCP print.
//!
//! Only what differs from the face's defaults is written: an axis away from its
//! default coordinate, a switchable feature away from its default state. With
//! nothing changed the value is `normal`. Axes keep the font's axis order (as
//! the preview rows do); features are sorted by tag (as the preview rows'
//! `font-feature-settings` is). A tag that is not four printable ASCII
//! characters is not CSS-safe and is left out; `"` and `\` are escaped.

use fm_api::common::{Axis, AxisValue, FeatureInfo, FeatureSetting};
use fm_api::preview::CssValues;

/// Decimal places a coordinate keeps; the finest axis step is 0.01.
const DECIMALS: usize = 3;

/// The two declarations for `axes` at `variations` and `features` switched
/// by `settings`. Tags not on the face, and automatic features, are ignored;
/// the first value given for a tag wins.
pub fn css_values(
    axes: &[Axis],
    features: &[FeatureInfo],
    variations: &[AxisValue],
    settings: &[FeatureSetting],
) -> CssValues {
    CssValues {
        variation_settings: declaration(
            "font-variation-settings",
            variation_parts(axes, variations),
        ),
        feature_settings: declaration("font-feature-settings", feature_parts(features, settings)),
    }
}

fn variation_parts(axes: &[Axis], variations: &[AxisValue]) -> Vec<String> {
    axes.iter()
        .filter_map(|axis| {
            let given = variations.iter().find(|v| v.tag == axis.tag)?.value;
            if !given.is_finite() {
                return None;
            }
            // `f32::clamp` panics on an inverted range; a malformed fvar can have one.
            let (lo, hi) = (axis.min.min(axis.max), axis.min.max(axis.max));
            let value = number(given.max(lo).min(hi));
            (value != number(axis.default)).then_some(())?;
            Some(format!("{} {value}", tag(&axis.tag)?))
        })
        .collect()
}

fn feature_parts(features: &[FeatureInfo], settings: &[FeatureSetting]) -> Vec<String> {
    let mut parts: Vec<(&str, bool)> = features
        .iter()
        .filter(|f| !f.automatic)
        .filter_map(|f| {
            let on = settings.iter().find(|s| s.tag == f.tag)?.on;
            (on != f.default_on).then_some((f.tag.as_str(), on))
        })
        .collect();
    parts.sort_unstable();
    parts.dedup_by(|a, b| a.0 == b.0);
    parts
        .into_iter()
        .filter_map(|(t, on)| Some(format!("{} {}", tag(t)?, u8::from(on))))
        .collect()
}

fn declaration(property: &str, parts: Vec<String>) -> String {
    if parts.is_empty() {
        format!("{property}: normal;")
    } else {
        format!("{property}: {};", parts.join(", "))
    }
}

/// A tag as a CSS string (`"wght"`), or `None` when it is not four printable
/// ASCII characters.
fn tag(tag: &str) -> Option<String> {
    let ok = tag.len() == 4 && tag.bytes().all(|b| (0x20..=0x7e).contains(&b));
    ok.then(|| {
        let escaped: String = tag
            .chars()
            .flat_map(|c| match c {
                '"' | '\\' => vec!['\\', c],
                _ => vec![c],
            })
            .collect();
        format!("\"{escaped}\"")
    })
}

/// A coordinate without float noise: at most [`DECIMALS`] places, trailing
/// zeros dropped (`650`, `87.5`, `-12.25`), never `-0`.
fn number(v: f32) -> String {
    let s = format!("{:.*}", DECIMALS, f64::from(v));
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(tag: &str, min: f32, default: f32, max: f32) -> Axis {
        Axis {
            tag: tag.into(),
            name: tag.into(),
            min,
            default,
            max,
        }
    }

    fn feature(tag: &str, default_on: bool, automatic: bool) -> FeatureInfo {
        FeatureInfo {
            tag: tag.into(),
            name: None,
            automatic,
            default_on,
        }
    }

    fn at(tag: &str, value: f32) -> AxisValue {
        AxisValue {
            tag: tag.into(),
            value,
        }
    }

    fn switch(tag: &str, on: bool) -> FeatureSetting {
        FeatureSetting {
            tag: tag.into(),
            on,
        }
    }

    fn axes() -> Vec<Axis> {
        vec![
            axis("wght", 100.0, 400.0, 900.0),
            axis("wdth", 75.0, 100.0, 125.0),
            axis("opsz", 8.0, 14.0, 144.0),
        ]
    }

    fn features() -> Vec<FeatureInfo> {
        vec![
            feature("ccmp", true, true),
            feature("liga", true, false),
            feature("smcp", false, false),
            feature("ss01", false, false),
        ]
    }

    #[test]
    fn moved_axes_in_font_order() {
        let css = css_values(&axes(), &[], &[at("wdth", 87.5), at("wght", 650.0)], &[]);
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 650, "wdth" 87.5;"#
        );
    }

    #[test]
    fn axes_at_their_default_are_left_out() {
        let css = css_values(&axes(), &[], &[at("wght", 400.0), at("opsz", 14.0)], &[]);
        assert_eq!(css.variation_settings, "font-variation-settings: normal;");
        let css = css_values(&axes(), &[], &[], &[]);
        assert_eq!(css.variation_settings, "font-variation-settings: normal;");
    }

    #[test]
    fn numbers_carry_no_float_noise() {
        let css = css_values(
            &axes(),
            &[],
            &[
                at("wght", 650.0001),
                at("wdth", 87.499_99),
                at("opsz", 12.3),
            ],
            &[],
        );
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 650, "wdth" 87.5, "opsz" 12.3;"#
        );
        // Noise around the default is the default.
        let css = css_values(&axes(), &[], &[at("wght", 400.000_02)], &[]);
        assert_eq!(css.variation_settings, "font-variation-settings: normal;");
    }

    #[test]
    fn negative_and_zero_coordinates() {
        let slnt = [axis("slnt", -12.0, 0.0, 0.0), axis("GRAD", -0.5, 0.25, 1.0)];
        let css = css_values(&slnt, &[], &[at("slnt", -7.25), at("GRAD", -0.0)], &[]);
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "slnt" -7.25, "GRAD" 0;"#
        );
    }

    #[test]
    fn coordinates_are_clamped_and_non_finite_ignored() {
        let css = css_values(
            &axes(),
            &[],
            &[
                at("wght", 2000.0),
                at("wdth", f32::NAN),
                at("opsz", f32::INFINITY),
            ],
            &[],
        );
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 900;"#
        );
    }

    #[test]
    fn an_inverted_axis_range_from_a_malformed_font_does_not_panic() {
        let inverted = [axis("wght", 900.0, 400.0, 100.0)];
        let css = css_values(&inverted, &[], &[at("wght", 2000.0)], &[]);
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 900;"#
        );
        let css = css_values(&inverted, &[], &[at("wght", 650.0)], &[]);
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 650;"#
        );
    }

    #[test]
    fn tags_not_on_the_face_are_ignored_and_the_first_value_wins() {
        let css = css_values(
            &axes(),
            &features(),
            &[at("XXXX", 3.0), at("wght", 700.0), at("wght", 300.0)],
            &[
                switch("zero", true),
                switch("smcp", true),
                switch("smcp", false),
            ],
        );
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "wght" 700;"#
        );
        assert_eq!(css.feature_settings, r#"font-feature-settings: "smcp" 1;"#);
    }

    #[test]
    fn tags_are_escaped_or_left_out() {
        let odd = [
            axis("a\"b\\", 0.0, 0.0, 10.0),
            axis("long", 0.0, 0.0, 10.0),
            axis("bad\u{e9}", 0.0, 0.0, 10.0),
            axis("tab\t", 0.0, 0.0, 10.0),
        ];
        let css = css_values(
            &odd,
            &[],
            &[
                at("a\"b\\", 1.0),
                at("long", 2.0),
                at("bad\u{e9}", 3.0),
                at("tab\t", 4.0),
            ],
            &[],
        );
        assert_eq!(
            css.variation_settings,
            r#"font-variation-settings: "a\"b\\" 1, "long" 2;"#
        );
        let feats = [feature("x\"yz", false, false), feature("abc", false, false)];
        let css = css_values(
            &[],
            &feats,
            &[],
            &[switch("x\"yz", true), switch("abc", true)],
        );
        assert_eq!(css.feature_settings, r#"font-feature-settings: "x\"yz" 1;"#);
    }

    #[test]
    fn changed_features_sorted_by_tag() {
        let css = css_values(
            &[],
            &features(),
            &[],
            &[switch("smcp", true), switch("liga", false)],
        );
        assert_eq!(
            css.feature_settings,
            r#"font-feature-settings: "liga" 0, "smcp" 1;"#
        );
    }

    #[test]
    fn features_at_their_default_and_automatic_ones_are_left_out() {
        let css = css_values(
            &[],
            &features(),
            &[],
            &[
                switch("liga", true),
                switch("ss01", false),
                switch("ccmp", false),
            ],
        );
        assert_eq!(css.feature_settings, "font-feature-settings: normal;");
        let css = css_values(&[], &[], &[], &[]);
        assert_eq!(css.feature_settings, "font-feature-settings: normal;");
    }
}
