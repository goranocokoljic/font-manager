//! Picking fonts for a design brief (E27.4): the pieces of
//! `WorkflowService::pick_fonts_for_brief` that need no index.
//!
//! [`words`] reads the brief, [`license_check`] decides whether a face
//! allows the use asked for, [`coverage_reason`] says which languages were
//! checked, and [`rank`] scores a family against the brief words by its
//! design class, its tags and its names. Nothing here reads the clock, the
//! network or a random source: the same library and brief give the same
//! shortlist.

use fm_api::common::Classification;
use fm_api::library::{LicenseInfo, LicenseType, OpenLicense};

/// Families a shortlist has when the caller does not say.
pub const DEFAULT_LIMIT: u32 = 5;
/// The longest shortlist.
pub const MAX_LIMIT: u32 = 50;

/// A brief word that matches the design class scores this much.
const CLASS_SCORE: u32 = 3;
/// A brief word that matches a tag scores this much.
const TAG_SCORE: u32 = 2;
/// A brief word that matches a name scores this much.
const NAME_SCORE: u32 = 1;

/// Words that say nothing about a font.
const STOP_WORDS: &[&str] = &[
    "and",
    "are",
    "but",
    "for",
    "font",
    "fonts",
    "from",
    "has",
    "into",
    "its",
    "like",
    "more",
    "not",
    "our",
    "some",
    "that",
    "the",
    "this",
    "typeface",
    "typefaces",
    "very",
    "with",
];

/// What a family is matched on.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    /// The family name.
    pub name: &'a str,
    /// Its design class, when known.
    pub classification: Option<Classification>,
    /// The user's tags on it.
    pub tags: &'a [String],
    /// Foundry and designer, when known.
    pub makers: &'a [&'a str],
}

/// How well a family fits the brief words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rank {
    /// Higher is better.
    pub score: u32,
    /// What matched, in plain words.
    pub reasons: Vec<String>,
}

/// The words of `brief` that can match: lower case, letters and digits
/// only, two characters or more, no stop words, each once, in brief order.
/// "sans serif" and "sans-serif" are the one word "sans".
pub fn words(brief: &str) -> Vec<String> {
    let lower = brief.to_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut previous = "";
    for word in tokens(&lower) {
        let skip = (word == "serif" && previous == "sans")
            || word.chars().count() < 2
            || STOP_WORDS.contains(&word)
            || out.iter().any(|w| w == word);
        previous = word;
        if !skip {
            out.push(word.to_owned());
        }
    }
    out
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// The reason a face allows `want`, or `None` when it does not: a
/// recognized open license allows every use, a declared license allows
/// its own. Without `want` every face passes and the reason names what is
/// known of its license.
pub fn license_check(info: &LicenseInfo, want: Option<LicenseType>) -> Option<String> {
    let open = info.open_license.map(open_name);
    match want {
        None => Some(match (open, info.declared) {
            (Some(open), _) => format!("No license use asked; it has an open license ({open})"),
            (None, Some(declared)) => format!(
                "No license use asked; its declared license is {}",
                use_name(declared)
            ),
            (None, None) => "No license use asked; its license is not known".to_owned(),
        }),
        Some(want) => {
            let what = use_name(want);
            if let Some(open) = open {
                Some(format!("Allows {what} use: open license ({open})"))
            } else if info.declared == Some(want) {
                Some(format!("Allows {what} use: declared {what} license"))
            } else {
                None
            }
        }
    }
}

/// The coverage reason of a shortlist asked to cover `languages`.
pub fn coverage_reason(languages: &[String]) -> String {
    match languages {
        [] => "No languages asked".to_owned(),
        [one] => format!("Covers {one}"),
        [first @ .., last] => format!("Covers {} and {last}", first.join(", ")),
    }
}

/// How `facts` fits `words`: a word naming the design class scores most,
/// one naming a tag less, one naming a word of the family, foundry or
/// designer name least.
pub fn rank(words: &[String], facts: &Facts<'_>) -> Rank {
    let mut score = 0;
    let mut reasons = Vec::new();

    if let Some(class) = facts.classification {
        let hits: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|w| class_words(class).contains(w))
            .collect();
        if !hits.is_empty() {
            score += CLASS_SCORE * count(hits.len());
            reasons.push(format!(
                "{} design fits {}",
                class_name(class),
                quoted(&hits)
            ));
        }
    }

    for tag in facts.tags {
        let lower = tag.to_lowercase();
        let hits: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|w| *w == lower || tokens(&lower).any(|t| t == *w))
            .collect();
        if !hits.is_empty() {
            score += TAG_SCORE * count(hits.len());
            reasons.push(format!("Tagged \"{tag}\""));
        }
    }

    for name in std::iter::once(facts.name).chain(facts.makers.iter().copied()) {
        let lower = name.to_lowercase();
        let hits: Vec<&str> = words
            .iter()
            .map(String::as_str)
            .filter(|w| tokens(&lower).any(|t| t == *w))
            .collect();
        if !hits.is_empty() {
            score += NAME_SCORE * count(hits.len());
            reasons.push(format!("Name \"{name}\" matches {}", quoted(&hits)));
        }
    }

    Rank { score, reasons }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn quoted(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The brief words that ask for design class `class`.
fn class_words(class: Classification) -> &'static [&'static str] {
    match class {
        Classification::Serif => &[
            "serif",
            "serifs",
            "roman",
            "bookish",
            "classic",
            "traditional",
            "editorial",
        ],
        Classification::Sans => &[
            "sans",
            "grotesk",
            "grotesque",
            "geometric",
            "humanist",
            "neutral",
            "clean",
            "modern",
        ],
        Classification::Slab => &["slab", "egyptian"],
        Classification::Script => &[
            "script",
            "handwritten",
            "handwriting",
            "calligraphic",
            "cursive",
            "brush",
        ],
        Classification::Display => &[
            "display",
            "headline",
            "headlines",
            "poster",
            "decorative",
            "title",
            "titles",
        ],
        Classification::Mono => &[
            "mono",
            "monospace",
            "monospaced",
            "code",
            "coding",
            "terminal",
        ],
        Classification::Symbol => &["symbol", "symbols", "icon", "icons", "dingbat", "dingbats"],
    }
}

fn class_name(class: Classification) -> &'static str {
    match class {
        Classification::Serif => "Serif",
        Classification::Sans => "Sans-serif",
        Classification::Slab => "Slab serif",
        Classification::Script => "Script",
        Classification::Display => "Display",
        Classification::Mono => "Monospaced",
        Classification::Symbol => "Symbol",
    }
}

fn use_name(use_: LicenseType) -> &'static str {
    match use_ {
        LicenseType::Desktop => "desktop",
        LicenseType::Web => "web",
        LicenseType::App => "app",
        LicenseType::Trial => "trial",
        LicenseType::PersonalOnly => "personal",
        LicenseType::Unknown => "unknown",
    }
}

fn open_name(open: OpenLicense) -> &'static str {
    match open {
        OpenLicense::Ofl => "SIL Open Font License",
        OpenLicense::Apache => "Apache License 2.0",
        OpenLicense::Ufl => "Ubuntu Font Licence",
        OpenLicense::Mit => "MIT",
    }
}

#[cfg(test)]
mod tests {
    use fm_api::common::Embedding;

    use super::*;

    fn license(open: Option<OpenLicense>, declared: Option<LicenseType>) -> LicenseInfo {
        LicenseInfo {
            description: None,
            url: None,
            open_license: open,
            embedding: Embedding::Installable,
            declared,
            seats: None,
            purchase_date: None,
            receipt: None,
            web_use: None,
        }
    }

    #[test]
    fn brief_words_drop_stop_words_and_join_sans_serif() {
        assert_eq!(
            words("A calm, sans-serif font for the Brand — calm & 2026!"),
            ["calm", "sans", "brand", "2026"]
        );
        assert_eq!(words("Sans Serif display"), ["sans", "display"]);
        assert_eq!(words("serif"), ["serif"]);
        assert!(words("  ").is_empty());
        assert_eq!(words("ćirilica ŠRIFT"), ["ćirilica", "šrift"]);
    }

    #[test]
    fn an_open_license_allows_every_use_a_declared_one_its_own() {
        let ofl = license(Some(OpenLicense::Ofl), None);
        for want in [LicenseType::Web, LicenseType::App, LicenseType::Desktop] {
            assert!(license_check(&ofl, Some(want)).is_some(), "{want:?}");
        }
        assert_eq!(
            license_check(&ofl, Some(LicenseType::Web)).as_deref(),
            Some("Allows web use: open license (SIL Open Font License)")
        );
        let web = license(None, Some(LicenseType::Web));
        assert_eq!(
            license_check(&web, Some(LicenseType::Web)).as_deref(),
            Some("Allows web use: declared web license")
        );
        assert_eq!(license_check(&web, Some(LicenseType::App)), None);
        let unknown = license(None, None);
        assert_eq!(license_check(&unknown, Some(LicenseType::Desktop)), None);
        assert_eq!(
            license_check(&unknown, None).as_deref(),
            Some("No license use asked; its license is not known")
        );
    }

    #[test]
    fn coverage_reasons_name_every_language() {
        assert_eq!(coverage_reason(&[]), "No languages asked");
        assert_eq!(coverage_reason(&["pl".into()]), "Covers pl");
        assert_eq!(
            coverage_reason(&["sr-Cyrl".into(), "sr-Latn".into(), "mk".into()]),
            "Covers sr-Cyrl, sr-Latn and mk"
        );
    }

    #[test]
    fn class_ranks_over_tags_over_names() {
        let words = words("a clean brand font like Inter");
        let tags = ["Brand".to_owned()];
        let class = rank(
            &words,
            &Facts {
                name: "Archivo",
                classification: Some(Classification::Sans),
                tags: &[],
                makers: &[],
            },
        );
        let tagged = rank(
            &words,
            &Facts {
                name: "Asul",
                classification: Some(Classification::Serif),
                tags: &tags,
                makers: &[],
            },
        );
        let named = rank(
            &words,
            &Facts {
                name: "Inter",
                classification: None,
                tags: &[],
                makers: &["Rasmus Andersson"],
            },
        );
        assert!(class.score > tagged.score && tagged.score > named.score);
        assert_eq!(class.reasons, ["Sans-serif design fits \"clean\""]);
        assert_eq!(tagged.reasons, ["Tagged \"Brand\""]);
        assert_eq!(named.reasons, ["Name \"Inter\" matches \"inter\""]);
        let none = rank(
            &words,
            &Facts {
                name: "Bebas Neue",
                classification: Some(Classification::Display),
                tags: &[],
                makers: &[],
            },
        );
        assert_eq!(none.score, 0);
        assert!(none.reasons.is_empty());
    }
}
