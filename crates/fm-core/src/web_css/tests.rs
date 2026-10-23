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

fn style(slant: Slant, weight: u16, axes: Vec<Axis>) -> FaceStyle {
    FaceStyle {
        family: "Inter".into(),
        slant,
        weight,
        axes,
    }
}

const BOTH: [WebFormat; 2] = [WebFormat::Woff2, WebFormat::Woff];

#[test]
fn a_static_face_has_one_weight_and_woff2_before_woff() {
    let css = font_face(
        &style(Slant::Upright, 700, Vec::new()),
        "Inter-Bold",
        &[WebFormat::Woff, WebFormat::Woff2],
        None,
    );
    assert_eq!(
        css,
        "@font-face {\n  font-family: \"Inter\";\n  font-style: normal;\n  font-weight: 700;\n  font-display: swap;\n  src: url(\"Inter-Bold.woff2\") format(\"woff2\"), url(\"Inter-Bold.woff\") format(\"woff\");\n}\n"
    );
    assert!(!css.contains("unicode-range"));
    assert!(!css.contains("font-stretch"));
}

#[test]
fn the_slant_gives_the_font_style() {
    let italic = font_face(&style(Slant::Italic, 400, Vec::new()), "a", &BOTH, None);
    assert!(italic.contains("  font-style: italic;\n"), "{italic}");
    let oblique = font_face(&style(Slant::Oblique, 400, Vec::new()), "a", &BOTH, None);
    assert!(oblique.contains("  font-style: oblique;\n"), "{oblique}");
}

#[test]
fn one_format_writes_one_source() {
    let css = font_face(
        &style(Slant::Upright, 400, Vec::new()),
        "a",
        &[WebFormat::Woff],
        None,
    );
    assert!(
        css.contains("  src: url(\"a.woff\") format(\"woff\");\n"),
        "{css}"
    );
}

#[test]
fn a_variable_face_writes_ranges_from_its_axes() {
    let css = font_face(
        &style(
            Slant::Upright,
            400,
            vec![
                axis("wght", 100.0, 400.0, 900.0),
                axis("wdth", 62.5, 100.0, 100.0),
                axis("slnt", -11.0, 0.0, 0.0),
                axis("opsz", 8.0, 14.0, 144.0),
            ],
        ),
        "a",
        &BOTH,
        None,
    );
    assert!(css.contains("  font-weight: 100 900;\n"), "{css}");
    assert!(css.contains("  font-stretch: 62.5% 100%;\n"), "{css}");
    assert!(css.contains("  font-style: oblique 0deg 11deg;\n"), "{css}");
    assert!(!css.contains("opsz"), "{css}");
}

#[test]
fn a_variable_face_without_wght_keeps_its_weight_class() {
    let css = font_face(
        &style(Slant::Italic, 300, vec![axis("opsz", 8.0, 14.0, 144.0)]),
        "a",
        &BOTH,
        None,
    );
    assert!(css.contains("  font-weight: 300;\n"), "{css}");
    assert!(css.contains("  font-style: italic;\n"), "{css}");
}

#[test]
fn an_inverted_or_single_point_axis_is_ordered_or_one_value() {
    let css = font_face(
        &style(
            Slant::Upright,
            400,
            vec![
                axis("wght", 900.0, 400.0, 100.0),
                axis("wdth", 100.0, 100.0, 100.0),
            ],
        ),
        "a",
        &BOTH,
        None,
    );
    assert!(css.contains("  font-weight: 100 900;\n"), "{css}");
    assert!(css.contains("  font-stretch: 100%;\n"), "{css}");
}

#[test]
fn a_non_finite_axis_is_ignored() {
    let css = font_face(
        &style(
            Slant::Upright,
            500,
            vec![axis("wght", f32::NAN, 400.0, 900.0)],
        ),
        "a",
        &BOTH,
        None,
    );
    assert!(css.contains("  font-weight: 500;\n"), "{css}");
}

#[test]
fn the_family_is_quoted_and_escaped() {
    let mut s = style(Slant::Upright, 400, Vec::new());
    s.family = "My \"Quoted\" \\ Family\n".into();
    let css = font_face(&s, "a", &BOTH, None);
    assert!(
        css.contains(r#"  font-family: "My \"Quoted\" \\ Family\A ";"#),
        "{css}"
    );
}

#[test]
fn a_subset_writes_the_mapped_code_points_as_ranges() {
    let css = font_face(
        &style(Slant::Upright, 400, Vec::new()),
        "a",
        &BOTH,
        Some(&[0x42, 0x20, 0x41, 0x1F600, 0x43, 0x41, 0x10FFFF]),
    );
    assert!(
        css.contains("  unicode-range: U+0020, U+0041-0043, U+1F600, U+10FFFF;\n"),
        "{css}"
    );
}

#[test]
fn code_points_collapse_into_ranges() {
    assert_eq!(ranges(&[]), Vec::new());
    assert_eq!(
        ranges(&[5, 3, 4, 4, 9, u32::MAX]),
        [(3, 5), (9, 9), (u32::MAX, u32::MAX)]
    );
    assert_eq!(unicode_range(&[0x30, 0x31, 0x39]), "U+0030-0031, U+0039");
}

#[test]
fn blocks_are_separated_by_a_blank_line() {
    assert_eq!(
        stylesheet(&["a {}\n".into(), "b {}\n".into()]),
        "a {}\n\nb {}\n"
    );
    assert_eq!(stylesheet(&[]), "");
}
