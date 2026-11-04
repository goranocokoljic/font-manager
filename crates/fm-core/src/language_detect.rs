//! Which language presets a typed text may be in (E43.2).
//!
//! A hint comes from the text's distinctive letters: the letters outside ASCII,
//! each counted once, in any case. ASCII letters say nothing, because every Latin
//! preset has them, so a text of ASCII alone gives no hints. Punctuation, digits
//! and symbols say nothing either: curly quotes are in the Chinese presets but do
//! not make a text Chinese.
//!
//! The score of a preset comes from three facts of `data/charsets`, in this order
//! of weight:
//!
//! 1. How much of the text the preset can write (an auxiliary letter counts half).
//!    This is squared, so a preset that writes all of the text comes before one
//!    that misses a letter.
//! 2. How many of the text's letters are in the preset's `sampleGlyphs`.
//! 3. How much of the glyph line the text uses. Presets with the same letters
//!    differ only here: `sr-Latn` has a line of 5, `hr` a line of 10.
//!
//! Text in decomposed form (`c` + U+0301) is not composed first: the workspace
//! has no normalization crate, and the coverage checks treat text the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use fm_api::search::LanguageHint;

use crate::charsets::{self, Charset};
use crate::error::FmResult;

/// How many characters of a text are read: typed text is short, and this keeps
/// a long paste within the budget of 1 ms per call.
const MAX_SCANNED_CHARS: usize = 1024;

/// What an auxiliary letter counts for; a required letter counts 1.
const AUXILIARY_WEIGHT: f32 = 0.5;

/// Part of the confidence that any preset writing the whole text gets.
const WRITES_WEIGHT: f32 = 0.6;
/// Part of the confidence for text letters that are in the glyph line.
const SAMPLE_WEIGHT: f32 = 0.3;
/// Part of the confidence for how much of the glyph line the text uses.
const FOCUS_WEIGHT: f32 = 0.1;

/// The language presets that may match `text`, best first, each with a
/// confidence above 0 and at most 1; presets of equal score stay in data order.
/// Empty for a text without distinctive letters, or with none that a preset has.
pub fn detect_languages(text: &str) -> FmResult<Vec<LanguageHint>> {
    Ok(table()?.hints(text))
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// The table of the embedded data; built once, because parsing the data files
/// on every call would not fit the budget.
fn table() -> FmResult<&'static Table> {
    if let Some(table) = TABLE.get() {
        return Ok(table);
    }
    let table = Table::new(&charsets::load()?);
    Ok(TABLE.get_or_init(|| table))
}

/// One preset that has a letter.
#[derive(Debug, Clone, Copy)]
struct Hit {
    preset: usize,
    /// In the base set; otherwise in the auxiliary set.
    required: bool,
    in_sample: bool,
}

/// What one preset matched in a text.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
    required: u32,
    auxiliary: u32,
    sample: u32,
}

/// The presets, indexed by distinctive letter: a call only visits the presets
/// that have one of the text's letters.
#[derive(Debug)]
struct Table {
    codes: Vec<String>,
    /// Distinctive letters on each preset's glyph line, as the data writes them.
    sample_lens: Vec<usize>,
    letters: BTreeMap<char, Vec<Hit>>,
}

impl Table {
    fn new(presets: &[Charset]) -> Self {
        let mut table = Self {
            codes: Vec::with_capacity(presets.len()),
            sample_lens: Vec::with_capacity(presets.len()),
            letters: BTreeMap::new(),
        };
        for (preset, charset) in presets.iter().enumerate() {
            let sample = charset.sample_glyphs.as_deref().unwrap_or_default();
            let in_sample: BTreeSet<char> = sample.chars().map(fold).collect();
            // Auxiliary first: a letter in both cases of the two sets is required.
            let mut required = BTreeMap::new();
            for c in &charset.auxiliary {
                required.insert(fold(*c), false);
            }
            for c in &charset.base {
                required.insert(fold(*c), true);
            }
            for (letter, required) in required {
                if is_distinctive(letter) {
                    table.letters.entry(letter).or_default().push(Hit {
                        preset,
                        required,
                        in_sample: in_sample.contains(&letter),
                    });
                }
            }
            table.codes.push(charset.code.clone());
            table
                .sample_lens
                .push(sample.chars().filter(|c| is_distinctive(fold(*c))).count());
        }
        table
    }

    fn hints(&self, text: &str) -> Vec<LanguageHint> {
        let mut letters: Vec<char> = text
            .chars()
            .take(MAX_SCANNED_CHARS)
            .map(fold)
            .filter(|c| is_distinctive(*c))
            .collect();
        letters.sort_unstable();
        letters.dedup();
        if letters.is_empty() {
            return Vec::new();
        }
        let mut tallies = vec![Tally::default(); self.codes.len()];
        for hit in letters
            .iter()
            .filter_map(|letter| self.letters.get(letter))
            .flatten()
        {
            let tally = &mut tallies[hit.preset];
            if hit.required {
                tally.required += 1;
            } else {
                tally.auxiliary += 1;
            }
            tally.sample += u32::from(hit.in_sample);
        }
        let total = letters.len() as f32;
        let mut hints: Vec<LanguageHint> = tallies
            .iter()
            .enumerate()
            .filter(|(_, tally)| tally.required + tally.auxiliary > 0)
            .map(|(preset, tally)| LanguageHint {
                code: self.codes[preset].clone(),
                confidence: confidence(*tally, total, self.sample_lens[preset]),
            })
            .collect();
        // A stable sort, so equal scores stay in data order.
        hints.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        hints
    }
}

fn confidence(tally: Tally, total: f32, sample_len: usize) -> f32 {
    let writes = (tally.required as f32 + AUXILIARY_WEIGHT * tally.auxiliary as f32) / total;
    let sample = tally.sample as f32;
    let focus = if sample_len == 0 {
        0.0
    } else {
        sample / sample_len as f32
    };
    writes * writes * (WRITES_WEIGHT + SAMPLE_WEIGHT * (sample / total) + FOCUS_WEIGHT * focus)
}

/// The lower case of a letter, when that is one character. `İ` lowers to two,
/// so it stays itself, which is how the Turkish preset lists it.
fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

fn is_distinctive(c: char) -> bool {
    !c.is_ascii() && c.is_alphabetic()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::charsets::Source;

    use super::*;

    fn codes(text: &str) -> Vec<String> {
        detect_languages(text)
            .expect("embedded data loads")
            .into_iter()
            .map(|h| h.code)
            .collect()
    }

    fn first(text: &str) -> String {
        codes(text).into_iter().next().expect("a hint")
    }

    fn position(list: &[String], code: &str) -> usize {
        list.iter()
            .position(|c| c == code)
            .unwrap_or_else(|| panic!("{code} is not in {list:?}"))
    }

    fn charset(code: &str, base: &str, auxiliary: &str, sample: Option<&str>) -> Charset {
        Charset {
            code: code.into(),
            name: code.into(),
            script: "Latn".into(),
            ot_language: None,
            base: base.chars().collect(),
            auxiliary: auxiliary.chars().collect(),
            source: Source::Project,
            sample_glyphs: sample.map(Into::into),
        }
    }

    fn hand_made(presets: &[Charset], text: &str) -> Vec<(String, f32)> {
        Table::new(presets)
            .hints(text)
            .into_iter()
            .map(|h| (h.code, h.confidence))
            .collect()
    }

    #[test]
    fn the_criterion_order_for_dj_and_c_acute() {
        let list = codes("đć");
        let (sr, hr, pl) = (
            position(&list, "sr-Latn"),
            position(&list, "hr"),
            position(&list, "pl"),
        );
        assert_eq!(sr, 0, "{list:?}");
        assert!(sr < hr && hr < pl, "{list:?}");
    }

    #[test]
    fn a_letter_counts_the_same_in_either_case_and_any_number_of_times() {
        assert_eq!(codes("đć"), codes("ĐĆ"));
        assert_eq!(codes("đć"), codes("Đ đ đ ć Ć ć"));
    }

    #[test]
    fn text_without_distinctive_letters_gives_no_hints() {
        for text in [
            "",
            "   ",
            "The quick brown fox, 1234!",
            // Punctuation and symbols outside ASCII, some of them in the presets.
            "“quoted” — … ¿que? 。、 €",
            "😀🔤",
            // Letters that no preset has: Georgian.
            "ქართული",
        ] {
            assert!(codes(text).is_empty(), "{text:?}: {:?}", codes(text));
        }
    }

    #[test]
    fn ascii_around_a_distinctive_letter_changes_nothing() {
        assert_eq!(codes("ź"), codes("Hello ź world 42"));
    }

    #[test]
    fn serbian_latin_comes_first() {
        assert_eq!(first("Đorđe Ćirić, čaša, šuma, žaba"), "sr-Latn");
    }

    #[test]
    fn serbian_cyrillic_comes_first() {
        assert_eq!(first("Ђорђе Ћирић, љубав, њега, џак"), "sr-Cyrl");
    }

    #[test]
    fn macedonian_is_not_taken_for_serbian() {
        assert_eq!(first("ѓавол, ќерка, ѕвезда"), "mk");
    }

    #[test]
    fn polish_comes_first() {
        assert_eq!(first("Zażółć gęślą jaźń"), "pl");
    }

    #[test]
    fn vietnamese_comes_first_without_a_glyph_line() {
        assert_eq!(first("Tiếng Việt rất đẹp"), "vi");
    }

    #[test]
    fn greek_comes_first() {
        assert_eq!(first("Ελληνικά γράμματα"), "el");
    }

    #[test]
    fn turkish_dotted_capital_i_is_found() {
        assert_eq!(first("İğne"), "tr");
    }

    #[test]
    fn mixed_text_ranks_both_languages_above_the_rest() {
        let list = codes("Zażółć Ελληνικά");
        let mut top = list[..2].to_vec();
        top.sort();
        assert_eq!(top, ["el", "pl"], "{list:?}");
    }

    #[test]
    fn mixed_text_ranks_the_larger_part_first() {
        // Seven Greek letters against four Polish ones.
        assert_eq!(first("Zażółć Ελληνικά"), "el");
        // Nine Polish letters against two Greek ones.
        assert_eq!(first("Zażółć gęślą jaźń αβ"), "pl");
    }

    #[test]
    fn confidences_are_in_range_and_descending() {
        for text in ["đć", "Zażółć Ελληνικά", "Tiếng Việt", "日本語", "한국어"]
        {
            let hints = detect_languages(text).expect("hints");
            assert!(!hints.is_empty(), "{text}");
            for pair in hints.windows(2) {
                assert!(pair[0].confidence >= pair[1].confidence, "{hints:?}");
            }
            for h in &hints {
                assert!(h.confidence > 0.0 && h.confidence <= 1.0, "{h:?}");
            }
        }
    }

    #[test]
    fn a_preset_that_uses_every_letter_of_its_line_reaches_full_confidence() {
        let hints = detect_languages("šđčćž").expect("hints");
        assert_eq!(hints[0].code, "sr-Latn");
        assert!((hints[0].confidence - 1.0).abs() < 1e-6, "{hints:?}");
    }

    #[test]
    fn a_required_letter_outranks_an_auxiliary_one() {
        let got = hand_made(
            &[
                charset("aux", "a", "ć", None),
                charset("req", "ć", "", None),
            ],
            "ć",
        );
        assert_eq!(got[0].0, "req");
        assert_eq!(got[1].0, "aux");
        // Both presets have the letter; only its weight differs.
        assert!((got[0].1 - WRITES_WEIGHT).abs() < 1e-6, "{got:?}");
        assert!(
            (got[1].1 - AUXILIARY_WEIGHT * AUXILIARY_WEIGHT * WRITES_WEIGHT).abs() < 1e-6,
            "{got:?}"
        );
    }

    #[test]
    fn only_a_letter_on_the_glyph_line_earns_the_sample_score() {
        // Both presets have ć and đ in the same set and a glyph line of two;
        // only the first has the text's letter on its line.
        let got = hand_made(
            &[
                charset("off", "ćđ", "", Some("đŋ")),
                charset("on", "ćđ", "", Some("ćŋ")),
            ],
            "ć",
        );
        assert_eq!(got[0].0, "on");
        assert!(
            (got[0].1 - (WRITES_WEIGHT + SAMPLE_WEIGHT + FOCUS_WEIGHT * 0.5)).abs() < 1e-6,
            "{got:?}"
        );
        assert!((got[1].1 - WRITES_WEIGHT).abs() < 1e-6, "{got:?}");
    }

    #[test]
    fn a_shorter_glyph_line_outranks_a_longer_one() {
        let got = hand_made(
            &[
                charset("long", "ćŋ", "", Some("ćĆŋŊ")),
                charset("short", "ćŋ", "", Some("ćŋ")),
            ],
            "ć",
        );
        assert_eq!(got[0].0, "short", "{got:?}");
    }

    #[test]
    fn the_ascii_of_a_glyph_line_does_not_dilute_it() {
        let got = hand_made(
            &[
                charset("plain", "ć", "", Some("ć")),
                charset("ascii", "abcć", "", Some("abcć")),
            ],
            "ć",
        );
        assert_eq!(got[0].1, got[1].1, "{got:?}");
    }

    #[test]
    fn writing_the_whole_text_outranks_a_glyph_line_match_of_part_of_it() {
        // "line" has ć on its glyph line but cannot write đ.
        let got = hand_made(
            &[
                charset("line", "ć", "", Some("ć")),
                charset("whole", "ćđ", "", None),
            ],
            "ćđ",
        );
        assert_eq!(got[0].0, "whole", "{got:?}");
    }

    #[test]
    fn a_preset_without_a_letter_of_the_text_is_left_out() {
        let got = hand_made(
            &[charset("yes", "ć", "", None), charset("no", "đ", "", None)],
            "ć",
        );
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].0, "yes");
    }

    #[test]
    fn equal_scores_stay_in_data_order() {
        let got = hand_made(
            &[
                charset("z", "ć", "", None),
                charset("a", "ć", "", None),
                charset("m", "ć", "", None),
            ],
            "ć",
        );
        let order: Vec<&str> = got.iter().map(|(code, _)| code.as_str()).collect();
        assert_eq!(order, ["z", "a", "m"]);
    }

    #[test]
    fn letters_past_the_scan_limit_are_not_read() {
        let presets = [charset("x", "ćđ", "", None)];
        let before = "a".repeat(MAX_SCANNED_CHARS - 1);
        // The last character read is ć; the đ after it is past the limit.
        let got = hand_made(&presets, &format!("{before}ćđ"));
        assert_eq!(got.len(), 1, "{got:?}");
        // Only ć counted, so the preset writes the whole text it read.
        assert!((got[0].1 - WRITES_WEIGHT).abs() < 1e-6, "{got:?}");
        assert!(hand_made(&presets, &format!("{before}aćđ")).is_empty());
    }

    /// The criterion: under 1 ms per call. The median holds off other work on a
    /// busy runner, and a test build is slower than the app, so this is a
    /// ceiling. The table is built once, before the clock starts.
    #[test]
    fn a_call_takes_under_a_millisecond() {
        let first = Instant::now();
        detect_languages("ć").expect("table");
        println!(
            "first call, with the table if no test built it: {:?}",
            first.elapsed()
        );
        let long: String = "Zażółć gęślą jaźń Ђорђе Tiếng Việt Ελληνικά 日本語 한국어 "
            .chars()
            .cycle()
            .take(MAX_SCANNED_CHARS * 4)
            .collect();
        for text in ["Đorđe Ćirić, čaša, šuma, žaba", long.as_str()] {
            let mut times: Vec<Duration> = (0..101)
                .map(|_| {
                    let start = Instant::now();
                    let hints = detect_languages(text).expect("hints");
                    let took = start.elapsed();
                    assert!(!hints.is_empty());
                    took
                })
                .collect();
            times.sort_unstable();
            let median = times[times.len() / 2];
            println!("{} chars: median {median:?}", text.chars().count());
            assert!(median < Duration::from_millis(1), "{median:?}");
        }
    }
}
