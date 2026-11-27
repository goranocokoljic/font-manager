//! The one order of the language presets (E58.1, ADR 0028).
//!
//! `list_languages` answers the presets of `data/charsets` in three parts:
//!
//! 1. the user's languages (`Settings::my_languages`) in the user's order;
//!    while none is set, the presets of the system's language;
//! 2. the most used languages of `data/charsets/order.json` that are not in
//!    part 1, in the file's order;
//! 3. every other preset from A to Z by its English name.
//!
//! Every adapter (the app, the CLI, MCP) gets this answer; none sorts again.
//! `order.json` is community-editable like the other data files: a code no
//! preset has is refused by [`most_used`] and by the tests below.

use std::collections::BTreeSet;

use fm_api::search::{LanguageGroup, LanguagePreset};
use serde::Deserialize;

use crate::charsets;
use crate::error::{FmError, FmResult};

/// The data file, embedded at build time: (file name, JSON).
pub const FILE: (&str, &str) = (
    "order.json",
    include_str!("../../../data/charsets/order.json"),
);

/// The schema next to it.
pub const SCHEMA_FILE: &str = "order.schema.json";

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./order.schema.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    most_used: Vec<String>,
}

/// The most used languages of `order.json`, in the file's order; every code
/// is a preset of `data/charsets`.
pub fn most_used() -> FmResult<Vec<String>> {
    let codes: BTreeSet<String> = charsets::load()?.into_iter().map(|c| c.code).collect();
    parse(FILE.0, FILE.1, &codes)
}

/// Parses and checks the data file; `codes` are the known presets.
fn parse(file: &str, json: &str, codes: &BTreeSet<String>) -> FmResult<Vec<String>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if raw.most_used.is_empty() {
        return Err(damaged(file, "mostUsed is empty"));
    }
    let mut seen = BTreeSet::new();
    for code in &raw.most_used {
        if !charsets::is_language_code(code) {
            return Err(damaged(
                file,
                &format!("{code}: not a BCP 47 language[-Script] tag"),
            ));
        }
        if !codes.contains(code) {
            return Err(damaged(
                file,
                &format!("{code}: code is not a data/charsets preset"),
            ));
        }
        if !seen.insert(code.as_str()) {
            return Err(damaged(file, &format!("{code}: code is listed twice")));
        }
    }
    Ok(raw.most_used)
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/charsets/{file}: {detail}"))
}

/// The primary language and the script a locale tag names: `sr-Latn-RS` →
/// (`sr`, `Some("Latn")`), `de_AT` → (`de`, `None`). Case does not matter.
fn split_tag(tag: &str) -> Option<(String, Option<String>)> {
    let mut parts = tag.trim().split(['-', '_']);
    let lang = parts.next()?.to_ascii_lowercase();
    if !(2..=3).contains(&lang.len()) || !lang.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let script = parts
        .find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(|p| {
            let lower = p.to_ascii_lowercase();
            let mut s = lower[..1].to_ascii_uppercase();
            s.push_str(&lower[1..]);
            s
        });
    Some((lang, script))
}

/// The codes of the presets of the system's language `tag`, in `presets`
/// order: the preset of the script the tag names when there is one, else
/// every preset of the language (`sr` → both Serbian scripts). Empty for a
/// language with no preset or a tag that is no language.
pub fn system_codes(tag: &str, presets: &[LanguagePreset]) -> Vec<String> {
    let Some((lang, script)) = split_tag(tag) else {
        return Vec::new();
    };
    let of_language: Vec<&LanguagePreset> = presets
        .iter()
        .filter(|p| p.code.split('-').next() == Some(lang.as_str()))
        .collect();
    let of_script: Vec<&LanguagePreset> = script
        .as_deref()
        .map(|s| {
            of_language
                .iter()
                .copied()
                .filter(|p| p.code.split('-').nth(1) == Some(s) || p.script == s)
                .collect()
        })
        .unwrap_or_default();
    let chosen = if of_script.is_empty() {
        of_language
    } else {
        of_script
    };
    chosen.into_iter().map(|p| p.code.clone()).collect()
}

/// `presets` in the one order, each with its [`LanguageGroup`]. `my_languages`
/// lead in their order (unknown ids and repeats are skipped); while it is
/// empty, the presets of `system` lead ([`system_codes`]). Then `most_used`
/// in its order without the ones already listed, then the rest by English
/// name (case-blind), then code. No preset is listed twice.
pub fn arrange(
    presets: Vec<LanguagePreset>,
    most_used: &[String],
    my_languages: &[String],
    system: Option<&str>,
) -> Vec<LanguagePreset> {
    let lead: Vec<String> = if my_languages.is_empty() {
        system
            .map(|tag| system_codes(tag, &presets))
            .unwrap_or_default()
    } else {
        my_languages.to_vec()
    };
    let mut rest: Vec<Option<LanguagePreset>> = presets.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(rest.len());
    let mut take = |code: &str, group: LanguageGroup, out: &mut Vec<LanguagePreset>| {
        if let Some(slot) = rest
            .iter_mut()
            .find(|p| p.as_ref().is_some_and(|p| p.code == code))
            && let Some(preset) = slot.take()
        {
            out.push(LanguagePreset {
                group: Some(group),
                ..preset
            });
        }
    };
    for code in &lead {
        take(code, LanguageGroup::Mine, &mut out);
    }
    for code in most_used {
        take(code, LanguageGroup::MostUsed, &mut out);
    }
    let mut others: Vec<LanguagePreset> = rest
        .into_iter()
        .flatten()
        .map(|p| LanguagePreset {
            group: Some(LanguageGroup::Others),
            ..p
        })
        .collect();
    others.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.code.cmp(&b.code))
    });
    out.extend(others);
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/charsets")
    }

    fn known() -> BTreeSet<String> {
        charsets::load()
            .expect("presets")
            .into_iter()
            .map(|c| c.code)
            .collect()
    }

    fn preset(code: &str, name: &str, script: &str) -> LanguagePreset {
        LanguagePreset {
            code: code.into(),
            name: name.into(),
            script: script.into(),
            pangram: None,
            sample_glyphs: None,
            group: None,
        }
    }

    fn codes(list: &[LanguagePreset]) -> Vec<&str> {
        list.iter().map(|p| p.code.as_str()).collect()
    }

    fn real() -> Vec<LanguagePreset> {
        crate::pangrams::presets().expect("presets")
    }

    #[test]
    fn the_data_file_is_valid_and_drafted_as_decided() {
        assert_eq!(
            most_used().expect("order.json loads"),
            [
                "en", "es", "fr", "de", "pt", "it", "ru", "zh-Hans", "ja", "ar"
            ]
        );
    }

    #[test]
    fn the_embedded_file_is_the_file_on_disk() {
        let disk = std::fs::read_to_string(data_dir().join(FILE.0)).expect("readable");
        assert_eq!(disk.replace("\r\n", "\n"), FILE.1.replace("\r\n", "\n"));
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let text = std::fs::read_to_string(data_dir().join(SCHEMA_FILE)).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"],
            serde_json::json!(["$schema", "mostUsed"])
        );
        let list = &schema["properties"]["mostUsed"];
        assert_eq!(list["uniqueItems"], true);
        assert_eq!(list["minItems"], 1);
        let file: Value = serde_json::from_str(FILE.1).expect("file is JSON");
        assert_eq!(file["$schema"], SCHEMA_REF);
        let keys: BTreeSet<&str> = file
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        let allowed: BTreeSet<&str> = schema["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .map(String::as_str)
            .collect();
        assert!(keys.is_subset(&allowed), "{keys:?} not in {allowed:?}");
        // The schema's pattern is the loader's code rule.
        assert_eq!(
            list["items"]["pattern"], "^[a-z]{2,3}(-[A-Z][a-z]{3})?$",
            "keep the pattern and charsets::is_language_code in step"
        );
    }

    #[test]
    fn a_code_that_no_preset_has_is_refused() {
        let known = known();
        let bad = r#"{"$schema": "./order.schema.json", "mostUsed": ["en", "xx"]}"#;
        let err = parse("order.json", bad, &known).expect_err("xx has no preset");
        assert!(err.to_string().contains("xx"), "{err}");
        for broken in [
            r#"{"$schema": "./order.schema.json", "mostUsed": []}"#,
            r#"{"$schema": "./order.schema.json", "mostUsed": ["en", "en"]}"#,
            r#"{"$schema": "./order.schema.json", "mostUsed": ["EN"]}"#,
            r#"{"$schema": "./charset.schema.json", "mostUsed": ["en"]}"#,
            r#"{"$schema": "./order.schema.json", "mostUsed": ["en"], "extra": 1}"#,
            r#"{"$schema": "./order.schema.json"}"#,
        ] {
            assert!(parse("order.json", broken, &known).is_err(), "{broken}");
        }
        let ok = r#"{"$schema": "./order.schema.json", "mostUsed": ["sr-Latn", "en"]}"#;
        assert_eq!(
            parse("order.json", ok, &known).expect("valid"),
            ["sr-Latn", "en"]
        );
    }

    #[test]
    fn three_languages_lead_in_the_users_order_then_most_used_then_a_to_z() {
        let mine = ["sr-Latn", "de", "ja"].map(String::from);
        let list = arrange(real(), &most_used().expect("order"), &mine, Some("en"));
        let all = codes(&list);
        assert_eq!(
            all[..11],
            [
                "sr-Latn", "de", "ja", "en", "es", "fr", "pt", "it", "ru", "zh-Hans", "ar"
            ]
        );
        let groups: Vec<_> = list.iter().map(|p| p.group).collect();
        assert!(groups[..3].iter().all(|g| *g == Some(LanguageGroup::Mine)));
        assert!(
            groups[3..11]
                .iter()
                .all(|g| *g == Some(LanguageGroup::MostUsed))
        );
        assert!(
            groups[11..]
                .iter()
                .all(|g| *g == Some(LanguageGroup::Others))
        );
        let names: Vec<String> = list[11..].iter().map(|p| p.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "the others are A to Z by English name");
        assert_eq!(list[11].name, "Albanian");
        // Every preset once.
        let unique: BTreeSet<&str> = all.iter().copied().collect();
        assert_eq!(unique.len(), all.len());
        assert_eq!(all.len(), known().len());
    }

    #[test]
    fn a_language_set_that_is_also_most_used_is_listed_once() {
        let mine = ["fr", "pl"].map(String::from);
        let list = arrange(real(), &most_used().expect("order"), &mine, None);
        let all = codes(&list);
        assert_eq!(all.iter().filter(|c| **c == "fr").count(), 1);
        assert_eq!(all[..3], ["fr", "pl", "en"]);
        assert_eq!(list[0].group, Some(LanguageGroup::Mine));
    }

    #[test]
    fn unknown_or_repeated_ids_of_mine_are_skipped() {
        let mine = ["de", "zz-Gone", "de"].map(String::from);
        let list = arrange(real(), &most_used().expect("order"), &mine, None);
        assert_eq!(codes(&list)[..2], ["de", "en"]);
        assert_eq!(list[1].group, Some(LanguageGroup::MostUsed));
    }

    #[test]
    fn with_no_language_set_the_system_language_with_one_preset_leads() {
        let list = arrange(real(), &most_used().expect("order"), &[], Some("pl-PL"));
        assert_eq!(codes(&list)[..3], ["pl", "en", "es"]);
        assert_eq!(list[0].group, Some(LanguageGroup::Mine));
        // A most-used system language leads and is not listed again.
        let list = arrange(real(), &most_used().expect("order"), &[], Some("de"));
        assert_eq!(codes(&list)[..3], ["de", "en", "es"]);
        assert_eq!(codes(&list).iter().filter(|c| **c == "de").count(), 1);
    }

    #[test]
    fn a_system_language_with_two_scripts_leads_with_the_script_it_names_else_both() {
        let order = most_used().expect("order");
        let named = arrange(real(), &order, &[], Some("sr-Latn-RS"));
        assert_eq!(codes(&named)[..2], ["sr-Latn", "en"]);
        // No script named: both, in file order (cyrillic.json before latin.json).
        let both = arrange(real(), &order, &[], Some("sr"));
        assert_eq!(codes(&both)[..3], ["sr-Cyrl", "sr-Latn", "en"]);
        assert_eq!(both[1].group, Some(LanguageGroup::Mine));
        let zh = arrange(real(), &order, &[], Some("zh_hant"));
        assert_eq!(codes(&zh)[..2], ["zh-Hant", "en"]);
    }

    #[test]
    fn a_system_language_without_a_preset_leads_nothing() {
        let order = most_used().expect("order");
        for tag in ["sw", "x", "", "123"] {
            let list = arrange(real(), &order, &[], Some(tag));
            assert_eq!(codes(&list)[..2], ["en", "es"], "{tag}");
            assert_eq!(list[0].group, Some(LanguageGroup::MostUsed), "{tag}");
        }
        let none = arrange(real(), &order, &[], None);
        assert_eq!(codes(&none)[0], "en");
    }

    #[test]
    fn the_users_languages_win_over_the_system_language() {
        let mine = ["it".to_owned()];
        let list = arrange(real(), &most_used().expect("order"), &mine, Some("pl"));
        assert_eq!(codes(&list)[..2], ["it", "en"]);
        let pl = list.iter().position(|p| p.code == "pl").expect("pl");
        assert_eq!(list[pl].group, Some(LanguageGroup::Others));
    }

    #[test]
    fn the_others_sort_by_name_case_blind_then_by_code() {
        let presets = vec![
            preset("b", "beta", "Latn"),
            preset("a", "Alpha", "Latn"),
            preset("c2", "Gamma", "Latn"),
            preset("c1", "Gamma", "Latn"),
        ];
        assert_eq!(
            codes(&arrange(presets, &[], &[], None)),
            ["a", "b", "c1", "c2"]
        );
    }

    /// The cases the Rust MockApi and the TypeScript mock also run, so the
    /// three give the same order for the same settings.
    #[test]
    fn the_shared_cases_hold() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/language-order/cases.json");
        let text = std::fs::read_to_string(path).expect("cases");
        let cases: Value = serde_json::from_str(&text).expect("JSON");
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .expect("array")
                .iter()
                .map(|s| s.as_str().expect("string").to_owned())
                .collect()
        };
        let presets: Vec<LanguagePreset> = cases["presets"]
            .as_array()
            .expect("presets")
            .iter()
            .map(|p| {
                preset(
                    p["code"].as_str().expect("code"),
                    p["name"].as_str().expect("name"),
                    p["script"].as_str().expect("script"),
                )
            })
            .collect();
        let most_used = strings(&cases["mostUsed"]);
        let list = cases["cases"].as_array().expect("cases");
        assert!(!list.is_empty());
        for case in list {
            let name = case["name"].as_str().expect("name");
            let got = arrange(
                presets.clone(),
                &most_used,
                &strings(&case["myLanguages"]),
                case["system"].as_str(),
            );
            let got: Vec<Value> = got
                .iter()
                .map(|p| serde_json::json!([p.code, p.group]))
                .collect();
            assert_eq!(Value::Array(got), case["expected"], "{name}");
        }
    }
}
