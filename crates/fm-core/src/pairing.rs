//! Rule-based font pairing (E23.4): whether a face of another family goes
//! with a face, how well, and why in plain words.
//!
//! Four rules, each worth a share of the score:
//!
//! - **Contrast**: a serif or slab with a sans and the other way round
//!   ([`CONTRAST`]); a display or script face with a sans or a serif
//!   ([`ACCENT_CONTRAST`], less: a text face's first partner is a text face).
//! - **Superfamily** ([`SUPERFAMILY`]): the family names share the words
//!   before their class word ("Noto Sans" / "Noto Serif").
//! - **Designer** ([`DESIGNER`]): the same designer (name ID 9).
//! - **x-height** ([`X_HEIGHT`]): x-heights, as a share of the em, within
//!   [`X_HEIGHT_TOLERANCE`] of each other.
//!
//! A display or script face pairs only with a sans or a serif, never with
//! another display or script face, whatever the other rules say. A face no
//! rule holds for is not a pairing.

use fm_api::common::Classification;

/// Score of the contrast rule between text classes (serif or slab with a
/// sans). It outweighs the designer and x-height rules together: a
/// contrasting partner comes before a look-alike.
pub const CONTRAST: f32 = 0.4;
/// Score of the contrast rule for a display or script face with a sans or a
/// serif.
pub const ACCENT_CONTRAST: f32 = 0.3;
/// Score of the superfamily rule.
pub const SUPERFAMILY: f32 = 0.3;
/// Score of the designer rule.
pub const DESIGNER: f32 = 0.2;
/// Score of the x-height rule.
pub const X_HEIGHT: f32 = 0.1;
/// How far apart two x-heights may be, as a share of the smaller one.
pub const X_HEIGHT_TOLERANCE: f32 = 0.05;

/// Words that end the superfamily part of a family name.
const CLASS_WORDS: [&str; 4] = ["sans", "serif", "slab", "mono"];

/// What the rules look at in one face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairFacts<'a> {
    /// Family name.
    pub family: &'a str,
    /// Designer, when declared.
    pub designer: Option<&'a str>,
    /// Design class, when known.
    pub class: Option<Classification>,
    /// x-height as a share of the em, when known.
    pub x_height: Option<f32>,
}

/// A pairing: its score (`0.0..=1.0`) and one reason per rule that holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Pairing {
    /// The sum of the scores of the rules that hold.
    pub score: f32,
    /// Plain-word reasons, in rule order.
    pub reasons: Vec<String>,
}

/// How `other` pairs with `base`; `None` when no rule holds, or when one of
/// them is a display or script face and the other is not a sans or serif.
/// The caller keeps faces of the same family apart.
pub fn pair(base: &PairFacts<'_>, other: &PairFacts<'_>) -> Option<Pairing> {
    if !allowed(base.class, other.class) {
        return None;
    }
    let mut score = 0.0;
    let mut reasons = Vec::new();
    if let (Some(a), Some(b)) = (base.class, other.class)
        && let Some(points) = contrast(a, b)
    {
        score += points;
        reasons.push(format!("contrast: {} with {}", word(a), word(b)));
    }
    if same_superfamily(base.family, other.family) {
        score += SUPERFAMILY;
        reasons.push("same superfamily".to_owned());
    }
    if let (Some(a), Some(b)) = (base.designer, other.designer)
        && same_text(a, b)
    {
        score += DESIGNER;
        reasons.push("same designer".to_owned());
    }
    if let (Some(a), Some(b)) = (base.x_height, other.x_height)
        && close_x_heights(a, b)
    {
        score += X_HEIGHT;
        reasons.push("similar x-height".to_owned());
    }
    (!reasons.is_empty()).then_some(Pairing { score, reasons })
}

/// A display or script face goes only with a sans or a serif.
fn allowed(a: Option<Classification>, b: Option<Classification>) -> bool {
    use Classification::{Display, Sans, Script, Serif};
    let loose = |c| matches!(c, Some(Display | Script));
    let plain = |c| matches!(c, Some(Sans | Serif));
    (!loose(a) || plain(b)) && (!loose(b) || plain(a))
}

/// The score of the contrast between two classes; `None` when they do not
/// contrast.
fn contrast(a: Classification, b: Classification) -> Option<f32> {
    use Classification::{Display, Sans, Script, Serif, Slab};
    match (a, b) {
        (Serif | Slab, Sans) | (Sans, Serif | Slab) => Some(CONTRAST),
        (Display | Script, Sans | Serif) | (Sans | Serif, Display | Script) => {
            Some(ACCENT_CONTRAST)
        }
        _ => None,
    }
}

fn word(class: Classification) -> &'static str {
    match class {
        Classification::Serif => "serif",
        Classification::Sans => "sans serif",
        Classification::Slab => "slab serif",
        Classification::Mono => "monospace",
        Classification::Script => "script",
        Classification::Display => "display",
        Classification::Symbol => "symbol",
    }
}

/// The superfamily of a family name: its words before the first class word
/// ("Sans", "Serif", "Slab", "Mono"), lower case. `None` without a class
/// word, or when the name starts with one.
pub fn superfamily(family: &str) -> Option<String> {
    let words: Vec<&str> = family.split_whitespace().collect();
    let at = words
        .iter()
        .position(|w| CLASS_WORDS.iter().any(|c| w.eq_ignore_ascii_case(c)))?;
    (at > 0).then(|| words[..at].join(" ").to_lowercase())
}

/// Two different families of one superfamily.
fn same_superfamily(a: &str, b: &str) -> bool {
    !same_text(a, b) && superfamily(a).is_some_and(|s| superfamily(b) == Some(s))
}

/// Equal, ignoring case and the space around them; never two blanks.
fn same_text(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    !a.is_empty() && a.to_lowercase() == b.to_lowercase()
}

fn close_x_heights(a: f32, b: f32) -> bool {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    lo > 0.0 && hi / lo <= 1.0 + X_HEIGHT_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use Classification::{Display, Mono, Sans, Script, Serif, Slab};

    fn facts(family: &str, class: Option<Classification>) -> PairFacts<'_> {
        PairFacts {
            family,
            designer: None,
            class,
            x_height: None,
        }
    }

    #[test]
    fn contrasting_classes_pair_both_ways() {
        for (a, b, score) in [
            (Serif, Sans, CONTRAST),
            (Slab, Sans, CONTRAST),
            (Sans, Serif, CONTRAST),
            (Sans, Slab, CONTRAST),
            (Display, Sans, ACCENT_CONTRAST),
            (Script, Serif, ACCENT_CONTRAST),
            (Sans, Script, ACCENT_CONTRAST),
            (Serif, Display, ACCENT_CONTRAST),
        ] {
            let got = pair(&facts("A", Some(a)), &facts("B", Some(b)))
                .unwrap_or_else(|| panic!("{a:?} with {b:?}"));
            assert_eq!(got.score, score);
            assert!(got.reasons[0].starts_with("contrast: "), "{got:?}");
        }
        // A serif's text partner outranks a display face with the same extras.
        let x = |family, class| PairFacts {
            x_height: Some(0.5),
            ..facts(family, Some(class))
        };
        let sans = pair(&x("A", Serif), &x("B", Sans)).expect("sans");
        let display = pair(&x("A", Serif), &x("C", Display)).expect("display");
        assert!(sans.score > display.score);
        let got = pair(&facts("A", Some(Serif)), &facts("B", Some(Sans))).expect("pair");
        assert_eq!(got.reasons, ["contrast: serif with sans serif"]);
    }

    #[test]
    fn like_classes_and_mono_do_not_contrast() {
        for (a, b) in [
            (Serif, Serif),
            (Sans, Sans),
            (Serif, Slab),
            (Mono, Sans),
            (Sans, Mono),
        ] {
            assert_eq!(pair(&facts("A", Some(a)), &facts("B", Some(b))), None);
        }
        assert_eq!(pair(&facts("A", Some(Serif)), &facts("B", None)), None);
    }

    #[test]
    fn a_display_or_script_face_goes_only_with_a_sans_or_serif() {
        let same_designer = |family, class| PairFacts {
            designer: Some("Jane Doe"),
            x_height: Some(0.5),
            ..facts(family, Some(class))
        };
        for (a, b) in [
            (Script, Script),
            (Script, Display),
            (Display, Display),
            (Script, Slab),
            (Mono, Script),
        ] {
            assert_eq!(
                pair(&same_designer("A", a), &same_designer("B", b)),
                None,
                "{a:?} with {b:?}"
            );
        }
        // An unknown class is not a sans or serif either.
        let unknown = PairFacts {
            class: None,
            ..same_designer("B", Sans)
        };
        assert_eq!(pair(&same_designer("A", Script), &unknown), None);
        // Other faces still pair on the other rules alone.
        let got = pair(&same_designer("A", Mono), &same_designer("B", Sans)).expect("pair");
        assert_eq!(got.reasons, ["same designer", "similar x-height"]);
    }

    #[test]
    fn x_heights_within_five_percent_pair() {
        let x = |family, x_height| PairFacts {
            x_height: Some(x_height),
            ..facts(family, None)
        };
        let got = pair(&x("A", 0.50), &x("B", 0.524)).expect("pair");
        assert_eq!(got.reasons, ["similar x-height"]);
        assert_eq!(got.score, X_HEIGHT);
        assert!(pair(&x("A", 0.524), &x("B", 0.50)).is_some());
        assert_eq!(pair(&x("A", 0.50), &x("B", 0.53)), None);
        assert_eq!(pair(&x("A", 0.0), &x("B", 0.0)), None);
        assert_eq!(pair(&x("A", 0.5), &facts("B", None)), None);
    }

    #[test]
    fn the_same_designer_pairs_ignoring_case_but_never_blank() {
        let by = |family, designer| PairFacts {
            designer: Some(designer),
            ..facts(family, None)
        };
        let got = pair(&by("A", "Pablo Impallari"), &by("B", " pablo impallari ")).expect("pair");
        assert_eq!(got.reasons, ["same designer"]);
        assert_eq!(got.score, DESIGNER);
        assert_eq!(pair(&by("A", " "), &by("B", " ")), None);
        assert_eq!(pair(&by("A", "One"), &by("B", "Two")), None);
    }

    #[test]
    fn a_superfamily_is_the_words_before_the_class_word() {
        assert_eq!(superfamily("Noto Sans").as_deref(), Some("noto"));
        assert_eq!(superfamily("Noto Serif Gurmukhi").as_deref(), Some("noto"));
        assert_eq!(superfamily("IBM Plex Mono").as_deref(), Some("ibm plex"));
        assert_eq!(superfamily("Source SERIF 4").as_deref(), Some("source"));
        assert_eq!(superfamily("Sans Forgetica"), None);
        assert_eq!(superfamily("Libre Baskerville"), None);
        assert_eq!(superfamily(""), None);

        let got = pair(
            &facts("Noto Sans Armenian", Some(Sans)),
            &facts("Noto Serif Toto", Some(Serif)),
        )
        .expect("pair");
        assert_eq!(
            got.reasons,
            ["contrast: sans serif with serif", "same superfamily"]
        );
        assert!((got.score - (CONTRAST + SUPERFAMILY)).abs() < 1e-6);
        // One family is not its own superfamily partner.
        assert_eq!(
            pair(&facts("Noto Sans", None), &facts("noto sans", None)),
            None
        );
        assert_eq!(
            pair(&facts("PT Sans", None), &facts("Fira Sans", None)),
            None
        );
    }

    #[test]
    fn every_rule_together_scores_one() {
        let a = PairFacts {
            family: "Noto Serif",
            designer: Some("Monotype"),
            class: Some(Serif),
            x_height: Some(0.536),
        };
        let b = PairFacts {
            family: "Noto Sans",
            class: Some(Sans),
            ..a
        };
        let got = pair(&a, &b).expect("pair");
        assert_eq!(got.reasons.len(), 4);
        assert!((got.score - 1.0).abs() < 1e-6);
    }
}
