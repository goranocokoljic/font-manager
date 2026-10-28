use std::path::PathBuf;

use serde_json::Value;

use super::*;
use Classification::{Display, Sans, Script, Serif};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pairings")
}

fn face<'a>(
    family: &'a str,
    class: Option<Classification>,
    designer: Option<&'a str>,
) -> FaceFeatures<'a> {
    FaceFeatures {
        facts: PairFacts {
            family,
            designer,
            class,
            x_height: Some(0.5),
        },
        weight: 400,
        width: 5,
        fingerprint: None,
    }
}

fn index_of(name: &str) -> usize {
    FEATURES
        .iter()
        .position(|f| *f == name)
        .unwrap_or_else(|| panic!("no feature {name}"))
}

fn one(entry: &str) -> FmResult<Vec<Curated>> {
    parse_curated(
        "test.json",
        &format!(r#"{{"$schema": "./pairings.schema.json", "pairings": [{entry}]}}"#),
    )
}

#[test]
fn the_data_files_are_the_only_files_and_are_embedded() {
    let mut on_disk: Vec<String> = std::fs::read_dir(data_dir())
        .expect("data/pairings exists")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".json") && !n.ends_with(".schema.json"))
        .collect();
    on_disk.sort();
    assert_eq!(on_disk, [MODEL_FILE.0, PAIRINGS_FILE.0]);
    for (name, embedded) in [PAIRINGS_FILE, MODEL_FILE] {
        let disk = std::fs::read_to_string(data_dir().join(name)).expect("readable");
        assert_eq!(disk.replace("\r\n", "\n"), embedded.replace("\r\n", "\n"));
    }
}

#[test]
fn the_curated_pairings_are_valid() {
    let all = curated().expect("pairings.json loads");
    assert!(all.len() >= 20, "{} pairings", all.len());
    let libre = all
        .iter()
        .find(|c| c.families == ["Libre Baskerville", "Libre Franklin"])
        .expect("Libre Baskerville / Libre Franklin");
    assert!(libre.source.contains("Impallari"));
}

#[test]
fn a_bad_curated_entry_is_refused() {
    let ok = r#"{"families": ["A", "B"], "source": "note"}"#;
    assert_eq!(one(ok).expect("valid").len(), 1);
    for (entry, says) in [
        (
            r#"{"families": ["A", "a"], "source": "note"}"#,
            "one family twice",
        ),
        (
            r#"{"families": ["A", "B"], "source": "note"}, {"families": ["b", "A"], "source": "again"}"#,
            "listed twice",
        ),
        (
            r#"{"families": [" A", "B"], "source": "note"}"#,
            "white space",
        ),
        (r#"{"families": ["A", "B"], "source": ""}"#, "source"),
        (
            r#"{"families": ["A", "B", "C"], "source": "note"}"#,
            "test.json",
        ),
        (
            r#"{"families": ["A", "B"], "source": "note", "x": 1}"#,
            "test.json",
        ),
    ] {
        let err = one(entry).expect_err(entry);
        assert!(format!("{err:?}").contains(says), "{entry}: {err:?}");
    }
    let err =
        parse_curated("t.json", r#"{"$schema": "./x.json", "pairings": []}"#).expect_err("schema");
    assert!(format!("{err:?}").contains("$schema"), "{err:?}");
    let err = parse_curated(
        "t.json",
        r#"{"$schema": "./pairings.schema.json", "pairings": []}"#,
    )
    .expect_err("empty");
    assert!(format!("{err:?}").contains("empty"), "{err:?}");
}

#[test]
fn the_schemas_and_the_loaders_agree() {
    let read = |name: &str| -> Value {
        let text = std::fs::read_to_string(data_dir().join(name)).expect("schema");
        serde_json::from_str(&text).expect("schema is JSON")
    };
    let keys = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        k.sort();
        k
    };
    let schema = read("pairings.schema.json");
    assert_eq!(keys(&schema["properties"]), ["$schema", "pairings"]);
    assert_eq!(
        schema["properties"]["$schema"]["const"],
        PAIRINGS_SCHEMA_REF
    );
    let entry = &schema["$defs"]["pairing"];
    assert_eq!(keys(&entry["properties"]), ["families", "source"]);
    assert_eq!(entry["additionalProperties"], false);
    assert_eq!(entry["properties"]["families"]["minItems"], 2);
    assert_eq!(entry["properties"]["families"]["maxItems"], 2);
    assert_eq!(entry["properties"]["source"]["maxLength"], 120);

    let schema = read("model.schema.json");
    assert_eq!(
        keys(&schema["properties"]),
        ["$schema", "features", "seed", "version", "weights"]
    );
    assert_eq!(schema["properties"]["$schema"]["const"], MODEL_SCHEMA_REF);
    assert_eq!(schema["properties"]["version"]["const"], MODEL_VERSION);
    assert_eq!(schema["additionalProperties"], false);
    let v: Value = serde_json::from_str(MODEL_FILE.1).expect("JSON");
    assert_eq!(v["$schema"], MODEL_SCHEMA_REF);
    let lower_snake = |f: &Value| {
        f.as_str()
            .is_some_and(|f| f.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
    };
    assert!(
        v["features"]
            .as_array()
            .expect("features")
            .iter()
            .all(lower_snake)
    );
}

#[test]
fn the_committed_model_loads_and_is_under_4_kb() {
    assert!(MODEL_FILE.1.len() < 4096, "{} bytes", MODEL_FILE.1.len());
    let model = embedded().expect("model.json loads");
    assert_eq!(model.seed, SEED);
    // What the curated pairings teach: a shared designer and a related
    // family name count for a pairing.
    assert!(model.weights[index_of("same_designer")] > 0.0);
    assert!(model.weights[index_of("same_superfamily")] > 0.0);
}

#[test]
fn a_model_survives_its_json_and_a_bad_file_is_refused() {
    let model = Model {
        seed: 7,
        weights: std::array::from_fn(|i| i as f64 * 0.25 - 1.0),
    };
    let json = model.to_json();
    assert_eq!(Model::from_json("m.json", &json).expect("parse"), model);
    let wrong_features = json.replace("\"bias\"", "\"intercept\"");
    let err = Model::from_json("m.json", &wrong_features).expect_err("features");
    assert!(format!("{err:?}").contains("retrain"), "{err:?}");
    let short = json.replace("[-1.000000, ", "[");
    assert!(Model::from_json("m.json", &short).is_err());
    let version = json.replace("\"version\": 1", "\"version\": 2");
    assert!(Model::from_json("m.json", &version).is_err());
    assert!(Model::from_json("m.json", "not json").is_err());
}

#[test]
fn the_series_exp_matches_the_library_exp() {
    for x in [-30.0, -5.0, -1.0, -0.1, 0.0, 0.3, 1.0, 2.5, 10.0, 30.0] {
        let (got, want) = (exp(x), f64::exp(x));
        assert!(
            ((got - want) / want).abs() < 1e-9,
            "exp({x}) = {got}, want {want}"
        );
    }
    assert!(sigmoid(100.0) <= 1.0 && sigmoid(-100.0) >= 0.0);
    assert!((sigmoid(0.0) - 0.5).abs() < 1e-12);
}

#[test]
fn fitting_learns_a_separable_feature_and_is_deterministic() {
    let example = |designer: bool| {
        let mut x: Features = [0.0; FEATURES.len()];
        x[0] = 1.0;
        x[index_of("same_designer")] = if designer { 1.0 } else { 0.0 };
        (x, designer)
    };
    let examples: Vec<_> = (0..20).map(|i| example(i % 3 == 0)).collect();
    let model = fit(&examples, 1);
    assert_eq!(model, fit(&examples, 1));
    assert!(model.weights[index_of("same_designer")] > 1.0);
    assert!(model.probability(&examples[0].0) > 0.8);
    assert!(model.probability(&examples[1].0) < 0.2);
    assert_eq!(fit(&[], 3).weights, [0.0; FEATURES.len()]);
}

#[test]
fn sampling_draws_distinct_items_the_same_way_each_time() {
    let items: Vec<u32> = (0..50).collect();
    let a = Rng::new(SEED).sample(&items, 6);
    assert_eq!(a, Rng::new(SEED).sample(&items, 6));
    assert_ne!(a, Rng::new(SEED + 1).sample(&items, 6));
    let mut distinct = a.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 6);
    assert_eq!(Rng::new(1).sample(&items[..3], 6).len(), 3);
    assert!(Rng::new(1).sample::<u32>(&[], 6).is_empty());
}

#[test]
fn features_describe_the_pair_both_ways() {
    let serif = face(
        "Libre Baskerville",
        Some(Serif),
        Some("Pablo Impallari, Rodrigo Fuenzalida"),
    );
    let sans = FaceFeatures {
        weight: 700,
        width: 3,
        ..face("Cabin Condensed", Some(Sans), Some("Pablo Impallari"))
    };
    let x = features(&serif, &sans);
    assert_eq!(x, features(&sans, &serif));
    assert_eq!(x[index_of("bias")], 1.0);
    assert_eq!(x[index_of("text_contrast")], 1.0);
    assert_eq!(x[index_of("accent_contrast")], 0.0);
    assert_eq!(x[index_of("same_class")], 0.0);
    assert_eq!(x[index_of("x_height_closeness")], 1.0);
    assert!((x[index_of("weight_gap")] - 0.3).abs() < 1e-9);
    assert!((x[index_of("width_gap")] - 0.25).abs() < 1e-9);
    assert_eq!(x[index_of("same_designer")], 1.0);
    assert_eq!(x[index_of("same_superfamily")], 0.0);
    assert_eq!(x[index_of("fingerprints")], 0.0);
    assert!(x.iter().all(|v| (0.0..=1.0).contains(v)));

    let script = face("Clicker Script", Some(Script), None);
    let x = features(&script, &serif);
    assert_eq!(x[index_of("accent_contrast")], 1.0);
    assert_eq!(x[index_of("text_contrast")], 0.0);
    let x = features(
        &face("A", Some(Display), None),
        &face("B", Some(Display), None),
    );
    assert_eq!(x[index_of("same_class")], 1.0);
    assert_eq!(x[index_of("accent_contrast")], 0.0);
    // An unknown x-height or class gives nothing.
    let unknown = FaceFeatures {
        facts: PairFacts {
            x_height: None,
            class: None,
            ..serif.facts
        },
        ..serif
    };
    let x = features(&unknown, &sans);
    assert_eq!(x[index_of("x_height_closeness")], 0.0);
    assert_eq!(x[index_of("text_contrast")], 0.0);
    assert_eq!(x[index_of("same_class")], 0.0);
}

#[test]
fn fingerprints_count_only_when_both_faces_have_one() {
    const INK: [f32; 4] = [0.2, 0.4, 0.6, 0.8];
    const OTHER: [f32; 4] = [0.4, 0.4, 0.6, 0.8];
    let with = |print: &'static [f32]| FaceFeatures {
        fingerprint: Some(print),
        ..face("A", Some(Sans), None)
    };
    let x = features(&with(&INK), &with(&OTHER));
    assert_eq!(x[index_of("fingerprints")], 1.0);
    assert!((x[index_of("fingerprint_closeness")] - 0.95).abs() < 1e-6);
    let x = features(&with(&INK), &face("B", Some(Sans), None));
    assert_eq!(x[index_of("fingerprints")], 0.0);
    assert_eq!(x[index_of("fingerprint_closeness")], 0.0);
    // Vectors of other lengths do not compare.
    let x = features(&with(&INK), &with(&INK[..2]));
    assert_eq!(x[index_of("fingerprints")], 0.0);
}

#[test]
fn designers_and_families_relate_by_a_shared_name() {
    assert!(shared_designer(
        Some("Eduardo Tunni / Pablo Cosgaya"),
        Some("pablo cosgaya")
    ));
    assert!(shared_designer(
        Some("Jelle Bosma - Monotype Design Team"),
        Some("Monotype Design Team")
    ));
    assert!(shared_designer(
        Some("Danh Hong and the Monotype Design Team"),
        Some("Danh Hong")
    ));
    assert!(!shared_designer(Some("Ana"), Some("Bob")));
    assert!(!shared_designer(None, Some("Bob")));
    assert!(!shared_designer(Some(" , "), Some(" , ")));

    assert!(related_families("Cossette Titre", "Cossette Texte"));
    assert!(related_families(
        "Noto Sans Georgian",
        "Noto Serif Gurmukhi"
    ));
    assert!(related_families("Ubuntu", "Ubuntu Mono"));
    assert!(!related_families("Ubuntu", "ubuntu"));
    assert!(!related_families("DM Sans 18pt", "Mona Sans"));
    assert!(!related_families("Mona Sans", "Hubot Sans"));
}
