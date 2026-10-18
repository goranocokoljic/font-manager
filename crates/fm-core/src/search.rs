//! Text search (E3.3): what a query means and how well a face matches it.
//!
//! A query is split into words. Style words ("bold", "italic", "extra
//! condensed") do not search text: they narrow the styles of the families
//! the other words find, by weight, width and slant — a variable face
//! qualifies when its axes reach the style. The other words are terms: every
//! term must match one of a face's fields (family, style, PostScript name,
//! foundry, designer, tags, file name), exactly, as a prefix, inside a word,
//! or within a small edit distance (typo tolerance).
//!
//! The index finds candidates with the FTS5 trigram tokenizer
//! ([`Query::fts_match`]): any face that shares a three-letter run with a
//! term. This module then scores each candidate ([`Query::score`]); a face
//! that scores is a hit. Ranking: an exact family name first, then the sum of
//! the best field match of every term, weighted by field.

use fm_api::common::{Axis, Slant};

/// How much a match in each field counts, in field order (see [`FAMILY`]).
const FIELD_WEIGHTS: [f32; FIELD_COUNT] = [1.0, 0.6, 0.8, 0.5, 0.5, 0.7, 0.3];

/// The number of searchable fields.
pub(crate) const FIELD_COUNT: usize = 7;

/// Match quality of a term against one word of a field.
const EXACT: f32 = 1.0;
const PREFIX: f32 = 0.85;
const INSIDE: f32 = 0.7;
const JOINED: f32 = 0.65;
const ONE_EDIT: f32 = 0.5;
const TWO_EDITS: f32 = 0.4;

/// Added for each style word a face satisfies, and for a family name equal
/// to the whole text of the query.
const STYLE_BONUS: f32 = 0.3;
const FAMILY_BONUS: f32 = 1.0;

/// Positions in [`FaceText::fields`], the order of the `face_fts` columns:
/// family, style, PostScript name, foundry, designer, tags, file name.
pub(crate) const FAMILY: usize = 0;
#[cfg(test)]
const STYLE: usize = 1;
pub(crate) const FOUNDRY: usize = 3;

/// What a face offers to the search: its fields, prepared for matching, and
/// its style.
#[derive(Debug, Clone)]
pub(crate) struct FaceText {
    /// The fields in `face_fts` column order, lower case; `None` when a face
    /// has no value.
    fields: [Option<String>; FIELD_COUNT],
    /// The same with letters and digits only, so "opensans" finds "Open Sans";
    /// `None` when that is the field itself.
    squeezed: [Option<String>; FIELD_COUNT],
    /// OS/2 weight class.
    pub weight: u16,
    /// OS/2 width class.
    pub width: u16,
    /// Slant of the face itself.
    pub slant: Slant,
    /// Variation axes; empty for a static face.
    pub axes: Vec<Axis>,
}

impl FaceText {
    /// Prepares `fields` (in `face_fts` column order) once, so every query
    /// compares without allocating.
    pub(crate) fn new(
        fields: [Option<&str>; FIELD_COUNT],
        weight: u16,
        width: u16,
        slant: Slant,
        axes: Vec<Axis>,
    ) -> Self {
        let fields = fields.map(|f| f.filter(|f| !f.is_empty()).map(str::to_lowercase));
        let squeezed = fields.each_ref().map(|f| {
            f.as_deref()
                .filter(|f| f.chars().any(|c| !c.is_alphanumeric()))
                .map(squeeze)
        });
        Self {
            fields,
            squeezed,
            weight,
            width,
            slant,
            axes,
        }
    }

    /// Field `i` without separators.
    fn squeezed(&self, i: usize) -> Option<&str> {
        self.squeezed[i].as_deref().or(self.fields[i].as_deref())
    }
}

/// A parsed query.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Query {
    /// Lower-case text terms.
    terms: Vec<String>,
    /// Style words, each with what it requires.
    styles: Vec<StyleWord>,
}

/// One style word of a query ("bold", or "extra bold" read as one).
#[derive(Debug, Clone, PartialEq)]
struct StyleWord {
    /// The words as typed, lower case, joined by a space.
    text: String,
    /// What the face must have.
    need: Need,
}

/// What a style word requires of a face.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Need {
    /// This weight class.
    Weight(u16),
    /// A width class in this inclusive range.
    Width(u16, u16),
    /// Italic or oblique.
    Sloped,
    /// Upright.
    Upright,
}

/// Width-class prefixes: "extra condensed" is class 2, "semi expanded" 6.
const WIDTH_PREFIXES: [(&str, u16); 4] = [("ultra", 4), ("extra", 3), ("semi", 1), ("demi", 1)];

impl Query {
    /// Splits `text` into terms and style words. Words are runs of letters
    /// and digits; anything else separates them.
    pub(crate) fn parse(text: &str) -> Self {
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let mut query = Self::default();
        let mut i = 0;
        while i < words.len() {
            let word = words[i];
            let next = words.get(i + 1).copied();
            if let Some(next) = next
                && let Some(need) = style_need(&format!("{word}{next}"))
            {
                query.push_style(format!("{word} {next}"), need);
                i += 2;
                continue;
            }
            match style_need(word) {
                Some(need) => query.push_style(word.to_owned(), need),
                None => query.terms.push(word.to_owned()),
            }
            i += 1;
        }
        // "Regular" means the upright style unless the query asks for a slant.
        let regular = query
            .styles
            .iter()
            .any(|s| matches!(s.text.as_str(), "regular" | "normal" | "book"));
        let slanted = query
            .styles
            .iter()
            .any(|s| matches!(s.need, Need::Sloped | Need::Upright));
        if regular && !slanted {
            query.styles.push(StyleWord {
                text: String::new(),
                need: Need::Upright,
            });
        }
        query
    }

    fn push_style(&mut self, text: String, need: Need) {
        self.styles.push(StyleWord { text, need });
    }

    /// No terms and no style words: every face matches.
    pub(crate) fn is_empty(&self) -> bool {
        self.terms.is_empty() && self.styles.is_empty()
    }

    /// The FTS5 `MATCH` expression that finds candidates: every trigram of
    /// every term of three characters or more, OR-ed, each quoted. `None`
    /// when no term is long enough; then every face is a candidate.
    pub(crate) fn fts_match(&self) -> Option<String> {
        let mut grams: Vec<String> = Vec::new();
        for term in &self.terms {
            let chars: Vec<char> = term.chars().collect();
            for window in chars.windows(3) {
                let gram: String = window.iter().collect();
                if !grams.contains(&gram) {
                    grams.push(gram);
                }
            }
        }
        if grams.is_empty() {
            return None;
        }
        let quoted: Vec<String> = grams
            .iter()
            .map(|g| format!("\"{}\"", g.replace('"', "\"\"")))
            .collect();
        Some(quoted.join(" OR "))
    }

    /// The score of `face`, or `None` when it does not match: a term matches
    /// no field, or a style word is neither the face's style nor part of its
    /// family name.
    pub(crate) fn score(&self, face: &FaceText) -> Option<f32> {
        let mut score = 0.0;
        for term in &self.terms {
            let fields = face.fields.iter().enumerate().zip(FIELD_WEIGHTS);
            let mut best = fields
                .clone()
                .filter_map(|((i, field), weight)| {
                    let quality = plain_quality(term, field.as_deref()?, face.squeezed(i)?);
                    quality.map(|q| q * weight)
                })
                .fold(0.0_f32, f32::max);
            // Typos only when nothing matches as typed: cheaper, and a true
            // match anywhere is the better answer.
            if best <= 0.0 {
                best = fields
                    .filter_map(|((_, field), weight)| {
                        fuzzy_quality(term, field.as_deref()?).map(|q| q * weight)
                    })
                    .fold(0.0_f32, f32::max);
            }
            if best <= 0.0 {
                return None;
            }
            score += best;
        }
        let family = face.fields[FAMILY].as_deref().unwrap_or("");
        for style in &self.styles {
            if satisfies(face, style.need) {
                score += STYLE_BONUS;
            } else if style.text.is_empty() || !names_style(family, &style.text) {
                return None;
            }
        }
        let squeezed_family = face.squeezed(FAMILY).unwrap_or("");
        if !self.terms.is_empty() && squeezed_family == self.terms.concat() {
            score += FAMILY_BONUS;
        }
        Some(score)
    }
}

/// The weight, width or slant a style word (spaces removed) stands for.
fn style_need(word: &str) -> Option<Need> {
    let weight = match word {
        "thin" | "hairline" => Some(100),
        "extralight" | "ultralight" => Some(200),
        "light" => Some(300),
        "semilight" | "demilight" => Some(350),
        "regular" | "normal" | "book" => Some(400),
        "medium" => Some(500),
        "semibold" | "demibold" => Some(600),
        "bold" => Some(700),
        "extrabold" | "ultrabold" => Some(800),
        "black" | "heavy" => Some(900),
        "extrablack" | "ultrablack" => Some(950),
        _ => None,
    };
    if let Some(weight) = weight {
        return Some(Need::Weight(weight));
    }
    match word {
        "italic" | "oblique" | "slanted" => return Some(Need::Sloped),
        "upright" | "roman" => return Some(Need::Upright),
        "condensed" | "narrow" | "compressed" => return Some(Need::Width(1, 4)),
        "expanded" | "extended" | "wide" => return Some(Need::Width(6, 9)),
        _ => {}
    }
    WIDTH_PREFIXES.iter().find_map(|(prefix, step)| {
        let base = word.strip_prefix(prefix)?;
        match base {
            "condensed" => Some(Need::Width(5 - step, 5 - step)),
            "expanded" => Some(Need::Width(5 + step, 5 + step)),
            _ => None,
        }
    })
}

/// Whether the face's style, or its variable axes, meet `need`.
fn satisfies(face: &FaceText, need: Need) -> bool {
    let axis = |tag: &str| face.axes.iter().find(|a| a.tag == tag);
    match need {
        Need::Weight(w) => {
            face.weight == w
                || axis("wght").is_some_and(|a| a.min <= f32::from(w) && f32::from(w) <= a.max)
        }
        Need::Width(lo, hi) => {
            (lo..=hi).contains(&face.width)
                || axis("wdth")
                    .is_some_and(|a| a.min <= width_percent(hi) && width_percent(lo) <= a.max)
        }
        Need::Sloped => {
            face.slant != Slant::Upright
                || axis("ital").is_some_and(|a| a.max >= 0.5)
                || axis("slnt").is_some_and(|a| a.min < 0.0 || a.max > 0.0)
        }
        Need::Upright => {
            face.slant == Slant::Upright
                || axis("ital").is_some_and(|a| a.min < 0.5)
                || axis("slnt").is_some_and(|a| a.min <= 0.0 && 0.0 <= a.max)
        }
    }
}

/// An OS/2 width class as the `wdth` axis percentage (OpenType spec).
fn width_percent(class: u16) -> f32 {
    match class {
        0 | 1 => 50.0,
        2 => 62.5,
        3 => 75.0,
        4 => 87.5,
        5 => 100.0,
        6 => 112.5,
        7 => 125.0,
        8 => 150.0,
        _ => 200.0,
    }
}

/// A style word that is part of the family name ("Black Ops One") matches
/// the family instead of narrowing its styles.
fn names_style(family: &str, style: &str) -> bool {
    let words: Vec<&str> = split_words(family).collect();
    let wanted: Vec<&str> = style.split(' ').collect();
    words.windows(wanted.len()).any(|w| w == wanted.as_slice())
        || split_words(family).any(|w| w == squeeze(style))
}

/// How well `term` matches the lower-case `field` as typed (`squeezed` is
/// the field without separators); `None` for no match.
fn plain_quality(term: &str, field: &str, squeezed: &str) -> Option<f32> {
    let mut best: Option<f32> = None;
    for word in split_words(field) {
        if word == term {
            return Some(EXACT);
        }
        let quality = if word.starts_with(term) {
            PREFIX
        } else if word.contains(term) {
            INSIDE
        } else {
            continue;
        };
        best = Some(best.map_or(quality, |b: f32| b.max(quality)));
    }
    best.or_else(|| squeezed.contains(term).then_some(JOINED))
}

/// How well `term` matches a word of the lower-case `field` within the edits
/// its length allows (none below four letters, one up to seven, else two).
fn fuzzy_quality(term: &str, field: &str) -> Option<f32> {
    let len = term.chars().count();
    let allowed = match len {
        0..=3 => return None,
        4..=7 => 1,
        _ => 2,
    };
    split_words(field)
        .filter_map(|word| {
            // The distance is at least the difference in length.
            let word_len = word.chars().count();
            let whole = (word_len.abs_diff(len) <= allowed).then(|| edit_distance(term, word));
            // A word still being typed: compare with its start.
            let partial = (word_len > len).then(|| {
                let start: String = word.chars().take(len).collect();
                edit_distance(term, &start)
            });
            let d = whole.into_iter().chain(partial).min()?;
            (d <= allowed).then_some(d)
        })
        .min()
        .map(|d| if d <= 1 { ONE_EDIT } else { TWO_EDITS })
}

fn split_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Letters and digits only: "Open Sans" and "opensans" compare equal.
fn squeeze(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Edits (insert, delete, substitute, swap two neighbours) from `a` to `b`:
/// the optimal string alignment distance.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let width = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * width];
    for i in 0..=a.len() {
        d[i * width] = i;
    }
    for (j, cell) in d.iter_mut().enumerate().take(width) {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[(i - 1) * width + j] + 1)
                .min(d[i * width + j - 1] + 1)
                .min(d[(i - 1) * width + j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[(i - 2) * width + j - 2] + 1);
            }
            d[i * width + j] = v;
        }
    }
    d[a.len() * width + b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(family: &str, style: &str, weight: u16, width: u16, slant: Slant) -> FaceText {
        face_with(family, style, None, weight, width, slant)
    }

    fn face_with(
        family: &str,
        style: &str,
        foundry: Option<&str>,
        weight: u16,
        width: u16,
        slant: Slant,
    ) -> FaceText {
        let mut fields: [Option<&str>; FIELD_COUNT] = Default::default();
        fields[FAMILY] = Some(family);
        fields[STYLE] = Some(style);
        fields[FOUNDRY] = foundry;
        FaceText::new(fields, weight, width, slant, Vec::new())
    }

    fn term_quality(term: &str, field: &str) -> Option<f32> {
        plain_quality(term, field, &squeeze(field)).or_else(|| fuzzy_quality(term, field))
    }

    fn axis(tag: &str, min: f32, max: f32) -> Axis {
        Axis {
            tag: tag.to_owned(),
            name: tag.to_owned(),
            min,
            default: min,
            max,
        }
    }

    #[test]
    fn style_words_are_split_from_terms() {
        let q = Query::parse("Roboto Extra-Bold italic");
        assert_eq!(q.terms, ["roboto"]);
        let needs: Vec<Need> = q.styles.iter().map(|s| s.need).collect();
        assert_eq!(needs, [Need::Weight(800), Need::Sloped]);
        assert_eq!(
            Query::parse("semi condensed").styles[0].need,
            Need::Width(4, 4)
        );
        assert_eq!(Query::parse("condensed").styles[0].need, Need::Width(1, 4));
        assert_eq!(
            Query::parse("ultra expanded").styles[0].need,
            Need::Width(9, 9)
        );
    }

    #[test]
    fn regular_means_upright_unless_a_slant_is_asked() {
        let q = Query::parse("regular");
        let upright = face("Inter", "Regular", 400, 5, Slant::Upright);
        let italic = face("Inter", "Italic", 400, 5, Slant::Italic);
        assert!(q.score(&upright).is_some());
        assert!(q.score(&italic).is_none());
        assert!(Query::parse("regular italic").score(&italic).is_some());
    }

    #[test]
    fn style_words_narrow_by_weight_width_and_slant() {
        let bold = face("Inter", "Bold", 700, 5, Slant::Upright);
        let bold_italic = face("Inter", "Bold Italic", 700, 5, Slant::Italic);
        let light = face("Inter", "Light", 300, 5, Slant::Upright);
        let condensed = face("Inter", "Condensed", 400, 3, Slant::Upright);
        let q = Query::parse("inter bold");
        assert!(q.score(&bold).is_some());
        assert!(q.score(&bold_italic).is_some());
        assert!(q.score(&light).is_none());
        let q = Query::parse("inter bold italic");
        assert!(q.score(&bold).is_none());
        assert!(q.score(&bold_italic).is_some());
        let q = Query::parse("condensed");
        assert!(q.score(&condensed).is_some());
        assert!(q.score(&light).is_none());
    }

    #[test]
    fn a_variable_face_meets_a_style_its_axes_reach() {
        let mut variable = face("Inter", "Regular", 400, 5, Slant::Upright);
        variable.axes = vec![axis("wght", 100.0, 900.0), axis("wdth", 75.0, 100.0)];
        assert!(Query::parse("bold").score(&variable).is_some());
        assert!(Query::parse("condensed").score(&variable).is_some());
        assert!(Query::parse("expanded").score(&variable).is_none());
        assert!(Query::parse("italic").score(&variable).is_none());
        variable.axes.push(axis("slnt", -10.0, 0.0));
        assert!(Query::parse("italic").score(&variable).is_some());
    }

    #[test]
    fn a_style_word_in_the_family_name_matches_the_family() {
        let ops = face("Black Ops One", "Regular", 400, 5, Slant::Upright);
        assert!(Query::parse("black ops").score(&ops).is_some());
        let han = face("Han Sans Heavy", "Regular", 400, 5, Slant::Upright);
        assert!(Query::parse("han heavy").score(&han).is_some());
    }

    #[test]
    fn every_term_must_match_some_field() {
        let f = face_with(
            "Source Sans",
            "Regular",
            Some("Adobe"),
            400,
            5,
            Slant::Upright,
        );
        assert!(Query::parse("source adobe").score(&f).is_some());
        assert!(Query::parse("source google").score(&f).is_none());
    }

    #[test]
    fn exact_beats_prefix_beats_inside_beats_typo() {
        assert_eq!(term_quality("sans", "open sans"), Some(EXACT));
        assert_eq!(term_quality("san", "open sans"), Some(PREFIX));
        assert_eq!(term_quality("ans", "open sans"), Some(INSIDE));
        assert_eq!(term_quality("opensans", "open sans"), Some(JOINED));
        assert_eq!(term_quality("snas", "open sans"), Some(ONE_EDIT));
        assert_eq!(term_quality("helvteica", "helvetica"), Some(ONE_EDIT));
        assert_eq!(term_quality("helvtica", "helvetica"), Some(ONE_EDIT));
        assert_eq!(term_quality("halvetika", "helvetica"), Some(TWO_EDITS));
        assert_eq!(term_quality("xyz", "open sans"), None);
        // Short words get no typo allowance.
        assert_eq!(term_quality("sna", "open sans"), None);
    }

    #[test]
    fn the_exact_family_ranks_above_a_family_that_contains_it() {
        let q = Query::parse("inter");
        let exact = face("Inter", "Regular", 400, 5, Slant::Upright);
        let longer = face("Inter Tight", "Regular", 400, 5, Slant::Upright);
        let a = q.score(&exact).expect("exact");
        let b = q.score(&longer).expect("longer");
        assert!(a > b, "{a} <= {b}");
    }

    #[test]
    fn the_fts_expression_quotes_every_trigram_once() {
        assert_eq!(Query::parse("bold").fts_match(), None);
        assert_eq!(Query::parse("ab").fts_match(), None);
        assert_eq!(
            Query::parse("abcd abc").fts_match().as_deref(),
            Some("\"abc\" OR \"bcd\"")
        );
    }

    #[test]
    fn edit_distance_counts_a_swap_as_one() {
        assert_eq!(edit_distance("abcd", "abcd"), 0);
        assert_eq!(edit_distance("abcd", "abdc"), 1);
        assert_eq!(edit_distance("abcd", "abd"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }
}
