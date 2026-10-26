use super::*;

/// A healthy, inactive face whose every name is distinct, so a test that
/// a rule matched cannot pass through another rule.
fn face(id: u32, ps: &str) -> NeededFace {
    NeededFace {
        id: FaceId(id),
        postscript_name: ps.to_owned(),
        full_name: Some(format!("Full {ps}")),
        typographic: Some((format!("Typo {ps}"), "Bold".to_owned())),
        legacy: Some((format!("Legacy {ps}"), "Italic".to_owned())),
        healthy: true,
        broken: false,
        active: false,
    }
}

fn ps(name: &str) -> NeededFont {
    NeededFont {
        postscript_name: Some(name.to_owned()),
        ..NeededFont::default()
    }
}

fn full(name: &str) -> NeededFont {
    NeededFont {
        full_name: Some(name.to_owned()),
        ..NeededFont::default()
    }
}

fn family(family: &str, style: Option<&str>) -> NeededFont {
    NeededFont {
        family: Some(family.to_owned()),
        style: style.map(str::to_owned),
        ..NeededFont::default()
    }
}

fn find(faces: &[NeededFace], font: &NeededFont) -> Option<(FaceId, NeededFontRule)> {
    NeededIndex::new(faces).find(font)
}

#[test]
fn each_rule_finds_its_face_ignoring_case_and_space() {
    let faces = [face(1, "A-Bold")];
    let hit = |rule| Some((FaceId(1), rule));
    assert_eq!(
        find(&faces, &ps(" a-bold ")),
        hit(NeededFontRule::PostscriptName)
    );
    assert_eq!(
        find(&faces, &full("FULL  a-bold")),
        hit(NeededFontRule::FullName)
    );
    assert_eq!(
        find(&faces, &family("typo A-BOLD", Some("bold"))),
        hit(NeededFontRule::TypographicFamily)
    );
    assert_eq!(
        find(&faces, &family("Legacy a-bold", Some("ITALIC"))),
        hit(NeededFontRule::Family)
    );
    // The style is part of the family rules.
    assert_eq!(find(&faces, &family("Typo A-Bold", Some("Black"))), None);
    assert_eq!(find(&faces, &family("Legacy A-Bold", Some("Bold"))), None);
}

#[test]
fn the_first_rule_that_finds_a_face_wins() {
    // Face 1's PostScript name is face 2's full name, and so on down.
    let mut one = face(1, "Shared");
    one.full_name = None;
    let mut two = face(2, "Two");
    two.full_name = Some("Shared".into());
    let mut three = face(3, "Three");
    three.typographic = Some(("Shared".into(), "Regular".into()));
    three.legacy = Some(("Fallback".into(), "Regular".into()));
    let mut four = face(4, "Four");
    four.legacy = Some(("Shared".into(), "Regular".into()));
    let faces = [one, two, three, four];
    let mut both = ps("Shared");
    both.full_name = Some("Shared".into());
    assert_eq!(
        find(&faces, &both),
        Some((FaceId(1), NeededFontRule::PostscriptName))
    );
    // A PostScript name nothing has falls through to the full name.
    let mut fallthrough = ps("Nobody");
    fallthrough.full_name = Some("Shared".into());
    fallthrough.family = Some("Shared".into());
    assert_eq!(
        find(&faces, &fallthrough),
        Some((FaceId(2), NeededFontRule::FullName))
    );
    // Typographic names come before legacy ones; no style asks for Regular.
    assert_eq!(
        find(&faces, &family("Shared", None)),
        Some((FaceId(3), NeededFontRule::TypographicFamily))
    );
    assert_eq!(
        find(&faces, &family("Fallback", None)),
        Some((FaceId(3), NeededFontRule::Family))
    );
}

#[test]
fn of_several_faces_active_wins_then_healthy_then_the_lowest_id() {
    let mut warned = face(1, "Dup");
    warned.healthy = false;
    let low = face(2, "Dup");
    let high = face(3, "Dup");
    let mut active = face(4, "Dup");
    active.active = true;
    let mut broken = face(0, "Dup");
    broken.healthy = false;
    broken.broken = true;
    let font = ps("Dup");
    let pick = |faces: &[NeededFace]| find(faces, &font).map(|(f, _)| f.0);
    assert_eq!(
        pick(&[
            broken.clone(),
            warned.clone(),
            low.clone(),
            high.clone(),
            active
        ]),
        Some(4)
    );
    assert_eq!(
        pick(&[broken.clone(), warned.clone(), high.clone(), low]),
        Some(2)
    );
    assert_eq!(pick(&[broken.clone(), warned, high]), Some(3));
    // An unhealthy face that is not broken can still stand for the font.
    let mut only_warned = face(5, "Dup");
    only_warned.healthy = false;
    assert_eq!(pick(&[broken.clone(), only_warned]), Some(5));
    // A broken face never does, even active.
    let mut active_broken = broken;
    active_broken.active = true;
    assert_eq!(pick(&[active_broken]), None);
}

#[test]
fn unmatched_fonts_are_listed_in_request_order() {
    let faces = [face(1, "A"), face(2, "B")];
    let fonts = vec![
        ps("Missing-One"),
        ps("B"),
        NeededFont::default(),
        family("Nope", Some("Bold")),
        ps("A"),
        ps("b"),
    ];
    let (matched, unmatched) = match_fonts(fonts, &faces);
    let got: Vec<(u32, NeededFontRule)> =
        matched.iter().map(|m| (m.face.0, m.matched_by)).collect();
    assert_eq!(
        got,
        [
            (2, NeededFontRule::PostscriptName),
            (1, NeededFontRule::PostscriptName),
            (2, NeededFontRule::PostscriptName),
        ]
    );
    assert_eq!(matched[2].font, ps("b"), "the font as asked");
    assert_eq!(
        unmatched,
        [
            ps("Missing-One"),
            NeededFont::default(),
            family("Nope", Some("Bold"))
        ]
    );
}

#[test]
fn blank_names_and_a_style_alone_match_nothing() {
    let mut blank = face(1, "");
    blank.full_name = Some(" ".into());
    blank.typographic = None;
    blank.legacy = Some((String::new(), "Regular".into()));
    let faces = [blank];
    assert_eq!(find(&faces, &ps("")), None);
    assert_eq!(find(&faces, &full("  ")), None);
    assert_eq!(find(&faces, &family("", Some("Regular"))), None);
    let style_only = NeededFont {
        style: Some("Regular".into()),
        ..NeededFont::default()
    };
    assert_eq!(find(&faces, &style_only), None);
}
