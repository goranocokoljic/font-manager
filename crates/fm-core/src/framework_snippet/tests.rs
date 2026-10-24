use fm_api::common::Axis;

use super::*;

fn face(family: &str, stem: &str, slant: Slant, weight: u16, axes: Vec<Axis>) -> SnippetFace {
    SnippetFace {
        style: FaceStyle {
            family: family.into(),
            slant,
            weight,
            axes,
        },
        file_stem: stem.into(),
        classification: None,
    }
}

fn classed(class: Classification, f: SnippetFace) -> SnippetFace {
    SnippetFace {
        classification: Some(class),
        ..f
    }
}

fn wght(min: f32, max: f32) -> Axis {
    Axis {
        tag: "wght".into(),
        name: "Weight".into(),
        min,
        default: min,
        max,
    }
}

#[test]
fn each_class_has_its_generic_fallback() {
    let cases = [
        (Some(Classification::Serif), "serif"),
        (Some(Classification::Slab), "serif"),
        (Some(Classification::Sans), "sans-serif"),
        (Some(Classification::Mono), "monospace"),
        (Some(Classification::Script), "cursive"),
        (Some(Classification::Display), "sans-serif"),
        (Some(Classification::Symbol), "sans-serif"),
        (None, "sans-serif"),
    ];
    for (class, generic) in cases {
        assert_eq!(generic_fallback(class), generic, "{class:?}");
    }
}

#[test]
fn kebab_case_keeps_letters_and_digits_and_joins_the_rest_with_one_dash() {
    assert_eq!(kebab("Open Sans"), "open-sans");
    assert_eq!(kebab("  IBM Plex  Mono "), "ibm-plex-mono");
    assert_eq!(kebab("Source_Code-Pro 3"), "source-code-pro-3");
    assert_eq!(kebab("Café Röst"), "café-röst");
    assert_eq!(kebab("\"\\'"), "font");
    assert_eq!(kebab(""), "font");
}

#[test]
fn js_strings_escape_quotes_backslashes_and_non_ascii() {
    assert_eq!(js_string("Inter"), "'Inter'");
    assert_eq!(js_string(r#"It's "a\b""#), r#"'It\'s "a\\b"'"#);
    assert_eq!(js_string("Café"), r"'Caf\u00E9'");
    // Outside the BMP: a surrogate pair.
    assert_eq!(js_string("a😀"), r"'a\uD83D\uDE00'");
    assert_eq!(js_string("a\nb\u{2028}"), r"'a\u000Ab\u2028'");
}

#[test]
fn css_strings_escape_quotes_backslashes_and_non_ascii() {
    assert_eq!(css_string("Inter"), "\"Inter\"");
    assert_eq!(css_string(r#"A "b" \c"#), r#""A \"b\" \\c""#);
    assert_eq!(css_string("Café"), "\"Caf\\E9 \"");
    assert_eq!(css_string("a\tb"), "\"a\\9 b\"");
}

#[test]
fn a_family_with_quotes_backslashes_and_non_ascii_stays_valid_in_every_snippet() {
    let faces = [classed(
        Classification::Serif,
        face(r#"Ünï "Q" \B's"#, "UniQB", Slant::Upright, 400, Vec::new()),
    )];
    let tailwind = snippet(Framework::Tailwind, &faces, None);
    assert!(
        tailwind.contains(
            r#"        '\u00FCn\u00EF-q-b-s': ['"\\DC n\\EF  \\"Q\\" \\\\B\'s"', 'serif'],"#
        ),
        "{tailwind}"
    );
    let css = snippet(Framework::CssVariables, &faces, None);
    assert_eq!(
        css,
        ":root {\n  --font-\\FC n\\EF -q-b-s: \"\\DC n\\EF  \\\"Q\\\" \\\\B's\", serif;\n}\n"
    );
    let next = snippet(Framework::NextLocalFont, &faces, None);
    assert!(
        next.contains("export const fontNQBS = localFont({"),
        "{next}"
    );
    assert!(
        next.contains(r"  variable: '--font-\u00FCn\u00EF-q-b-s',"),
        "{next}"
    );
    for text in [&tailwind, &css, &next] {
        assert!(text.is_ascii(), "{text}");
    }
}

#[test]
fn faces_group_by_family_in_request_order_lightest_and_upright_first() {
    let faces = [
        face("Sora", "Sora-BoldItalic", Slant::Italic, 700, Vec::new()),
        classed(
            Classification::Mono,
            face("Mono", "Mono-Regular", Slant::Upright, 400, Vec::new()),
        ),
        face("Sora", "Sora-Bold", Slant::Upright, 700, Vec::new()),
        classed(
            Classification::Sans,
            face("Sora", "Sora-Light", Slant::Upright, 300, Vec::new()),
        ),
    ];
    let css = snippet(Framework::CssVariables, &faces, None);
    assert_eq!(
        css,
        ":root {\n  --font-sora: \"Sora\", sans-serif;\n  --font-mono: \"Mono\", monospace;\n}\n"
    );
    let next = snippet(Framework::NextLocalFont, &faces, Some("public/fonts/"));
    let paths: Vec<_> = next
        .lines()
        .filter_map(|l| l.split("path: '").nth(1))
        .map(|l| l.split('\'').next().unwrap_or_default())
        .collect();
    assert_eq!(
        paths,
        [
            "public/fonts/Sora-Light.woff2",
            "public/fonts/Sora-Bold.woff2",
            "public/fonts/Sora-BoldItalic.woff2",
            "public/fonts/Mono-Regular.woff2",
        ]
    );
}

#[test]
fn families_with_the_same_key_or_name_are_told_apart() {
    let faces = [
        face("Open Sans", "A", Slant::Upright, 400, Vec::new()),
        face("Open-Sans", "B", Slant::Upright, 400, Vec::new()),
        face("open sans", "C", Slant::Upright, 400, Vec::new()),
    ];
    let css = snippet(Framework::CssVariables, &faces, None);
    assert!(css.contains("  --font-open-sans: \"Open Sans\""), "{css}");
    assert!(css.contains("  --font-open-sans-2: \"Open-Sans\""), "{css}");
    assert!(css.contains("  --font-open-sans-3: \"open sans\""), "{css}");
    let next = snippet(Framework::NextLocalFont, &faces, None);
    assert!(next.contains("export const fontOpenSans = "), "{next}");
    assert!(next.contains("export const fontOpenSans2 = "), "{next}");
    // "open sans" → fontOpenSans too (the first letter is upper-cased).
    assert!(next.contains("export const fontOpenSans3 = "), "{next}");
}

#[test]
fn the_fonts_path_defaults_to_dot_fonts_and_loses_a_trailing_slash() {
    let faces = [face(
        "Inter",
        "Inter-Regular",
        Slant::Upright,
        400,
        Vec::new(),
    )];
    for path in [None, Some(""), Some("   ")] {
        let next = snippet(Framework::NextLocalFont, &faces, path);
        assert!(
            next.contains("path: './fonts/Inter-Regular.woff2'"),
            "{next}"
        );
    }
    let next = snippet(Framework::NextLocalFont, &faces, Some(r"..\assets\fonts\"));
    assert!(
        next.contains(r"path: '..\\assets\\fonts/Inter-Regular.woff2'"),
        "{next}"
    );
}

#[test]
fn a_variable_face_gives_a_weight_range() {
    let faces = [face(
        "Inter",
        "Inter-Regular",
        Slant::Upright,
        400,
        vec![wght(100.0, 900.0)],
    )];
    let next = snippet(Framework::NextLocalFont, &faces, None);
    assert!(
        next.contains(
            "    { path: './fonts/Inter-Regular.woff2', weight: '100 900', style: 'normal' },\n"
        ),
        "{next}"
    );
}

#[test]
fn each_face_entry_carries_the_style_of_its_slant() {
    let slnt = Axis {
        tag: "slnt".into(),
        name: "Slant".into(),
        min: -10.0,
        default: 0.0,
        max: 0.0,
    };
    let faces = [
        face("Sora", "Sora-Regular", Slant::Upright, 400, Vec::new()),
        face("Sora", "Sora-Italic", Slant::Italic, 400, Vec::new()),
        face("Sora", "Sora-Oblique", Slant::Oblique, 400, Vec::new()),
        face(
            "Roboto Flex",
            "RobotoFlex-Regular",
            Slant::Upright,
            400,
            vec![slnt],
        ),
    ];
    let next = snippet(Framework::NextLocalFont, &faces, None);
    for line in [
        "    { path: './fonts/Sora-Regular.woff2', weight: '400', style: 'normal' },\n",
        "    { path: './fonts/Sora-Italic.woff2', weight: '400', style: 'italic' },\n",
        "    { path: './fonts/Sora-Oblique.woff2', weight: '400', style: 'oblique' },\n",
        "    { path: './fonts/RobotoFlex-Regular.woff2', weight: '400', style: 'oblique 0deg 10deg' },\n",
    ] {
        assert!(next.contains(line), "{line}\n{next}");
    }
}
