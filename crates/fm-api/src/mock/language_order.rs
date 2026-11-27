//! The language order of the mock (E58.1, ADR 0028): a copy of
//! `fm_core::language_order::arrange` (fm-api cannot use fm-core).
//! `fixtures/language-order/cases.json` runs against both, and against the
//! TypeScript mock, so they give the same order for the same settings.

use crate::search::{LanguageGroup, LanguagePreset};

/// `data/charsets/order.json`, the same file fm-core reads.
const ORDER_JSON: &str = include_str!("../../../../data/charsets/order.json");

/// The most used languages of `order.json`, in the file's order; empty when
/// the file does not parse (fm-core's tests refuse such a file).
pub(super) fn most_used() -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(ORDER_JSON)
        .ok()
        .and_then(|v| {
            v["mostUsed"].as_array().map(|list| {
                list.iter()
                    .filter_map(|c| c.as_str().map(str::to_owned))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// The presets of the system's language `tag`: the one of the script the tag
/// names, else all of the language, in `presets` order.
fn system_codes(tag: &str, presets: &[LanguagePreset]) -> Vec<String> {
    let mut parts = tag.trim().split(['-', '_']);
    let Some(lang) = parts.next().map(str::to_ascii_lowercase) else {
        return Vec::new();
    };
    if !(2..=3).contains(&lang.len()) || !lang.bytes().all(|b| b.is_ascii_lowercase()) {
        return Vec::new();
    }
    let script = parts
        .find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(|p| {
            let lower = p.to_ascii_lowercase();
            format!("{}{}", lower[..1].to_ascii_uppercase(), &lower[1..])
        });
    let of_language: Vec<&LanguagePreset> = presets
        .iter()
        .filter(|p| p.code.split('-').next() == Some(lang.as_str()))
        .collect();
    let of_script: Vec<&LanguagePreset> = match &script {
        Some(s) => of_language
            .iter()
            .copied()
            .filter(|p| p.code.split('-').nth(1) == Some(s.as_str()) || p.script == *s)
            .collect(),
        None => Vec::new(),
    };
    let chosen = if of_script.is_empty() {
        of_language
    } else {
        of_script
    };
    chosen.into_iter().map(|p| p.code.clone()).collect()
}

/// `presets` in the one order, each with its group: `my_languages` (else the
/// system's language), then `most_used`, then the rest A to Z by name.
pub(super) fn arrange(
    presets: Vec<LanguagePreset>,
    most_used: &[String],
    my_languages: &[String],
    system: Option<&str>,
) -> Vec<LanguagePreset> {
    let lead = if my_languages.is_empty() {
        system
            .map(|t| system_codes(t, &presets))
            .unwrap_or_default()
    } else {
        my_languages.to_vec()
    };
    let mut rest: Vec<Option<LanguagePreset>> = presets.into_iter().map(Some).collect();
    let mut out = Vec::new();
    let groups = [
        (lead.as_slice(), LanguageGroup::Mine),
        (most_used, LanguageGroup::MostUsed),
    ];
    for (codes, group) in groups {
        for code in codes {
            if let Some(preset) = rest
                .iter_mut()
                .find(|p| p.as_ref().is_some_and(|p| p.code == *code))
                .and_then(Option::take)
            {
                out.push(LanguagePreset {
                    group: Some(group),
                    ..preset
                });
            }
        }
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
    use serde_json::Value;

    use super::*;

    #[test]
    fn the_order_file_is_read() {
        let list = most_used();
        assert_eq!(list.first().map(String::as_str), Some("en"));
        assert!(list.len() >= 3);
    }

    #[test]
    fn the_shared_cases_hold() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/language-order/cases.json"),
        )
        .expect("cases");
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
            .map(|p| LanguagePreset {
                code: p["code"].as_str().expect("code").into(),
                name: p["name"].as_str().expect("name").into(),
                script: p["script"].as_str().expect("script").into(),
                pangram: None,
                sample_glyphs: None,
                group: None,
            })
            .collect();
        let most_used = strings(&cases["mostUsed"]);
        let list = cases["cases"].as_array().expect("cases");
        assert!(!list.is_empty());
        for case in list {
            let got: Vec<Value> = arrange(
                presets.clone(),
                &most_used,
                &strings(&case["myLanguages"]),
                case["system"].as_str(),
            )
            .iter()
            .map(|p| serde_json::json!([p.code, p.group]))
            .collect();
            assert_eq!(Value::Array(got), case["expected"], "{}", case["name"]);
        }
    }
}
