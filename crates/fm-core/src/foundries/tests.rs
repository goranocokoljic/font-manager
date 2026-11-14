use std::path::Path;

use serde_json::Value;

use super::*;

fn names<'a>(
    manufacturer: Option<&'a str>,
    trademark: Option<&'a str>,
    copyright: Option<&'a str>,
    designer: Option<&'a str>,
) -> FaceNames<'a> {
    FaceNames {
        manufacturer,
        trademark,
        copyright,
        designer,
    }
}

/// The raw name a face's names state.
fn raw(n: FaceNames<'_>) -> Option<String> {
    identify(&n).map(|f| f.raw)
}

fn from_copyright(text: &str) -> Option<String> {
    raw(names(None, None, Some(text), None))
}

/// The canonical names of one face per raw manufacturer string.
fn listed(raws: &[&str], merges: &[Merge]) -> (Table, Vec<Option<u32>>) {
    let found: Vec<Option<Identified>> = raws
        .iter()
        .map(|r| identify(&names(Some(r), None, None, None)))
        .collect();
    Table::build(&found, merges)
}

fn name_of(table: &Table, slot: Option<u32>) -> Option<&str> {
    slot.map(|s| table.slots[s as usize].name.as_str())
}

#[test]
fn the_embedded_file_is_the_file_on_disk() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/foundries.json");
    let disk = std::fs::read_to_string(path).expect("readable");
    assert_eq!(disk.replace("\r\n", "\n"), FILE.1.replace("\r\n", "\n"));
}

#[test]
fn the_embedded_file_loads() {
    let catalog = parse(FILE.0, FILE.1).expect("embedded data loads");
    assert!(catalog.canonical.len() > 20, "{}", catalog.canonical.len());
    assert!(!catalog.patterns.is_empty());
}

#[test]
fn the_schema_and_the_loader_agree() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/foundries.schema.json");
    let text = std::fs::read_to_string(path).expect("schema");
    let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
    let sorted = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v
            .as_array()
            .expect("array")
            .iter()
            .map(|s| s.as_str().expect("string").to_owned())
            .collect();
        k.sort();
        k
    };
    assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        sorted(&schema["required"]),
        [
            "$schema",
            "foundries",
            "ignore",
            "nameStops",
            "noticeStops",
            "patterns",
            "suffixes"
        ]
    );
    let mut properties: Vec<String> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .cloned()
        .collect();
    properties.sort();
    assert_eq!(properties, sorted(&schema["required"]));
    let entry = &schema["$defs"]["foundry"];
    assert_eq!(sorted(&entry["required"]), ["aliases", "name"]);
    assert_eq!(entry["additionalProperties"], false);
    let file: Value = serde_json::from_str(FILE.1).expect("JSON");
    assert_eq!(file["$schema"], SCHEMA_REF);
    // Every pattern of the file has the shape the schema asks for, as the
    // loader reads it: one {name}, at the end.
    for p in file["patterns"].as_array().expect("patterns") {
        let p = p.as_str().expect("string");
        assert!(tokens(p).is_ok(), "{p}");
        assert!(p.ends_with("{name}"), "{p}");
    }
}

#[test]
fn broken_data_is_rejected() {
    let file = |suffixes: &str, ignore: &str, patterns: &str, foundries: &str| {
        format!(
            r#"{{"$schema": "./foundries.schema.json", "suffixes": [{suffixes}],
                "ignore": [{ignore}], "patterns": [{patterns}], "nameStops": [" ("],
                "noticeStops": [","], "foundries": [{foundries}]}}"#
        )
    };
    let ok_p = r#""copyright {years} {name}""#;
    let ok_f = r#"{"name": "Google", "aliases": ["Google Fonts"]}"#;
    let cases = [
        (
            file(r#""llc", "inc""#, "", ok_p, ok_f),
            "suffixes: \"inc\" is out of order",
        ),
        (
            file(r#""L.L.C.""#, "", ok_p, ok_f),
            "is not one lower-case word",
        ),
        (file("", r#""Bad Key""#, ok_p, ok_f), "is not a key"),
        (file("", "", "", ok_f), "patterns is empty"),
        (
            file("", "", r#""copyright {name} {years}""#, ok_f),
            "it must end in its one {name}",
        ),
        (
            file("", "", r#""copyright {year} {name}""#, ok_f),
            "{year} is not a placeholder",
        ),
        (
            file("", "", r#""copyright {name""#, ok_f),
            "a { is not closed",
        ),
        (file("", "", ok_p, ""), "foundries is empty"),
        (
            file(
                "",
                "",
                ok_p,
                r#"{"name": "Zeta", "aliases": []}, {"name": "Alpha", "aliases": []}"#,
            ),
            "Alpha: name is out of order",
        ),
        (
            file(
                "",
                "",
                ok_p,
                r#"{"name": "Alpha", "aliases": ["Shared"]}, {"name": "Beta", "aliases": ["SHARED"]}"#,
            ),
            "belongs to Alpha and Beta",
        ),
        (
            file("", "", ok_p, r#"{"name": " Google", "aliases": []}"#),
            "has white space at an end",
        ),
        (
            file("", "", ok_p, r#"{"name": "Google", "aliases": ["--"]}"#),
            "has no letters or digits",
        ),
        (
            file("", "", ok_p, r#"{"name": "Google", "aliases": [], "x": 1}"#),
            "unknown field",
        ),
    ];
    for (json, why) in cases {
        let err = parse("t.json", &json).expect_err(&json);
        let detail = err.detail().unwrap_or_default();
        assert!(detail.contains(why), "{json}: {detail}");
        assert!(detail.starts_with("data/t.json"), "{detail}");
    }
    assert!(parse("t.json", "{").is_err());
    let other = file("", "", ok_p, ok_f).replace("./foundries.schema.json", "other.json");
    assert!(parse("t.json", &other).is_err());
    assert!(parse("t.json", &file("", "", ok_p, ok_f)).is_ok());
}

#[test]
fn keys_ignore_case_punctuation_and_legal_suffixes() {
    assert_eq!(key("Google LLC"), "google");
    assert_eq!(key("GOOGLE, Inc."), "google");
    assert_eq!(key("Omnibus-Type"), "omnibus type");
    assert_eq!(key("omnibus   type"), "omnibus type");
    assert_eq!(key("Cadson Demak Co.,Ltd."), "cadson demak");
    assert_eq!(key("Jörg Drees"), "jörg drees");
    // A suffix alone is still a name.
    assert_eq!(key("Inc."), "inc");
    // A suffix inside a word or in the middle stays.
    assert_eq!(key("Fontco"), "fontco");
    assert_eq!(key("Co Type"), "co type");
    assert_eq!(key("--"), "");
}

#[test]
fn the_story_s_google_names_fold_into_google() {
    for raw in [
        "Google",
        "Google Fonts",
        "The Google Font Team",
        "google fonts",
        "Google LLC",
        "GOOGLE, Inc.",
    ] {
        assert_eq!(group_key(raw), "google", "{raw}");
    }
    let (table, slots) = listed(
        &[
            "Google Fonts",
            "The Google Font Team",
            "Google",
            "Monotype Imaging Inc.",
        ],
        &[],
    );
    let named: Vec<Option<&str>> = slots.iter().map(|s| name_of(&table, *s)).collect();
    assert_eq!(
        named,
        [
            Some("Google"),
            Some("Google"),
            Some("Google"),
            Some("Monotype")
        ]
    );
    let google = &table.slots[slots[0].expect("slot") as usize];
    assert_eq!(
        google.raws.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Google", "Google Fonts", "The Google Font Team"]
    );
    // A name the table does not know stays its own foundry.
    assert_ne!(group_key("Google Type"), "google");
}

#[test]
fn a_name_the_table_does_not_know_lists_under_its_most_used_spelling() {
    let (table, slots) = listed(&["vernon adams", "Vernon Adams", "Vernon Adams"], &[]);
    assert_eq!(table.slots.len(), 1);
    assert!(slots.iter().all(|s| *s == Some(0)));
    assert_eq!(table.slots[0].name, "Vernon Adams");
    // A tie goes to the first spelling A to Z.
    let (table, _) = listed(&["vernon adams", "Vernon Adams"], &[]);
    assert_eq!(table.slots[0].name, "Vernon Adams");
}

#[test]
fn copyright_notices_give_the_holder() {
    let cases = [
        (
            "Copyright (c) 2011, Typomondo, with Reserved Font Name \"Nosifer\"",
            Some("Typomondo"),
        ),
        (
            "Copyright (c) 2005-2011, Carrois Type Design (carrois.com), with Reserved Font Name \"Share\"",
            Some("Carrois Type Design"),
        ),
        (
            "Copyright (c) 2011, Dan Sayers (i@iotic.com), with Reserved Font Name 'Averia'.",
            Some("Dan Sayers"),
        ),
        (
            "© 2019 Colophon Foundry. All rights reserved.",
            Some("Colophon Foundry"),
        ),
        ("Copyright 2012 by Typomondo", Some("Typomondo")),
        (
            "COPYRIGHT 2011, 2012 Some Foundry Ltd.",
            Some("Some Foundry Ltd"),
        ),
        ("(c)2020 Kanon Foundry", Some("Kanon Foundry")),
        // A project's authors are no foundry.
        (
            "Copyright 2015 The Public Sans Project Authors (https://github.com/uswds/public-sans)",
            None,
        ),
        ("Copyright 2015 The Authors", None),
        // No pattern matches: no year, or no notice at all.
        ("Copyright Typomondo", None),
        ("Made with love", None),
        ("", None),
    ];
    for (text, want) in cases {
        assert_eq!(from_copyright(text).as_deref(), want, "{text}");
    }
}

#[test]
fn trademark_notices_give_the_owner() {
    let tm = |text| raw(names(None, Some(text), None, None));
    assert_eq!(
        tm("Roboto is a trademark of Google.").as_deref(),
        Some("Google")
    );
    assert_eq!(
        tm("Source is a registered trademark of Adobe Systems Incorporated in the United States")
            .as_deref(),
        Some("Adobe Systems Incorporated in the United States")
    );
    assert_eq!(tm("Roboto").as_deref(), None);
    assert_eq!(
        identify(&names(
            None,
            Some("Roboto is a trademark of Google."),
            None,
            None
        ))
        .map(|f| f.key)
        .as_deref(),
        Some("google")
    );
}

#[test]
fn the_names_are_read_in_order_and_an_ignored_one_falls_through() {
    let copyright = Some("Copyright (c) 2011, Typomondo");
    let trademark = Some("Nosifer is a trademark of Typomondo Ltd.");
    // The manufacturer wins.
    assert_eq!(
        raw(names(
            Some("Omnibus-Type"),
            trademark,
            copyright,
            Some("A Designer")
        ))
        .as_deref(),
        Some("Omnibus-Type")
    );
    // Then the trademark, then the copyright, then the designer.
    assert_eq!(
        raw(names(None, trademark, copyright, Some("A Designer"))).as_deref(),
        Some("Typomondo Ltd")
    );
    assert_eq!(
        raw(names(None, None, copyright, Some("A Designer"))).as_deref(),
        Some("Typomondo")
    );
    assert_eq!(
        raw(names(
            None,
            None,
            Some("Copyright 2018 The Roboto Slab Project Authors (https://github.com/x)"),
            Some("Google")
        ))
        .as_deref(),
        Some("Google")
    );
    // An ignored manufacturer gives way to the next string.
    assert_eq!(
        raw(names(Some("Unknown"), None, copyright, None)).as_deref(),
        Some("Typomondo")
    );
    // A name is cut at its first stop.
    assert_eq!(
        raw(names(Some("Cyreal (www.cyreal.org)"), None, None, None)).as_deref(),
        Some("Cyreal")
    );
    assert_eq!(
        raw(names(
            Some("Pablo Impallari. http://www.impallari.com Igino Marini."),
            None,
            None,
            None
        ))
        .as_deref(),
        Some("Pablo Impallari")
    );
    // A manufacturer keeps its commas; only notices end at one.
    assert_eq!(
        raw(names(
            Some("Applied Design Works, Letters from Sweden"),
            None,
            None,
            None
        ))
        .as_deref(),
        Some("Applied Design Works, Letters from Sweden")
    );
    assert_eq!(raw(names(None, None, None, None)), None);
    assert_eq!(raw(names(Some("  "), None, Some("no notice"), None)), None);
}

#[test]
fn merges_fold_one_foundry_into_another_and_chain() {
    let merges = [Merge {
        name: "Omnibus-Type".into(),
        into: "Google".into(),
    }];
    let (table, slots) = listed(&["Omnibus Type", "Google LLC", "Tribby Type"], &merges);
    let named: Vec<Option<&str>> = slots.iter().map(|s| name_of(&table, *s)).collect();
    assert_eq!(named, [Some("Google"), Some("Google"), Some("Tribby Type")]);
    let google = &table.slots[slots[1].expect("slot") as usize];
    assert_eq!(google.merged.iter().collect::<Vec<_>>(), ["Omnibus-Type"]);
    assert!(google.raws.contains("Omnibus Type"));
    // The merged name still finds the foundry it became.
    assert_eq!(table.find("omnibus-type"), slots[1]);
    assert_eq!(table.find("Nobody"), None);

    // A chain: Tribby Type → Omnibus-Type → Google.
    let chained = [
        merges[0].clone(),
        Merge {
            name: "Tribby Type".into(),
            into: "Omnibus-Type".into(),
        },
    ];
    let (table, slots) = listed(&["Omnibus Type", "Google LLC", "Tribby Type"], &chained);
    assert!(slots.iter().all(|s| name_of(&table, *s) == Some("Google")));
    assert_eq!(table.slots.len(), 1);

    // A cycle (a damaged index) is not followed.
    let cycle = [
        Merge {
            name: "Google".into(),
            into: "Tribby Type".into(),
        },
        Merge {
            name: "Tribby Type".into(),
            into: "Google".into(),
        },
    ];
    let (table, slots) = listed(&["Google", "Tribby Type"], &cycle);
    let named: Vec<Option<&str>> = slots.iter().map(|s| name_of(&table, *s)).collect();
    assert_eq!(named, [Some("Google"), Some("Tribby Type")]);
}

#[test]
fn faces_without_a_foundry_have_no_slot() {
    let found = [None, identify(&names(Some("Google"), None, None, None))];
    let (table, slots) = Table::build(&found, &[]);
    assert_eq!(slots, [None, Some(0)]);
    assert_eq!(table.slots.len(), 1);
}

#[test]
fn the_slots_are_a_to_z_without_case() {
    let (table, _) = listed(&["fragTYPE", "Bold Monday", "a.Foundry", "Zeta"], &[]);
    let order: Vec<&str> = table.slots.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(order, ["a.Foundry", "Bold Monday", "fragTYPE", "Zeta"]);
}
