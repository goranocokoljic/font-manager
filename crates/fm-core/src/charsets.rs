//! Character-set data: the language presets of `data/charsets`.
//!
//! Every `*.json` file in `data/charsets` (except the schema) lists languages
//! and the characters a font must cover to set them. The files are embedded at
//! build time, so the presets need no file access and no network. [`load`]
//! parses them and checks every rule of `charset.schema.json` plus the ones a
//! schema cannot say: no character twice, no overlap between the base and the
//! auxiliary sets, no language code twice across files.
//!
//! A contribution is a pull request to one data file; the tests below reject a
//! file that breaks a rule, and a new file that is not embedded here.

use std::collections::BTreeSet;

use fm_api::search::LanguagePreset;
use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value every data file has in `$schema`.
const SCHEMA_REF: &str = "./charset.schema.json";

/// The data files, embedded at build time, by file name, sorted.
const FILES: &[(&str, &str)] = &[
    (
        "arabic.json",
        include_str!("../../../data/charsets/arabic.json"),
    ),
    ("cjk.json", include_str!("../../../data/charsets/cjk.json")),
    (
        "cyrillic.json",
        include_str!("../../../data/charsets/cyrillic.json"),
    ),
    (
        "devanagari.json",
        include_str!("../../../data/charsets/devanagari.json"),
    ),
    (
        "greek.json",
        include_str!("../../../data/charsets/greek.json"),
    ),
    (
        "hebrew.json",
        include_str!("../../../data/charsets/hebrew.json"),
    ),
    (
        "latin.json",
        include_str!("../../../data/charsets/latin.json"),
    ),
    (
        "thai.json",
        include_str!("../../../data/charsets/thai.json"),
    ),
];

/// Where a language's base set comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Hyperglot (Rosetta Type).
    Hyperglot,
    /// Google Fonts language data (`gflanguages`).
    GoogleFonts,
    /// Curated in this project; `data/charsets/README.md` says how.
    Project,
}

/// One language preset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charset {
    /// BCP 47 code (`sr-Cyrl`), unique across all files.
    pub code: String,
    /// English name ("Serbian (Cyrillic)").
    pub name: String,
    /// ISO 15924 script (`Cyrl`).
    pub script: String,
    /// OpenType language-system tag, padded to four bytes (`"SRB "`).
    pub ot_language: Option<String>,
    /// Characters required to set the language, in file order.
    pub base: Vec<char>,
    /// Characters reported but never required; none of them is in `base`.
    pub auxiliary: Vec<char>,
    /// Where the base set comes from.
    pub source: Source,
}

impl Charset {
    /// The preset as the API lists it, without its pangram; see
    /// [`crate::pangrams::presets`].
    pub fn preset(&self) -> LanguagePreset {
        LanguagePreset {
            code: self.code.clone(),
            name: self.name.clone(),
            script: self.script.clone(),
            pangram: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    languages: Vec<RawLanguage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawLanguage {
    code: String,
    name: String,
    script: String,
    #[serde(default)]
    ot_language: Option<String>,
    base: String,
    #[serde(default)]
    auxiliary: Option<String>,
    source: Source,
}

/// Every language preset, in file-name order, then file order.
pub fn load() -> FmResult<Vec<Charset>> {
    load_files(FILES)
}

/// A fingerprint of the embedded data files: it changes when any file does,
/// so coverage stored against older data can be computed again.
pub fn fingerprint() -> String {
    let mut hasher = blake3::Hasher::new();
    for (name, json) in FILES {
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        // A checkout with CRLF line ends is the same data.
        hasher.update(json.replace("\r\n", "\n").as_bytes());
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex()[..16].to_owned()
}

fn load_files(files: &[(&str, &str)]) -> FmResult<Vec<Charset>> {
    let mut out = Vec::new();
    let mut codes = BTreeSet::new();
    for (file, json) in files {
        for charset in parse(file, json)? {
            if !codes.insert(charset.code.clone()) {
                return Err(damaged(
                    file,
                    &format!("{}: code is listed twice", charset.code),
                ));
            }
            out.push(charset);
        }
    }
    Ok(out)
}

/// Parses and checks one data file; `file` names it in errors.
fn parse(file: &str, json: &str) -> FmResult<Vec<Charset>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if raw.languages.is_empty() {
        return Err(damaged(file, "languages is empty"));
    }
    raw.languages
        .into_iter()
        .map(|lang| {
            check_language(&lang).map_err(|why| damaged(file, &format!("{}: {why}", lang.code)))
        })
        .collect()
}

fn check_language(lang: &RawLanguage) -> Result<Charset, String> {
    if !is_language_code(&lang.code) {
        return Err("code is not a BCP 47 language[-Script] tag".into());
    }
    if lang.name.trim().is_empty() {
        return Err("name is empty".into());
    }
    if !is_script_code(&lang.script) {
        return Err("script is not an ISO 15924 code".into());
    }
    let ot_language = match &lang.ot_language {
        Some(tag)
            if (2..=4).contains(&tag.len()) && tag.bytes().all(|b| b.is_ascii_uppercase()) =>
        {
            Some(format!("{tag:<4}"))
        }
        Some(_) => return Err("otLanguage is not 2–4 capital letters".into()),
        None => None,
    };
    let base = chars(&lang.base).map_err(|why| format!("base: {why}"))?;
    let auxiliary = match &lang.auxiliary {
        Some(aux) => chars(aux).map_err(|why| format!("auxiliary: {why}"))?,
        None => Vec::new(),
    };
    if let Some(c) = auxiliary.iter().find(|c| base.contains(c)) {
        return Err(format!("auxiliary repeats base character {c:?}"));
    }
    Ok(Charset {
        code: lang.code.clone(),
        name: lang.name.clone(),
        script: lang.script.clone(),
        ot_language,
        base,
        auxiliary,
        source: lang.source,
    })
}

/// Splits a space-separated character list; each item is one scalar value.
fn chars(list: &str) -> Result<Vec<char>, String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for item in list.split(' ') {
        let mut it = item.chars();
        let c = match (it.next(), it.next()) {
            (Some(c), None) => c,
            (None, _) => return Err("empty item (two spaces, or a space at an end)".into()),
            (Some(_), Some(_)) => return Err(format!("{item:?} is more than one character")),
        };
        if c.is_whitespace() || c.is_control() {
            return Err(format!(
                "U+{:04X} is white space or a control",
                u32::from(c)
            ));
        }
        if !seen.insert(c) {
            return Err(format!("{c:?} is listed twice"));
        }
        out.push(c);
    }
    Ok(out)
}

fn is_language_code(code: &str) -> bool {
    let mut parts = code.split('-');
    let lang_ok = parts
        .next()
        .is_some_and(|l| (2..=3).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_lowercase()));
    let script_ok = parts.next().is_none_or(is_script_code);
    lang_ok && script_ok && parts.next().is_none()
}

fn is_script_code(script: &str) -> bool {
    let b = script.as_bytes();
    b.len() == 4 && b[0].is_ascii_uppercase() && b[1..].iter().all(u8::is_ascii_lowercase)
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/charsets/{file}: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/charsets")
    }

    fn all() -> BTreeMap<String, Charset> {
        load()
            .expect("embedded data loads")
            .into_iter()
            .map(|c| (c.code.clone(), c))
            .collect()
    }

    fn one(json_language: &str) -> FmResult<Vec<Charset>> {
        parse(
            "test.json",
            &format!(
                r#"{{"$schema": "./charset.schema.json", "languages": [{{{json_language}}}]}}"#
            ),
        )
    }

    const OK: &str = r#""code": "xx", "name": "X", "script": "Latn", "source": "project""#;

    #[test]
    fn every_data_file_is_embedded() {
        let mut on_disk: Vec<String> = std::fs::read_dir(data_dir())
            .expect("data/charsets exists")
            .map(|e| {
                e.expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|n| n.ends_with(".json") && n != "charset.schema.json")
            .collect();
        on_disk.sort();
        let embedded: Vec<&str> = FILES.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            on_disk, embedded,
            "add new data files to FILES in charsets.rs"
        );
    }

    #[test]
    fn embedded_files_are_the_files_on_disk() {
        for (name, json) in FILES {
            let disk = std::fs::read_to_string(data_dir().join(name)).expect("readable");
            assert_eq!(
                disk.replace("\r\n", "\n"),
                json.replace("\r\n", "\n"),
                "{name}"
            );
        }
    }

    #[test]
    fn the_fingerprint_is_stable_and_short() {
        assert_eq!(fingerprint(), fingerprint());
        assert_eq!(fingerprint().len(), 16);
    }

    #[test]
    fn every_data_file_is_valid() {
        let charsets = load().expect("embedded data loads");
        assert!(charsets.len() >= 50, "{} presets", charsets.len());
    }

    #[test]
    fn the_required_languages_are_present() {
        let all = all();
        let required = [
            // Latin-script European languages.
            "en", "de", "fr", "es", "it", "pt", "nl", "da", "nb", "sv", "fi", "is", "pl", "cs",
            "sk", "hu", "ro", "hr", "sl", "sr-Latn", "bs", "sq", "et", "lv", "lt", "ga", "cy",
            "mt", "ca", "eu", "gl", // Cyrillic.
            "ru", "uk", "be", "bg", "sr-Cyrl", "mk",
            // Greek, Turkish, Vietnamese, Arabic, Hebrew, Thai, Devanagari, CJK.
            "el", "tr", "vi", "ar", "fa", "he", "th", "hi", "mr", "ne", "zh-Hans", "zh-Hant", "ja",
            "ko",
        ];
        for code in required {
            let c = all.get(code).unwrap_or_else(|| panic!("{code} missing"));
            assert!(!c.base.is_empty(), "{code} has no base characters");
        }
    }

    /// The Unicode blocks each script's characters may come from; catches a
    /// look-alike from another script (Cyrillic `а` in a Latin list).
    fn allowed(script: &str, c: char) -> bool {
        let u = u32::from(c);
        let r = |a: u32, b: u32| (a..=b).contains(&u);
        let cjk_punct = r(0x3000, 0x303F) || r(0xFF00, 0xFFEF) || matches!(u, 0x201C | 0x201D);
        match script {
            "Latn" => c.is_ascii_alphabetic() || r(0x00A1, 0x024F) || r(0x1E00, 0x1EFF),
            "Cyrl" => r(0x0400, 0x04FF),
            "Grek" => r(0x0370, 0x03FF),
            "Arab" => r(0x0600, 0x06FF),
            "Hebr" => r(0x0590, 0x05FF),
            "Thai" => r(0x0E00, 0x0E7F),
            "Deva" => r(0x0900, 0x097F),
            "Hans" | "Hant" => r(0x4E00, 0x9FFF) || cjk_punct,
            "Jpan" => r(0x3040, 0x30FF) || r(0x4E00, 0x9FFF) || cjk_punct,
            "Kore" => r(0xAC00, 0xD7A3) || r(0x1100, 0x11FF) || r(0x3130, 0x318F),
            _ => false,
        }
    }

    #[test]
    fn characters_belong_to_the_script() {
        for c in all().values() {
            for ch in c.base.iter().chain(&c.auxiliary) {
                assert!(
                    allowed(&c.script, *ch),
                    "{}: U+{:04X} is outside {}",
                    c.code,
                    u32::from(*ch),
                    c.script
                );
            }
        }
    }

    #[test]
    fn spot_checks_on_the_data() {
        let all = all();
        let sr = &all["sr-Cyrl"];
        assert!(sr.base.contains(&'Ђ') && sr.base.contains(&'џ'));
        assert!(!sr.base.contains(&'Ё'));
        assert_eq!(sr.ot_language.as_deref(), Some("SRB "));
        let tr = &all["tr"];
        assert!(tr.base.contains(&'İ') && tr.base.contains(&'ı') && tr.base.contains(&'ğ'));
        // 26 letters, đ, ă â ê ô ơ ư, and 12 vowels × 5 tones, in both cases.
        let vi = &all["vi"];
        assert_eq!(vi.base.len(), 2 * (26 + 1 + 6 + 12 * 5));
        assert!(vi.base.contains(&'ệ') && vi.base.contains(&'Ữ'));
        assert_eq!(
            all["ro"]
                .base
                .iter()
                .filter(|c| matches!(c, 'ș' | 'ț'))
                .count(),
            2
        );
        assert_ne!(all["zh-Hans"].base, all["zh-Hant"].base);
        assert!(all["zh-Hans"].base.contains(&'这') && all["zh-Hant"].base.contains(&'這'));
        assert!(all["ja"].base.contains(&'あ') && all["ja"].base.contains(&'ア'));
        assert_eq!(
            all["el"].base.iter().filter(|c| c.is_uppercase()).count(),
            24 + 9
        );
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let text = std::fs::read_to_string(data_dir().join("charset.schema.json")).expect("schema");
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
        assert_eq!(keys(&schema["properties"]), ["$schema", "languages"]);
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        let lang = &schema["$defs"]["language"];
        assert_eq!(
            keys(&lang["properties"]),
            [
                "auxiliary",
                "base",
                "code",
                "name",
                "otLanguage",
                "script",
                "source"
            ]
        );
        assert_eq!(
            strings(&lang["required"]),
            ["base", "code", "name", "script", "source"]
        );
        assert_eq!(
            strings(&lang["properties"]["source"]["enum"]),
            ["google-fonts", "hyperglot", "project"]
        );
        assert_eq!(lang["additionalProperties"], false);
        // Every optional field the schema names is accepted by the loader.
        let full = one(&format!(
            r#"{OK}, "otLanguage": "XX", "base": "a", "auxiliary": "b""#
        ))
        .expect("all fields parse");
        assert_eq!(full[0].ot_language.as_deref(), Some("XX  "));
        assert_eq!(full[0].auxiliary, ['b']);
    }

    #[test]
    fn every_data_file_points_at_the_schema() {
        for (name, json) in FILES {
            let v: Value = serde_json::from_str(json).expect("JSON");
            assert_eq!(v["$schema"], SCHEMA_REF, "{name}");
        }
    }

    #[test]
    fn preset_carries_code_name_and_script() {
        let p = all()["sr-Cyrl"].preset();
        assert_eq!(
            (p.code.as_str(), p.name.as_str(), p.script.as_str()),
            ("sr-Cyrl", "Serbian (Cyrillic)", "Cyrl")
        );
    }

    #[test]
    fn a_minimal_language_parses() {
        let c = one(&format!(r#"{OK}, "base": "a b ć""#)).expect("valid");
        assert_eq!(c[0].base, ['a', 'b', 'ć']);
        assert_eq!(c[0].source, Source::Project);
        assert!(c[0].ot_language.is_none() && c[0].auxiliary.is_empty());
    }

    #[test]
    fn broken_data_is_rejected() {
        let cases = [
            (r#""base": "a a""#, "listed twice"),
            (r#""base": "a  b""#, "empty item"),
            (r#""base": "a ""#, "empty item"),
            (r#""base": """#, "empty item"),
            (r#""base": "ab""#, "more than one character"),
            (r#""base": "a\tb""#, "more than one character"),
            (r#""base": "a \u0007""#, "control"),
            (r#""base": "a", "auxiliary": "a""#, "auxiliary repeats"),
            (r#""base": "a", "otLanguage": "srb""#, "otLanguage"),
            (r#""base": "a", "extra": 1"#, "unknown field"),
        ];
        for (fields, why) in cases {
            let err = one(&format!("{OK}, {fields}")).expect_err(fields);
            let detail = err.detail().unwrap_or_default();
            assert!(detail.contains(why), "{fields}: {detail}");
            assert!(detail.starts_with("data/charsets/test.json"), "{detail}");
        }
    }

    #[test]
    fn bad_codes_and_names_are_rejected() {
        for (field, why) in [
            (r#""code": "EN", "name": "X", "script": "Latn""#, "code"),
            (
                r#""code": "sr-cyrl", "name": "X", "script": "Latn""#,
                "code",
            ),
            (
                r#""code": "sr-Cyrl-RS", "name": "X", "script": "Latn""#,
                "code",
            ),
            (r#""code": "xx", "name": " ", "script": "Latn""#, "name"),
            (r#""code": "xx", "name": "X", "script": "LATN""#, "script"),
        ] {
            let err = parse(
                "t.json",
                &format!(
                    r#"{{"$schema": "./charset.schema.json", "languages": [{{{field}, "base": "a", "source": "project"}}]}}"#
                ),
            )
            .expect_err(field);
            assert!(err.detail().unwrap_or_default().contains(why), "{field}");
        }
    }

    #[test]
    fn bad_files_are_rejected() {
        for json in [
            "{",
            r#"{"languages": []}"#,
            r#"{"$schema": "./charset.schema.json", "languages": []}"#,
            r#"{"$schema": "other.json", "languages": [{"code": "xx", "name": "X", "script": "Latn", "base": "a", "source": "project"}]}"#,
            r#"{"$schema": "./charset.schema.json", "languages": [{"code": "xx", "name": "X", "script": "Latn", "base": "a", "source": "wikipedia"}]}"#,
        ] {
            assert!(parse("t.json", json).is_err(), "{json}");
        }
    }

    #[test]
    fn a_code_in_two_files_is_rejected() {
        let file = r#"{"$schema": "./charset.schema.json", "languages": [{"code": "xx", "name": "X", "script": "Latn", "base": "a", "source": "project"}]}"#;
        let err = load_files(&[("a.json", file), ("b.json", file)]).expect_err("duplicate");
        assert!(
            err.detail()
                .unwrap_or_default()
                .contains("xx: code is listed twice")
        );
    }
}
