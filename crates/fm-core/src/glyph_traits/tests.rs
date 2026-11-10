use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn catalog() -> &'static [GlyphTrait] {
    load().expect("the catalog is valid")
}

/// A valid catalog with one enum and one scalar trait.
fn valid() -> Value {
    json!({
        "$schema": "./glyph-traits.schema.json",
        "traits": [
            {
                "id": "R.leg", "script": "Latn", "glyph": "R", "codepoint": "U+0052",
                "kind": "enum",
                "name": { "en": "R leg", "de": "Bein des R" },
                "description": "The leg.",
                "values": [
                    { "id": "straight", "label": { "en": "Straight", "de": "Gerade" },
                      "description": "Straight.", "example": "M18 46V10L40 46" },
                    { "id": "curved", "label": { "en": "Curved", "de": "Geschwungen" },
                      "description": "Curved.", "example": "M18 46V10Q30 30 40 46" }
                ]
            },
            {
                "id": "O.roundness", "script": "Latn", "glyph": "O", "codepoint": "U+004F",
                "kind": "scalar",
                "name": { "en": "O shape", "de": "Form des O" },
                "description": "Roundness.",
                "measures": [
                    { "id": "roundness", "description": "W/h.", "min": 0, "max": 4 }
                ],
                "values": [
                    { "id": "round", "label": { "en": "Round", "de": "Rund" },
                      "description": "Round.", "when": [{ "measure": "roundness", "min": 0.95 }],
                      "example": "M28 10A18 18 0 1 1 28 46A18 18 0 1 1 28 10Z" },
                    { "id": "narrow", "label": { "en": "Narrow", "de": "Schmal" },
                      "description": "Narrow.", "when": [],
                      "example": "M28 10A9 18 0 1 1 28 46A9 18 0 1 1 28 10Z" }
                ]
            }
        ]
    })
}

fn refused(file: &Value) -> String {
    parse(&file.to_string()).expect_err("refused").to_string()
}

#[test]
fn the_first_cut_lists_the_thirteen_latin_traits_with_their_values() {
    let found: Vec<(&str, Vec<&str>)> = catalog()
        .iter()
        .map(|t| {
            (
                t.id.as_str(),
                t.values.iter().map(|v| v.id.as_str()).collect(),
            )
        })
        .collect();
    let expected: Vec<(&str, Vec<&str>)> = vec![
        ("R.leg", vec!["straight", "curved", "both"]),
        ("O.roundness", vec!["squared", "round", "oval", "narrow"]),
        ("g.storeys", vec!["single", "double"]),
        ("a.storeys", vec!["single", "double"]),
        ("G.spur", vec!["spur", "none"]),
        ("Q.tail", vec!["inside", "crossing", "swash"]),
        ("t.terminal", vec!["flat", "angled"]),
        ("one.flag", vec!["flag", "none"]),
        ("seven.bar", vec!["bar", "none"]),
        ("I.form", vec!["plain", "serifed", "barred"]),
        ("l.form", vec!["plain", "tailed"]),
        ("M.sides", vec!["vertical", "splayed"]),
        ("e.bar", vec!["horizontal", "angled"]),
    ];
    assert_eq!(found, expected);
    for t in catalog() {
        let kind = if t.id == "O.roundness" {
            TraitKind::Scalar
        } else {
            TraitKind::Enum
        };
        assert_eq!(t.kind, kind, "{}", t.id);
        assert_eq!(t.script, "Latn", "{}: Latin only in this cut", t.id);
        for v in &t.values {
            assert!(v.label.contains_key("en") && v.label.contains_key("de"));
        }
    }
    let one = find("one.flag").expect("valid").expect("listed");
    assert_eq!(one.glyph, '1');
    assert_eq!(find("R.leg").expect("valid").expect("listed").glyph, 'R');
    assert!(find("r.leg").expect("valid").is_none());
}

#[test]
fn o_roundness_maps_its_measures_to_chips_at_the_thresholds() {
    let o = find("O.roundness").expect("valid").expect("listed");
    let chip = |roundness: f64, deviation: f64| {
        o.chip(&[("roundness", roundness), ("deviation", deviation)])
            .map(|v| v.id.as_str())
    };
    // O.roundness ≥ 0.95 → round.
    assert_eq!(chip(0.95, 0.0), Some("round"));
    assert_eq!(chip(1.30, 0.01), Some("round"));
    assert_eq!(chip(0.9499, 0.0), Some("oval"));
    assert_eq!(chip(0.75, 0.0), Some("oval"));
    assert_eq!(chip(0.7499, 0.0), Some("narrow"));
    // A boxy outline is squared whatever its proportions.
    assert_eq!(chip(1.0, 0.04), Some("squared"));
    assert_eq!(chip(0.5, 0.2), Some("squared"));
    assert_eq!(chip(1.0, 0.0399), Some("round"));
    // A threshold on a measure that is not given does not hold; the
    // catch-all still answers.
    assert_eq!(
        o.chip(&[("roundness", 1.0)]).map(|v| v.id.as_str()),
        Some("round")
    );
    assert_eq!(o.chip(&[]).map(|v| v.id.as_str()), Some("narrow"));
    // An enum trait has no chips.
    let r = find("R.leg").expect("valid").expect("listed");
    assert!(r.chip(&[("roundness", 1.0)]).is_none());
}

#[test]
fn every_example_is_a_stroke_path_inside_the_box_and_differs_within_its_trait() {
    for t in catalog() {
        for v in &t.values {
            assert!(check_example(&v.example).is_ok(), "{} {}", t.id, v.id);
        }
    }
    assert!(check_example("M0 0L56 56Z").is_ok());
    assert!(check_example("M10 10A8 8 0 1 1 20 20").is_ok());
    let bad = [
        ("L10 10", "start with M"),
        ("M10 10l5 5", "absolute"),
        ("M10 10L57 10", "outside"),
        ("M10 10L-1 10", "outside"),
        ("M10 10L5", "L has 1"),
        ("M10 10C1 2 3 4 5", "C has 5"),
        ("M10 10Z5", "Z has 1"),
        ("M10 10A8 8 0 2 1 20 20", "arc flag"),
        ("M10 10L1..2 3", "not a number"),
    ];
    for (path, why) in bad {
        let error = check_example(path).expect_err(path);
        assert!(error.contains(why), "{path}: {error}");
    }
    assert!(check_example(&format!("M1 1{}", "L2 2".repeat(120))).is_err());
}

#[test]
fn a_cyrillic_trait_fits_the_same_shape() {
    let mut file = valid();
    file["traits"][0]["id"] = json!("Decy.descenders");
    file["traits"][0]["script"] = json!("Cyrl");
    file["traits"][0]["glyph"] = json!("Д");
    file["traits"][0]["codepoint"] = json!("U+0414");
    let traits = parse(&file.to_string()).expect("a Cyrillic trait parses");
    assert_eq!(traits[0].script, "Cyrl");
    assert_eq!(traits[0].glyph, 'Д');
}

#[test]
fn a_broken_catalog_is_refused_with_the_reason() {
    type Edit = fn(&mut Value);
    let cases: [(&str, Edit); 26] = [
        ("$schema", |f| f["$schema"] = json!("./other.json")),
        ("traits is empty", |f| f["traits"] = json!([])),
        ("listed twice", |f| {
            let first = f["traits"][0].clone();
            f["traits"].as_array_mut().expect("traits").push(first);
        }),
        ("exactly one character", |f| {
            f["traits"][0]["glyph"] = json!("RR")
        }),
        ("is not the glyph", |f| {
            f["traits"][0]["codepoint"] = json!("U+0053");
        }),
        ("must start with R", |f| {
            f["traits"][0]["id"] = json!("S.leg")
        }),
        ("must start with one", |f| {
            f["traits"][0]["glyph"] = json!("1");
            f["traits"][0]["codepoint"] = json!("U+0031");
            f["traits"][0]["id"] = json!("seven.flag");
        }),
        ("<glyph name>.<trait>", |f| {
            f["traits"][0]["id"] = json!("Rleg")
        }),
        ("ISO 15924", |f| f["traits"][0]["script"] = json!("latn")),
        ("no de text", |f| {
            f["traits"][0]["name"] = json!({ "en": "R leg" });
        }),
        ("two-letter", |f| {
            f["traits"][0]["name"]["eng"] = json!("R leg");
        }),
        ("one non-empty line", |f| {
            f["traits"][0]["description"] = json!("Two\nlines.");
        }),
        ("at least two values", |f| {
            f["traits"][0]["values"]
                .as_array_mut()
                .expect("values")
                .pop();
        }),
        ("value straight is listed twice", |f| {
            f["traits"][0]["values"][1]["id"] = json!("straight");
        }),
        ("example of another value", |f| {
            let first = f["traits"][0]["values"][0]["example"].clone();
            f["traits"][0]["values"][1]["example"] = first;
        }),
        ("lowercase letters", |f| {
            f["traits"][0]["values"][0]["id"] = json!("Straight");
        }),
        ("example: ", |f| {
            f["traits"][0]["values"][0]["example"] = json!("M0 0L60 0");
        }),
        ("enum trait has no measures", |f| {
            f["traits"][0]["measures"] = f["traits"][1]["measures"].clone();
        }),
        ("enum value has no thresholds", |f| {
            f["traits"][0]["values"][0]["when"] = json!([]);
        }),
        ("scalar trait needs measures", |f| {
            f["traits"][1]
                .as_object_mut()
                .expect("trait")
                .remove("measures");
        }),
        ("scalar value needs when", |f| {
            f["traits"][1]["values"][1]
                .as_object_mut()
                .expect("value")
                .remove("when");
        }),
        ("only the last value", |f| {
            f["traits"][1]["values"][0]["when"] = json!([]);
        }),
        ("unknown measure", |f| {
            f["traits"][1]["values"][0]["when"][0]["measure"] = json!("width");
        }),
        ("neither min nor max", |f| {
            f["traits"][1]["values"][0]["when"] = json!([{ "measure": "roundness", "min": null }]);
        }),
        ("outside roundness", |f| {
            f["traits"][1]["values"][0]["when"][0]["min"] = json!(5);
        }),
        ("unknown field", |f| f["traits"][0]["extra"] = json!(1)),
    ];
    for (why, edit) in cases {
        let mut file = valid();
        edit(&mut file);
        let error = refused(&file);
        assert!(
            error.contains("data/classification/glyph-traits.json"),
            "{why}: {error}"
        );
        assert!(error.contains(why), "{why}: {error}");
    }
    assert!(parse(&valid().to_string()).is_ok(), "the cases start valid");
}

#[test]
fn the_embedded_catalog_is_the_file_on_disk() {
    let disk = std::fs::read_to_string(repo().join("data/classification/glyph-traits.json"))
        .expect("read");
    assert_eq!(disk.replace("\r\n", "\n"), FILE.replace("\r\n", "\n"));
}

#[test]
fn the_schema_and_the_loader_agree() {
    let text = std::fs::read_to_string(repo().join("data/classification/glyph-traits.schema.json"))
        .expect("schema");
    let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
    let keys = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&schema["properties"]), ["$schema", "traits"]);
    assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
    assert_eq!(schema["additionalProperties"], false);
    let defs = &schema["$defs"];
    assert_eq!(
        keys(&defs["trait"]["properties"]),
        [
            "codepoint",
            "description",
            "glyph",
            "id",
            "kind",
            "measures",
            "name",
            "script",
            "values"
        ]
    );
    assert_eq!(
        keys(&defs["value"]["properties"]),
        ["description", "example", "id", "label", "when"]
    );
    for def in ["trait", "value"] {
        assert_eq!(defs[def]["additionalProperties"], false, "{def}");
    }
    assert_eq!(
        defs["trait"]["properties"]["kind"]["enum"],
        json!(["enum", "scalar"])
    );
    assert_eq!(defs["labels"]["required"], json!(REQUIRED_LANGUAGES));
    assert_eq!(defs["example"]["maxLength"], MAX_EXAMPLE_LEN);
    assert_eq!(
        defs["value"]["properties"]["id"]["pattern"],
        "^[a-z][a-z0-9-]*$"
    );
    assert_eq!(
        defs["trait"]["properties"]["script"]["pattern"],
        "^[A-Z][a-z]{3}$"
    );
}

// ---- The hand-labelled corpus: fixtures/glyph-traits-labels.json ----

/// The least number of labelled families.
const MIN_FAMILIES: usize = 50;

/// The least number of families that show each value.
const MIN_PER_VALUE: usize = 5;

/// The licences the fixture corpus allows.
const LICENCES: [&str; 3] = ["OFL-1.1", "Apache-2.0", "UFL-1.0"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelFile {
    about: String,
    families: Vec<LabelledFamily>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelledFamily {
    family: String,
    file: String,
    traits: BTreeMap<String, String>,
}

fn labels() -> LabelFile {
    let text = std::fs::read_to_string(repo().join("fixtures/glyph-traits-labels.json"))
        .expect("read the labels");
    serde_json::from_str(&text).expect("the labels file is valid")
}

/// Licence by path, from `fixtures/fonts/manifest.tsv`.
fn manifest() -> BTreeMap<String, String> {
    let text =
        std::fs::read_to_string(repo().join("fixtures/fonts/manifest.tsv")).expect("manifest");
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            Some((cols.get(4)?.to_string(), cols.get(2)?.to_string()))
        })
        .collect()
}

/// The problems of a labels file against the catalog: unknown traits or
/// values, missing traits, families or files twice, too few families, a value
/// shown by too few families.
fn coverage_problems(file: &LabelFile, catalog: &[GlyphTrait]) -> Vec<String> {
    let mut problems = Vec::new();
    if file.families.len() < MIN_FAMILIES {
        problems.push(format!(
            "{} families, at least {MIN_FAMILIES} needed",
            file.families.len()
        ));
    }
    let mut count: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (at, f) in file.families.iter().enumerate() {
        let earlier = &file.families[..at];
        if earlier.iter().any(|o| o.family == f.family) {
            problems.push(format!("{} is labelled twice", f.family));
        }
        if earlier.iter().any(|o| o.file == f.file) {
            problems.push(format!("{} is labelled twice", f.file));
        }
        for t in catalog {
            match f.traits.get(&t.id) {
                None => problems.push(format!("{}: {} is not labelled", f.family, t.id)),
                Some(value) => match t.value(value) {
                    None => problems.push(format!("{}: {} has no value {value}", f.family, t.id)),
                    Some(v) => *count.entry((t.id.as_str(), v.id.as_str())).or_default() += 1,
                },
            }
        }
        for id in f.traits.keys() {
            if !catalog.iter().any(|t| &t.id == id) {
                problems.push(format!("{}: unknown trait {id}", f.family));
            }
        }
    }
    for t in catalog {
        for v in &t.values {
            let n = count
                .get(&(t.id.as_str(), v.id.as_str()))
                .copied()
                .unwrap_or(0);
            if n < MIN_PER_VALUE {
                problems.push(format!(
                    "{} {}: {n} families, at least {MIN_PER_VALUE} needed",
                    t.id, v.id
                ));
            }
        }
    }
    problems
}

#[test]
fn the_labels_cover_every_value_of_every_trait_at_least_five_times() {
    let file = labels();
    assert!(!file.about.is_empty());
    assert_eq!(coverage_problems(&file, catalog()), Vec::<String>::new());
}

#[test]
fn the_coverage_check_reports_each_gap() {
    let full = labels();
    let first = &full.families[0];
    let traits = |f: &LabelledFamily| f.traits.clone();

    // One family short of the minimum.
    let short = LabelFile {
        about: String::new(),
        families: full
            .families
            .iter()
            .take(MIN_FAMILIES - 1)
            .map(|f| LabelledFamily {
                family: f.family.clone(),
                file: f.file.clone(),
                traits: traits(f),
            })
            .collect(),
    };
    let problems = coverage_problems(&short, catalog());
    assert!(
        problems.iter().any(|p| p.contains("at least 50 needed")),
        "{problems:?}"
    );

    // A value shown by exactly five families falls short when one of them is
    // relabelled: seven.bar bar is the scarcest value.
    let mut fewer = labels();
    let bar = fewer
        .families
        .iter_mut()
        .find(|f| f.traits["seven.bar"] == "bar")
        .expect("a family with a barred seven");
    bar.traits.insert("seven.bar".into(), "none".into());
    let problems = coverage_problems(&fewer, catalog());
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with("seven.bar bar: 4 families")),
        "{problems:?}"
    );

    // A missing trait, an unknown value, an unknown trait, a family twice.
    let mut broken = labels();
    broken.families[0].traits.remove("R.leg");
    broken.families[1]
        .traits
        .insert("G.spur".into(), "hook".into());
    broken.families[2]
        .traits
        .insert("Z.tail".into(), "long".into());
    broken.families.push(LabelledFamily {
        family: first.family.clone(),
        file: "other.ttf".into(),
        traits: traits(first),
    });
    let problems = coverage_problems(&broken, catalog());
    for expected in [
        "R.leg is not labelled",
        "G.spur has no value hook",
        "unknown trait Z.tail",
        "is labelled twice",
    ] {
        assert!(
            problems.iter().any(|p| p.contains(expected)),
            "{expected}: {problems:?}"
        );
    }
}

#[test]
fn every_labelled_file_is_a_licensed_fixture_of_its_family_with_the_glyphs() {
    let licences = manifest();
    let catalog = catalog();
    for f in labels().families {
        let licence = licences
            .get(&f.file)
            .unwrap_or_else(|| panic!("{} is not in fixtures/fonts/manifest.tsv", f.file));
        assert!(
            LICENCES.contains(&licence.as_str()),
            "{}: {licence}",
            f.file
        );
        let parsed = crate::parser::parse_file(&repo().join("fixtures/fonts").join(&f.file))
            .unwrap_or_else(|e| panic!("{}: {e}", f.file));
        let face = parsed
            .faces
            .first()
            .unwrap_or_else(|| panic!("{}: no face", f.file));
        let family = face.typographic_family.as_ref().or(face.family.as_ref());
        assert_eq!(family, Some(&f.family), "{}", f.file);
        for t in catalog {
            assert!(
                face.code_points.binary_search(&u32::from(t.glyph)).is_ok(),
                "{} has no {}",
                f.file,
                t.glyph
            );
        }
    }
}
