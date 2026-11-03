use std::path::Path;

use fm_api::library::LicensePermissionsSource as Source;
use serde_json::Value;

use super::*;

const EMBEDDINGS: [Embedding; 4] = [
    Embedding::Installable,
    Embedding::Restricted,
    Embedding::PreviewAndPrint,
    Embedding::Editable,
];

const OPEN: [Option<OpenLicense>; 5] = [
    None,
    Some(OpenLicense::Ofl),
    Some(OpenLicense::Apache),
    Some(OpenLicense::Ufl),
    Some(OpenLicense::Mit),
];

const DECLARED: [Option<LicenseType>; 7] = [
    None,
    Some(LicenseType::Desktop),
    Some(LicenseType::Web),
    Some(LicenseType::App),
    Some(LicenseType::Trial),
    Some(LicenseType::PersonalOnly),
    Some(LicenseType::Unknown),
];

fn rules() -> Rules {
    parse(FILE.0, FILE.1).expect("the data file loads")
}

/// `rule` as the API gives it, from `source`.
fn lines(rule: &Rule, source: Source) -> Option<LicensePermissions> {
    Some(LicensePermissions {
        may: rule.may.clone(),
        may_not: rule.may_not.clone(),
        source,
    })
}

#[test]
fn the_data_file_loads_and_every_rule_is_its_own() {
    let rules = rules();
    assert!(RULES.is_some());
    let named = rules.named();
    for (i, (name, rule)) in named.iter().enumerate() {
        for (other, other_rule) in &named[i + 1..] {
            assert_ne!(rule, other_rule, "{name} and {other} have the same lines");
        }
    }
}

#[test]
fn each_open_license_gives_its_own_lines() {
    let rules = rules();
    for (license, rule) in [
        (OpenLicense::Ofl, &rules.open_licenses.ofl),
        (OpenLicense::Apache, &rules.open_licenses.apache),
        (OpenLicense::Ufl, &rules.open_licenses.ufl),
        (OpenLicense::Mit, &rules.open_licenses.mit),
    ] {
        assert!(!rule.may.is_empty(), "{license:?}");
        assert_eq!(
            from_rules(&rules, Embedding::Installable, Some(license), None),
            lines(rule, Source::OpenLicense),
            "{license:?}"
        );
    }
}

#[test]
fn the_open_license_lines_say_what_each_license_says() {
    let rules = rules();
    let may_not = |rule: &Rule, line: &str| rule.may_not.iter().any(|l| l == line);
    let sell = "Sell the font files on their own";
    assert!(may_not(&rules.open_licenses.ofl, sell));
    assert!(may_not(&rules.open_licenses.ufl, sell));
    // Apache and MIT allow it, so they must not forbid it.
    assert!(!may_not(&rules.open_licenses.apache, sell));
    assert!(!may_not(&rules.open_licenses.mit, sell));
}

#[test]
fn a_declared_desktop_license_gives_its_lines_also_over_an_open_license() {
    let rules = rules();
    for open in OPEN {
        for embedding in EMBEDDINGS {
            assert_eq!(
                from_rules(&rules, embedding, open, Some(LicenseType::Desktop)),
                lines(&rules.declared.desktop, Source::Declared),
                "{open:?} {embedding:?}"
            );
        }
    }
}

#[test]
fn each_declared_type_gives_its_own_lines_on_a_file_that_says_nothing() {
    let rules = rules();
    for (declared, rule) in [
        (LicenseType::Desktop, &rules.declared.desktop),
        (LicenseType::Web, &rules.declared.web),
        (LicenseType::App, &rules.declared.app),
        (LicenseType::Trial, &rules.declared.trial),
        (LicenseType::PersonalOnly, &rules.declared.personal_only),
    ] {
        assert_eq!(
            from_rules(&rules, Embedding::Installable, None, Some(declared)),
            lines(rule, Source::Declared),
            "{declared:?}"
        );
    }
}

#[test]
fn restricted_embedding_gives_its_lines_also_over_an_open_license() {
    let rules = rules();
    for (embedding, rule) in [
        (Embedding::Restricted, &rules.embedding.restricted),
        (
            Embedding::PreviewAndPrint,
            &rules.embedding.preview_and_print,
        ),
    ] {
        for open in OPEN {
            for declared in [None, Some(LicenseType::Unknown)] {
                assert_eq!(
                    from_rules(&rules, embedding, open, declared),
                    lines(rule, Source::EmbeddingBits),
                    "{embedding:?} {open:?} {declared:?}"
                );
            }
        }
    }
    // A restricted file allows nothing, and the summary does not pretend it does.
    assert!(rules.embedding.restricted.may.is_empty());
    assert!(!rules.embedding.restricted.may_not.is_empty());
}

#[test]
fn editable_embedding_gives_its_lines_only_when_no_license_says_more() {
    let rules = rules();
    let bits = lines(&rules.embedding.editable, Source::EmbeddingBits);
    assert_eq!(from_rules(&rules, Embedding::Editable, None, None), bits);
    assert_eq!(
        from_rules(
            &rules,
            Embedding::Editable,
            None,
            Some(LicenseType::Unknown)
        ),
        bits
    );
    assert_eq!(
        from_rules(&rules, Embedding::Editable, Some(OpenLicense::Ofl), None),
        lines(&rules.open_licenses.ofl, Source::OpenLicense)
    );
}

#[test]
fn a_declared_app_license_yields_to_an_open_license_like_web_use_does() {
    let rules = rules();
    let app = Some(LicenseType::App);
    let declared = lines(&rules.declared.app, Source::Declared);
    assert_eq!(
        from_rules(&rules, Embedding::Installable, None, app),
        declared
    );
    assert_eq!(
        from_rules(&rules, Embedding::Restricted, Some(OpenLicense::Ofl), app),
        declared
    );
    assert_eq!(
        from_rules(&rules, Embedding::Installable, Some(OpenLicense::Ofl), app),
        lines(&rules.open_licenses.ofl, Source::OpenLicense)
    );
}

#[test]
fn nothing_known_gives_nothing() {
    let rules = rules();
    // Exactly the file with no statement, with and without an empty declaration.
    for declared in [None, Some(LicenseType::Unknown)] {
        assert_eq!(
            from_rules(&rules, Embedding::Installable, None, declared),
            None,
            "{declared:?}"
        );
    }
    // Every other input says something.
    for embedding in EMBEDDINGS {
        for open in OPEN {
            for declared in DECLARED {
                let silent = embedding == Embedding::Installable
                    && open.is_none()
                    && matches!(declared, None | Some(LicenseType::Unknown));
                assert_eq!(
                    from_rules(&rules, embedding, open, declared).is_none(),
                    silent,
                    "{embedding:?} {open:?} {declared:?}"
                );
            }
        }
    }
}

/// The lesson of E27.4: the summary next to the web answer must not disagree
/// with it. An editor who rewords a line keeps the word "websites" in it.
#[test]
fn the_lines_agree_with_web_use_on_every_combination() {
    let rules = rules();
    for embedding in EMBEDDINGS {
        for open in OPEN {
            for declared in DECLARED {
                let Some(p) = from_rules(&rules, embedding, open, declared) else {
                    continue;
                };
                let names_web = |l: &String| l.to_lowercase().contains("websites");
                let ok = web_use(embedding, open, declared).ok();
                let input = format!("{embedding:?} {open:?} {declared:?}: {p:?}");
                assert_eq!(p.may.iter().any(names_web), ok, "{input}");
                assert_eq!(p.may_not.iter().any(names_web), !ok, "{input}");
            }
        }
    }
}

#[test]
fn the_schema_and_the_loader_agree() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/licenses/permissions.schema.json");
    let text = std::fs::read_to_string(path).expect("schema");
    let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
    assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
    assert_eq!(schema["additionalProperties"], false);

    let keys = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v
            .as_array()
            .expect("array")
            .iter()
            .map(|s| s.as_str().expect("string").to_owned())
            .collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&schema["required"]),
        ["$schema", "declared", "embedding", "openLicenses"]
    );
    let rule = &schema["$defs"]["rule"];
    assert_eq!(keys(&rule["required"]), ["may", "mayNot"]);
    assert_eq!(rule["additionalProperties"], false);

    // The rule names in the schema are the ones the loader reads, and the keys
    // that have no rule on purpose are the API's silent values.
    let loaded: Value = serde_json::from_str(FILE.1).expect("JSON");
    assert_eq!(loaded["$schema"], SCHEMA_REF);
    for (name, _) in rules().named() {
        let (group_name, key) = name.split_once('.').expect("group.key");
        assert!(loaded[group_name][key].is_object(), "{name}");
        let group = &schema["properties"][group_name];
        assert_eq!(group["additionalProperties"], false, "{name}");
        assert!(
            group["required"]
                .as_array()
                .expect("required")
                .iter()
                .any(|k| k == key),
            "{name}"
        );
    }
    let count = |group: &str| {
        schema["properties"][group]["required"]
            .as_array()
            .expect("required")
            .len()
    };
    assert_eq!(
        count("openLicenses") + count("declared") + count("embedding"),
        rules().named().len()
    );
    // The keys follow the API's own spelling of its enums.
    for (key, value) in [
        (
            "personalOnly",
            serde_json::to_value(LicenseType::PersonalOnly),
        ),
        (
            "previewAndPrint",
            serde_json::to_value(Embedding::PreviewAndPrint),
        ),
    ] {
        assert_eq!(value.expect("json"), key);
    }
}

#[test]
fn the_source_is_spelled_as_the_story_asks() {
    for (source, text) in [
        (Source::OpenLicense, "open-license"),
        (Source::Declared, "declared"),
        (Source::EmbeddingBits, "embedding-bits"),
    ] {
        assert_eq!(serde_json::to_value(source).expect("json"), text);
    }
}

#[test]
fn broken_data_is_rejected() {
    let good = r#"{"may": ["A"], "mayNot": []}"#;
    let file = |ofl: &str| -> String {
        let open = format!(
            r#""openLicenses": {{"ofl": {ofl}, "apache": {good}, "ufl": {good}, "mit": {good}}}"#
        );
        let declared = format!(
            r#""declared": {{"desktop": {good}, "web": {good}, "app": {good}, "trial": {good}, "personalOnly": {good}}}"#
        );
        let embedding = format!(
            r#""embedding": {{"restricted": {good}, "previewAndPrint": {good}, "editable": {good}}}"#
        );
        format!(r#"{{"$schema": "./permissions.schema.json", {open}, {declared}, {embedding}}}"#)
    };
    assert!(parse("t.json", &file(good)).is_ok());
    for (ofl, why) in [
        (
            r#"{"may": [], "mayNot": []}"#,
            "openLicenses.ofl: the rule has no lines",
        ),
        (r#"{"may": [" A"], "mayNot": []}"#, "white space at an end"),
        (r#"{"may": [""], "mayNot": []}"#, "is empty"),
        (r#"{"may": [], "mayNot": ["A", "A"]}"#, "is listed twice"),
        (r#"{"may": ["A"]}"#, "missing field"),
        (r#"{"may": ["A"], "mayNot": [], "x": 1}"#, "unknown field"),
    ] {
        let err = parse("t.json", &file(ofl)).expect_err(ofl);
        let detail = err.detail().unwrap_or_default().to_owned();
        assert!(detail.contains(why), "{ofl}: {detail}");
        assert!(detail.starts_with("data/licenses/t.json"), "{detail}");
    }
    let wrong_ref = file(good).replace("./permissions.schema.json", "other.json");
    assert!(parse("t.json", &wrong_ref).is_err());
    assert!(parse("t.json", "{").is_err());
    let no_group = file(good).replace("\"declared\"", "\"declaredTypes\"");
    assert!(parse("t.json", &no_group).is_err());
}
