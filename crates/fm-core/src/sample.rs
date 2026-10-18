//! Dynamic previews (E4.5): the text a face previews when it lacks
//! characters of the sample text.
//!
//! A face that maps every character of the sample previews the sample. A face
//! that lacks some previews text from its own coverage instead, so a row never
//! shows only missing-glyph boxes:
//!
//! - **Script:** of the language presets of [`crate::charsets`] the face
//!   covers completely, the script whose presets' characters the face maps
//!   most (ties: data order); the sample is up to [`SCRIPT_SAMPLE_LEN`]
//!   letters of the first complete preset of that script, lower case when the
//!   script has case (`abcdefghijklmnopqrstuvwxyz`, `абвгдеёжзий…`).
//! - **Symbols:** a face that covers no preset (a dingbat, icon or caps-only
//!   face) previews up to [`SYMBOL_SAMPLE_LEN`] characters it maps, letters
//!   first, separated by spaces.
//!
//! Characters a font needs no glyph for (whitespace, controls, zero-width and
//! joiner characters, variation selectors: [`coverage::is_ignorable`]) never
//! count as missing.
//! A face with an empty set (no `cmap` stored, see E3.7) is not judged: it
//! previews the sample.

use std::sync::LazyLock;

use fm_api::preview::SampleFallback;

use crate::charsets::{self, Charset};
use crate::coverage::{self, CodePointSet};

/// Letters in a script fallback.
pub const SCRIPT_SAMPLE_LEN: usize = 26;

/// Characters in a symbol fallback.
pub const SYMBOL_SAMPLE_LEN: usize = 24;

/// The text one face previews for a sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSample {
    /// The sample, or text from the face's own coverage.
    pub text: String,
    /// Set when `text` is not the sample.
    pub fallback: Option<SampleFallback>,
    /// Characters of the sample the face lacks, in sample order, once each.
    pub missing: Vec<char>,
}

/// One preset prepared for the choice.
struct Preset {
    base: CodePointSet,
    /// The letters a fallback shows, in data order.
    letters: Vec<char>,
}

/// The presets of one ISO 15924 script.
struct Script {
    script: String,
    /// Every base character of the script's presets.
    all: CodePointSet,
    presets: Vec<Preset>,
}

/// The presets grouped by script, scripts in order of first appearance.
fn scripts_of(charsets: &[Charset]) -> Vec<Script> {
    let mut scripts: Vec<(String, Vec<&Charset>)> = Vec::new();
    for c in charsets {
        match scripts.iter_mut().find(|(s, _)| *s == c.script) {
            Some((_, list)) => list.push(c),
            None => scripts.push((c.script.clone(), vec![c])),
        }
    }
    scripts
        .into_iter()
        .map(|(script, list)| Script {
            script,
            all: CodePointSet::from_code_points(
                list.iter()
                    .flat_map(|c| c.base.iter().map(|&ch| u32::from(ch))),
            ),
            presets: list.iter().map(|c| preset(c)).collect(),
        })
        .collect()
}

fn preset(charset: &Charset) -> Preset {
    let alphabetic: Vec<char> = charset
        .base
        .iter()
        .copied()
        .filter(|c| c.is_alphabetic())
        .collect();
    let cased = alphabetic.iter().any(|c| c.is_lowercase());
    Preset {
        base: CodePointSet::from_code_points(charset.base.iter().map(|&c| u32::from(c))),
        letters: alphabetic
            .into_iter()
            .filter(|c| !cased || c.is_lowercase())
            .take(SCRIPT_SAMPLE_LEN)
            .collect(),
    }
}

/// The presets of `data/charsets`, grouped once. The data is embedded and its
/// tests reject a broken file, so a load error is a build defect: it is
/// logged and every fallback is a symbol fallback.
static SCRIPTS: LazyLock<Vec<Script>> = LazyLock::new(|| match charsets::load() {
    Ok(all) => scripts_of(&all),
    Err(e) => {
        tracing::error!(error = %e, "language presets could not be loaded");
        Vec::new()
    }
});

/// Whether a character of the sample must be in the face.
fn counts(c: char) -> bool {
    !coverage::is_ignorable(c)
}

/// The text a face that maps `set` previews for the sample `text`.
pub fn dynamic_sample(set: &CodePointSet, text: &str) -> DynamicSample {
    sample_of(&SCRIPTS, set, text)
}

fn sample_of(scripts: &[Script], set: &CodePointSet, text: &str) -> DynamicSample {
    let same = |missing| DynamicSample {
        text: text.to_owned(),
        fallback: None,
        missing,
    };
    if set.is_empty() {
        return same(Vec::new());
    }
    let mut missing: Vec<char> = Vec::new();
    for c in text.chars().filter(|&c| counts(c)) {
        if !set.contains(u32::from(c)) && !missing.contains(&c) {
            missing.push(c);
        }
    }
    if missing.is_empty() {
        return same(missing);
    }
    if let Some((script, letters)) = script_fallback(scripts, set) {
        return DynamicSample {
            text: letters.iter().collect(),
            fallback: Some(SampleFallback::Script {
                script: script.to_owned(),
            }),
            missing,
        };
    }
    let symbols = symbol_fallback(set);
    if symbols.is_empty() {
        return same(missing);
    }
    DynamicSample {
        text: symbols,
        fallback: Some(SampleFallback::Symbols),
        missing,
    }
}

/// The script the face covers best among those with a complete preset, and
/// the letters of its first complete preset.
fn script_fallback<'a>(scripts: &'a [Script], set: &CodePointSet) -> Option<(&'a str, &'a [char])> {
    let mut best: Option<(usize, &Script, &Preset)> = None;
    for script in scripts {
        let Some(preset) = script
            .presets
            .iter()
            .find(|p| !p.letters.is_empty() && set.intersection_len(&p.base) == p.base.len())
        else {
            continue;
        };
        let score = set.intersection_len(&script.all);
        if best.is_none_or(|(top, _, _)| score > top) {
            best = Some((score, script, preset));
        }
    }
    best.map(|(_, s, p)| (s.script.as_str(), p.letters.as_slice()))
}

/// Up to [`SYMBOL_SAMPLE_LEN`] characters the face maps, letters first.
fn symbol_fallback(set: &CodePointSet) -> String {
    let mut letters = Vec::new();
    let mut others = Vec::new();
    for c in set.iter().filter_map(char::from_u32).filter(|&c| counts(c)) {
        if c.is_alphanumeric() {
            letters.push(c);
            if letters.len() == SYMBOL_SAMPLE_LEN {
                break;
            }
        } else if others.len() < SYMBOL_SAMPLE_LEN {
            others.push(c);
        }
    }
    let mut out = String::new();
    for c in letters.into_iter().chain(others).take(SYMBOL_SAMPLE_LEN) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(text: &str) -> CodePointSet {
        CodePointSet::from_code_points(text.chars().map(u32::from))
    }

    fn charset(code: &str, script: &str, base: &str) -> Charset {
        Charset {
            code: code.into(),
            name: code.into(),
            script: script.into(),
            ot_language: None,
            base: base.chars().filter(|c| !c.is_whitespace()).collect(),
            auxiliary: Vec::new(),
            source: charsets::Source::Project,
        }
    }

    fn scripts() -> Vec<Script> {
        scripts_of(&[
            charset("ru", "Cyrl", "АБВабв"),
            charset("en", "Latn", "ABCDabcd"),
            charset("pl", "Latn", "ABCDĄabcdą"),
            charset("ar", "Arab", "ابت"),
        ])
    }

    #[test]
    fn a_face_that_covers_the_sample_previews_it() {
        let got = sample_of(&scripts(), &set("ABCDabcd"), "Abc\tdab\n cd");
        assert_eq!(
            got,
            DynamicSample {
                text: "Abc\tdab\n cd".into(),
                fallback: None,
                missing: Vec::new(),
            }
        );
    }

    #[test]
    fn a_face_that_lacks_characters_previews_letters_of_its_script() {
        let got = sample_of(&scripts(), &set("ABCDabcd"), "Ваба bad");
        assert_eq!(got.text, "abcd", "lower-case letters of en");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Latn".into()
            })
        );
        assert_eq!(got.missing, vec!['В', 'а', 'б'], "once each, in order");
    }

    #[test]
    fn the_script_the_face_covers_most_wins() {
        // Complete Cyrillic (6 of 6) and complete en; Latin scores 10 with Ą ą.
        let face = set("АБВабвABCDabcdĄą");
        let got = sample_of(&scripts(), &face, "ابت");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Latn".into()
            })
        );
        assert_eq!(got.text, "abcd", "the first complete preset of the script");
        // en incomplete: Cyrillic is the only script with a complete preset.
        let cyr = set("АБВабвABab");
        let got = sample_of(&scripts(), &cyr, "ابت");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Cyrl".into()
            })
        );
        assert_eq!(got.text, "абв");
    }

    #[test]
    fn ties_go_to_the_script_listed_first() {
        let both = scripts_of(&[charset("x", "Xaaa", "ab"), charset("y", "Ybbb", "cd")]);
        let got = sample_of(&both, &set("abcd"), "z");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Xaaa".into()
            })
        );
    }

    #[test]
    fn a_caseless_script_previews_all_its_letters() {
        let got = sample_of(&scripts(), &set("ابت"), "abc");
        assert_eq!(got.text, "ابت");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Arab".into()
            })
        );
    }

    #[test]
    fn a_face_that_covers_no_language_previews_its_own_symbols() {
        let face = set("★☆♠♣ABC");
        let got = sample_of(&scripts(), &face, "Hello");
        assert_eq!(got.fallback, Some(SampleFallback::Symbols));
        assert_eq!(
            got.text, "A B C ★ ☆ ♠ ♣",
            "letters first, then in code point order"
        );
        let dingbats = CodePointSet::from_code_points(0xE000..0xE100);
        let got = sample_of(&scripts(), &dingbats, "Hello");
        assert_eq!(got.text.split(' ').count(), SYMBOL_SAMPLE_LEN);
        assert!(got.text.starts_with('\u{E000}'));
    }

    #[test]
    fn invisible_format_characters_never_count_as_missing() {
        let text = "ab\u{200D}c\u{AD}d\u{FE0F}\u{FEFF}";
        let got = sample_of(&scripts(), &set("ABCDabcd"), text);
        assert_eq!(got.fallback, None);
        assert_eq!(got.text, text);
        assert!(got.missing.is_empty(), "{:?}", got.missing);
    }

    #[test]
    fn a_face_without_a_stored_cmap_is_not_judged() {
        let got = sample_of(&scripts(), &CodePointSet::default(), "Ваба");
        assert_eq!(got.text, "Ваба");
        assert_eq!(got.fallback, None);
        assert!(got.missing.is_empty());
    }

    #[test]
    fn a_face_with_only_whitespace_keeps_the_sample() {
        let got = sample_of(&scripts(), &set(" \u{a0}"), "Hello");
        assert_eq!(got.text, "Hello");
        assert_eq!(got.fallback, None);
        assert_eq!(got.missing, vec!['H', 'e', 'l', 'o']);
    }

    #[test]
    fn script_letters_come_from_the_embedded_presets() {
        let latin = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz");
        let got = dynamic_sample(&latin, "Съешь");
        assert_eq!(got.text, "abcdefghijklmnopqrstuvwxyz");
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Latn".into()
            })
        );
        let ru: Vec<char> = charsets::load()
            .expect("presets")
            .into_iter()
            .find(|c| c.code == "ru")
            .expect("ru")
            .base;
        let got = dynamic_sample(
            &CodePointSet::from_code_points(ru.iter().map(|&c| u32::from(c))),
            "fox",
        );
        assert_eq!(
            got.fallback,
            Some(SampleFallback::Script {
                script: "Cyrl".into()
            })
        );
        assert!(got.text.starts_with("абвгд"), "{}", got.text);
        assert_eq!(got.text.chars().count(), SCRIPT_SAMPLE_LEN);
    }
}
