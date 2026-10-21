//! Pangram data: the sample sentences of `data/pangrams`.
//!
//! `pangrams.json` gives each language preset of `data/charsets` at most one
//! pangram, or a sample sentence where the language has no true pangram. The
//! file is embedded at build time. [`load`] parses it and checks every rule of
//! `pangram.schema.json` plus the ones a schema cannot say: each code is a
//! language preset, no code twice. The tests check that every letter of a
//! pangram is in its preset's characters.

use std::collections::{BTreeMap, BTreeSet};

use fm_api::search::LanguagePreset;
use serde::Deserialize;

use crate::charsets::{self, Charset};
use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./pangram.schema.json";

/// The data file, embedded at build time.
const FILE: (&str, &str) = (
    "pangrams.json",
    include_str!("../../../data/pangrams/pangrams.json"),
);

/// Whether a text uses every letter of its alphabet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A true pangram.
    Pangram,
    /// A sample sentence, where the language has no true pangram (CJK).
    Sample,
}

/// Where a text comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Wikipedia's list of pangrams.
    Wikipedia,
    /// Written in this project; `data/pangrams/README.md` says how.
    Project,
}

/// One language's pangram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pangram {
    /// BCP 47 code of a `data/charsets` preset.
    pub code: String,
    /// Pangram or sample sentence.
    pub kind: Kind,
    /// The text.
    pub text: String,
    /// Where the text comes from.
    pub source: Source,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    pangrams: Vec<RawPangram>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPangram {
    code: String,
    kind: Kind,
    text: String,
    source: Source,
}

/// Every pangram, in file order.
pub fn load() -> FmResult<Vec<Pangram>> {
    let codes: BTreeSet<String> = charsets::load()?.into_iter().map(|c| c.code).collect();
    parse(FILE.0, FILE.1, &codes)
}

/// The language presets as the API lists them, each with its pangram.
pub fn presets() -> FmResult<Vec<LanguagePreset>> {
    let charsets = charsets::load()?;
    let codes: BTreeSet<String> = charsets.iter().map(|c| c.code.clone()).collect();
    let mut texts: BTreeMap<String, String> = parse(FILE.0, FILE.1, &codes)?
        .into_iter()
        .map(|p| (p.code, p.text))
        .collect();
    Ok(charsets
        .iter()
        .map(|c| LanguagePreset {
            pangram: texts.remove(&c.code),
            ..Charset::preset(c)
        })
        .collect())
}

/// Parses and checks the data file; `codes` are the known language presets.
fn parse(file: &str, json: &str, codes: &BTreeSet<String>) -> FmResult<Vec<Pangram>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if raw.pangrams.is_empty() {
        return Err(damaged(file, "pangrams is empty"));
    }
    let mut seen = BTreeSet::new();
    raw.pangrams
        .into_iter()
        .map(|p| {
            if !codes.contains(&p.code) {
                return Err(damaged(
                    file,
                    &format!("{}: code is not a data/charsets preset", p.code),
                ));
            }
            if !seen.insert(p.code.clone()) {
                return Err(damaged(file, &format!("{}: code is listed twice", p.code)));
            }
            if p.text.trim().is_empty() || p.text.trim() != p.text {
                return Err(damaged(
                    file,
                    &format!("{}: text is empty or has white space at an end", p.code),
                ));
            }
            Ok(Pangram {
                code: p.code,
                kind: p.kind,
                text: p.text,
                source: p.source,
            })
        })
        .collect()
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/pangrams/{file}: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pangrams")
    }

    fn by_code() -> BTreeMap<String, Pangram> {
        load()
            .expect("embedded data loads")
            .into_iter()
            .map(|p| (p.code.clone(), p))
            .collect()
    }

    fn codes() -> BTreeSet<String> {
        ["en", "sr-Cyrl"].map(String::from).into()
    }

    fn one(entry: &str) -> FmResult<Vec<Pangram>> {
        parse(
            "test.json",
            &format!(r#"{{"$schema": "./pangram.schema.json", "pangrams": [{{{entry}}}]}}"#),
            &codes(),
        )
    }

    /// Punctuation a pangram may use without its preset listing it.
    const PUNCTUATION: &str = "!\"'(),-.:;?«»—–…‘’“”„";

    #[test]
    fn the_data_file_is_the_only_file_and_is_embedded() {
        let mut on_disk: Vec<String> = std::fs::read_dir(data_dir())
            .expect("data/pangrams exists")
            .map(|e| {
                e.expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|n| n.ends_with(".json") && n != "pangram.schema.json")
            .collect();
        on_disk.sort();
        assert_eq!(on_disk, [FILE.0]);
        let disk = std::fs::read_to_string(data_dir().join(FILE.0)).expect("readable");
        assert_eq!(disk.replace("\r\n", "\n"), FILE.1.replace("\r\n", "\n"));
    }

    #[test]
    fn the_data_file_is_valid() {
        let all = load().expect("embedded data loads");
        assert!(all.len() >= 12, "{} pangrams", all.len());
    }

    #[test]
    fn every_script_has_a_pangram() {
        let all = by_code();
        let charsets: BTreeMap<String, Charset> = charsets::load()
            .expect("charsets")
            .into_iter()
            .map(|c| (c.code.clone(), c))
            .collect();
        let scripts: BTreeSet<&str> = charsets.values().map(|c| c.script.as_str()).collect();
        for script in scripts {
            assert!(
                all.keys().any(|code| charsets[code].script == script),
                "no pangram for script {script}"
            );
        }
        for code in ["sr-Latn", "sr-Cyrl", "zh-Hans", "zh-Hant", "ja", "ko"] {
            assert!(all.contains_key(code), "{code} has no pangram");
        }
        // The pangrams the preview offered before this data existed.
        for code in ["en", "de", "fr", "es", "pl", "cs", "ru", "el", "he", "ja"] {
            assert!(all.contains_key(code), "{code} lost its pangram");
        }
    }

    #[test]
    fn every_letter_of_a_pangram_is_in_its_preset() {
        let charsets: BTreeMap<String, Charset> = charsets::load()
            .expect("charsets")
            .into_iter()
            .map(|c| (c.code.clone(), c))
            .collect();
        for p in load().expect("pangrams") {
            let c = &charsets[&p.code];
            for ch in p.text.chars() {
                if ch.is_whitespace() || PUNCTUATION.contains(ch) {
                    continue;
                }
                assert!(
                    c.base.contains(&ch) || c.auxiliary.contains(&ch),
                    "{}: {ch:?} (U+{:04X}) is not in the preset",
                    p.code,
                    u32::from(ch)
                );
            }
        }
    }

    #[test]
    fn spot_checks_on_the_data() {
        let all = by_code();
        assert_eq!(
            all["en"].text,
            "The quick brown fox jumps over the lazy dog"
        );
        assert!(all["sr-Cyrl"].text.contains('џ') && all["sr-Latn"].text.contains('đ'));
        assert_eq!(all["en"].kind, Kind::Pangram);
        assert_eq!(all["zh-Hans"].kind, Kind::Sample);
        assert_eq!(all["ko"].source, Source::Project);
    }

    #[test]
    fn presets_carry_the_pangram_of_their_language() {
        let presets = presets().expect("presets");
        assert_eq!(presets.len(), charsets::load().expect("charsets").len());
        let sr = presets.iter().find(|p| p.code == "sr-Cyrl").expect("sr");
        assert_eq!(
            sr.pangram.as_deref(),
            Some(by_code()["sr-Cyrl"].text.as_str())
        );
        assert_eq!(sr.name, "Serbian (Cyrillic)");
        // A preset without data has no pangram, not an empty one.
        let all = by_code();
        for p in &presets {
            assert_eq!(p.pangram.is_some(), all.contains_key(&p.code), "{}", p.code);
        }
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let text = std::fs::read_to_string(data_dir().join("pangram.schema.json")).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
            k.sort();
            k
        };
        let strings = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v
                .as_array()
                .expect("array")
                .iter()
                .map(|s| s.as_str().expect("string").to_owned())
                .collect();
            k.sort();
            k
        };
        assert_eq!(keys(&schema["properties"]), ["$schema", "pangrams"]);
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        let entry = &schema["$defs"]["pangram"];
        assert_eq!(
            keys(&entry["properties"]),
            ["code", "kind", "source", "text"]
        );
        assert_eq!(
            strings(&entry["required"]),
            ["code", "kind", "source", "text"]
        );
        assert_eq!(
            strings(&entry["properties"]["kind"]["enum"]),
            ["pangram", "sample"]
        );
        assert_eq!(
            strings(&entry["properties"]["source"]["enum"]),
            ["project", "wikipedia"]
        );
        assert_eq!(entry["additionalProperties"], false);
        let v: Value = serde_json::from_str(FILE.1).expect("JSON");
        assert_eq!(v["$schema"], SCHEMA_REF);
    }

    #[test]
    fn broken_data_is_rejected() {
        let ok = r#""kind": "pangram", "source": "project""#;
        let cases = [
            (r#""code": "xx", "text": "a""#, "not a data/charsets preset"),
            (r#""code": "en", "text": """#, "text is empty"),
            (r#""code": "en", "text": " a""#, "white space"),
            (r#""code": "en", "text": "a", "extra": 1"#, "unknown field"),
        ];
        for (fields, why) in cases {
            let err = one(&format!("{ok}, {fields}")).expect_err(fields);
            let detail = err.detail().unwrap_or_default();
            assert!(detail.contains(why), "{fields}: {detail}");
            assert!(detail.starts_with("data/pangrams/test.json"), "{detail}");
        }
        assert!(one(r#""code": "en", "text": "a", "kind": "poem", "source": "project""#).is_err());
        assert!(one(r#""code": "en", "text": "a", "kind": "pangram", "source": "blog""#).is_err());
    }

    #[test]
    fn a_code_twice_and_bad_files_are_rejected() {
        let entry = r#"{"code": "en", "kind": "pangram", "text": "a", "source": "project"}"#;
        let twice =
            format!(r#"{{"$schema": "./pangram.schema.json", "pangrams": [{entry}, {entry}]}}"#);
        let err = parse("t.json", &twice, &codes()).expect_err("duplicate");
        assert!(
            err.detail()
                .unwrap_or_default()
                .contains("en: code is listed twice")
        );
        for json in [
            "{",
            r#"{"$schema": "./pangram.schema.json", "pangrams": []}"#,
            r#"{"$schema": "other.json", "pangrams": [{"code": "en", "kind": "pangram", "text": "a", "source": "project"}]}"#,
        ] {
            assert!(parse("t.json", json, &codes()).is_err(), "{json}");
        }
    }
}
