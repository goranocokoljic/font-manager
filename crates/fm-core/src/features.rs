//! OpenType layout features by name (E10.3, E42.11): the friendly name, group
//! and sample text of every tag in the OpenType feature registry, which
//! features the shaper applies by itself, and which are on in default text
//! rendering.
//!
//! The catalog is `data/features/features.json`, embedded at build time. It
//! lists every registered tag, also `ss01`–`ss20` ("Stylistic set N") and
//! `cv01`–`cv99` ("Character variant N"); a font's own name replaces those
//! (E10.4, [`named_by_font`]). A tag that is not registered has no name, group
//! or sample; the UI shows the tag.
//!
//! "Automatic" and "default on" follow what browsers do (HarfBuzz): the
//! automatic features are required for correct text in some script (they
//! compose marks, pick contextual forms, place marks), so the Features tab
//! lists them but does not offer a switch. The default-on features are on
//! unless `font-feature-settings` turns them off.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use fm_api::common::{FeatureGroup, FeatureInfo};
use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value the data file has in `$schema`.
const SCHEMA_REF: &str = "./features.schema.json";

/// The data file, embedded at build time.
const FILE: (&str, &str) = (
    "features.json",
    include_str!("../../../data/features/features.json"),
);

/// What the catalog says about one tag.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    name: String,
    group: FeatureGroup,
    sample: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(rename = "$schema")]
    schema: String,
    features: Vec<RawFeature>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFeature {
    tag: String,
    name: String,
    group: FeatureGroup,
    sample: String,
}

/// The catalog, loaded once. The data is embedded and its tests reject a broken
/// file, so a load error is a build defect: it is logged and no tag has a name.
static CATALOG: LazyLock<BTreeMap<String, Entry>> = LazyLock::new(|| match parse(FILE.0, FILE.1) {
    Ok(catalog) => catalog,
    Err(e) => {
        tracing::error!(error = %e, "feature catalog could not be loaded");
        BTreeMap::new()
    }
});

/// Parses and checks the data file: every rule of `features.schema.json`, plus
/// the sort order, which also rejects a tag listed twice.
fn parse(file: &str, json: &str) -> FmResult<BTreeMap<String, Entry>> {
    let raw: RawFile = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if raw.features.is_empty() {
        return Err(damaged(file, "features is empty"));
    }
    if let Some(w) = raw.features.windows(2).find(|w| w[0].tag >= w[1].tag) {
        return Err(damaged(
            file,
            &format!("{}: tag is out of order or listed twice", w[1].tag),
        ));
    }
    raw.features
        .into_iter()
        .map(|f| {
            let valid_tag = f.tag.len() == 4
                && f.tag
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
            if !valid_tag {
                return Err(damaged(file, &format!("{}: tag is not valid", f.tag)));
            }
            for (field, text) in [("name", &f.name), ("sample", &f.sample)] {
                if text.trim().is_empty() || text.trim() != text {
                    return Err(damaged(
                        file,
                        &format!("{}: {field} is empty or has white space at an end", f.tag),
                    ));
                }
            }
            Ok((
                f.tag,
                Entry {
                    name: f.name,
                    group: f.group,
                    sample: f.sample,
                },
            ))
        })
        .collect()
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/features/{file}: {detail}"))
}

/// Features the shaper applies by itself where the script needs them, sorted.
pub const AUTOMATIC: &[&str] = &[
    "abvf", "abvm", "abvs", "akhn", "blwf", "blwm", "blws", "ccmp", "cfar", "cjct", "curs", "dist",
    "fin2", "fin3", "fina", "half", "haln", "init", "isol", "ljmo", "locl", "ltra", "ltrm", "mark",
    "med2", "medi", "mkmk", "mset", "nukt", "pref", "pres", "pstf", "psts", "rclt", "rkrf", "rlig",
    "rphf", "rtla", "rtlm", "rvrn", "stch", "tjmo", "vatu", "vjmo",
];

/// Features a user can switch that are on in default text rendering, sorted.
pub const DEFAULT_ON: &[&str] = &["calt", "clig", "kern", "liga"];

/// The number `N` of a `{prefix}NN` tag, when it is in `1..=max`.
fn numbered(tag: &str, prefix: &str, max: u8) -> Option<u8> {
    let digits = tag.strip_prefix(prefix)?;
    if digits.len() != 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u8 = digits.parse().ok()?;
    (1..=max).contains(&n).then_some(n)
}

/// Whether the font can name `tag` itself: the stylistic sets `ss01`–`ss20`
/// (FeatureParams UINameID) and the character variants `cv01`–`cv99`
/// (FeatureUINameID).
pub fn named_by_font(tag: &str) -> bool {
    numbered(tag, "ss", 20).is_some() || numbered(tag, "cv", 99).is_some()
}

/// The friendly name of `tag`, or `None` when the tag is not registered.
pub fn feature_name(tag: &str) -> Option<String> {
    CATALOG.get(tag).map(|e| e.name.clone())
}

/// What the Features tab needs to know about `tag`.
pub fn feature_info(tag: &str) -> FeatureInfo {
    feature_info_named(tag, None)
}

/// [`feature_info`] with the font's own name for `tag` ([`named_by_font`]),
/// which replaces the generic "Stylistic set N" when there is one.
pub fn feature_info_named(tag: &str, font_name: Option<&str>) -> FeatureInfo {
    let entry = CATALOG.get(tag);
    FeatureInfo {
        tag: tag.to_owned(),
        name: font_name
            .map(str::to_owned)
            .or_else(|| entry.map(|e| e.name.clone())),
        group: entry.map(|e| e.group),
        sample: entry.map(|e| e.sample.clone()),
        automatic: AUTOMATIC.binary_search(&tag).is_ok(),
        default_on: DEFAULT_ON.binary_search(&tag).is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use super::*;
    use crate::parser;

    /// The OpenType feature registry without the numbered tags. A copy on
    /// purpose: it fails the tests when an edit drops a tag from the data file.
    const REGISTERED: &[&str] = &[
        "aalt", "abvf", "abvm", "abvs", "afrc", "akhn", "apkn", "blwf", "blwm", "blws", "c2pc",
        "c2sc", "calt", "case", "ccmp", "cfar", "chws", "cjct", "clig", "cpct", "cpsp", "cswh",
        "curs", "dist", "dlig", "dnom", "dtls", "expt", "falt", "fin2", "fin3", "fina", "flac",
        "frac", "fwid", "half", "haln", "halt", "hist", "hkna", "hlig", "hngl", "hojo", "hwid",
        "init", "isol", "ital", "jalt", "jp04", "jp78", "jp83", "jp90", "kern", "lfbd", "liga",
        "ljmo", "lnum", "locl", "ltra", "ltrm", "mark", "med2", "medi", "mgrk", "mkmk", "mset",
        "nalt", "nlck", "nukt", "numr", "onum", "opbd", "ordn", "ornm", "palt", "pcap", "pkna",
        "pnum", "pref", "pres", "pstf", "psts", "pwid", "qwid", "rand", "rclt", "rkrf", "rlig",
        "rphf", "rtbd", "rtla", "rtlm", "ruby", "rvrn", "salt", "sinf", "size", "smcp", "smpl",
        "ssty", "stch", "subs", "sups", "swsh", "titl", "tjmo", "tnam", "tnum", "trad", "twid",
        "unic", "valt", "vapk", "vatu", "vchw", "vert", "vhal", "vjmo", "vkna", "vkrn", "vpal",
        "vrt2", "vrtr", "zero",
    ];

    fn numbered_tags() -> Vec<String> {
        let sets = (1..=20).map(|n| format!("ss{n:02}"));
        sets.chain((1..=99).map(|n| format!("cv{n:02}"))).collect()
    }

    fn sorted_unique(tags: &[&str]) -> bool {
        tags.windows(2).all(|w| w[0] < w[1])
    }

    #[test]
    fn the_data_file_lists_every_registered_tag() {
        let expected: BTreeSet<String> = REGISTERED
            .iter()
            .map(|t| (*t).to_owned())
            .chain(numbered_tags())
            .collect();
        let listed: BTreeSet<String> = parse(FILE.0, FILE.1)
            .expect("embedded data loads")
            .into_keys()
            .collect();
        assert_eq!(listed, expected);
        for tag in &expected {
            let info = feature_info(tag);
            assert!(info.name.is_some_and(|n| !n.is_empty()), "{tag}");
            assert!(info.group.is_some(), "{tag}");
            assert!(info.sample.is_some_and(|s| !s.is_empty()), "{tag}");
        }
    }

    #[test]
    fn the_embedded_file_is_the_file_on_disk() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/features/features.json");
        let disk = std::fs::read_to_string(path).expect("readable");
        assert_eq!(disk.replace("\r\n", "\n"), FILE.1.replace("\r\n", "\n"));
    }

    /// Every file under `dir`, at any depth.
    fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read fixture dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                files_under(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    #[test]
    fn every_tag_of_the_fixture_corpus_has_a_name() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts");
        let mut files = Vec::new();
        files_under(&root, &mut files);
        let mut seen = BTreeSet::new();
        let mut missing = BTreeSet::new();
        for path in &files {
            let bytes = std::fs::read(path).expect("read fixture");
            for face in parser::parse(&bytes).faces {
                for tag in &face.features {
                    seen.insert(tag.clone());
                    let font_name = face.feature_names.get(tag).map(String::as_str);
                    let info = feature_info_named(tag, font_name);
                    if info.name.is_none() {
                        missing.insert(tag.clone());
                    }
                    // A set or variant the font does not name gets the generic name.
                    if font_name.is_none() && named_by_font(tag) {
                        let name = info.name.as_deref().unwrap_or_default();
                        assert!(
                            name.starts_with("Stylistic set ")
                                || name.starts_with("Character variant "),
                            "{tag} in {path:?}: {name:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(missing, BTreeSet::new(), "tags without a name");
        // The walk reached the fonts: these tags are in many of them.
        for tag in ["kern", "liga", "mark", "ss01"] {
            assert!(
                seen.contains(tag),
                "{tag} not seen in {} files",
                files.len()
            );
        }
    }

    #[test]
    fn numbered_tags_without_a_font_name_fall_back() {
        assert_eq!(
            feature_info_named("cv03", None).name.as_deref(),
            Some("Character variant 3")
        );
        assert_eq!(
            feature_info_named("ss01", None).name.as_deref(),
            Some("Stylistic set 1")
        );
    }

    #[test]
    fn groups_and_samples_from_the_story() {
        let cases = [
            ("liga", FeatureGroup::Ligatures, "fi fl ffi ffl"),
            ("smcp", FeatureGroup::Capitals, "Small Capitals"),
            ("onum", FeatureGroup::Figures, "0123456789"),
            ("frac", FeatureGroup::Fractions, "1/2 3/4 7/8"),
            ("kern", FeatureGroup::Positioning, "AVATAR Toy"),
            ("swsh", FeatureGroup::Stylistic, "Hamburg"),
            ("locl", FeatureGroup::Language, "şţ ij б"),
            ("ccmp", FeatureGroup::Automatic, "é ñ ü"),
        ];
        for (tag, group, sample) in cases {
            let info = feature_info(tag);
            assert_eq!(info.group, Some(group), "{tag}");
            assert_eq!(info.sample.as_deref(), Some(sample), "{tag}");
        }
    }

    #[test]
    fn numbered_tags_are_stylistic() {
        for tag in numbered_tags() {
            assert_eq!(
                feature_info(&tag).group,
                Some(FeatureGroup::Stylistic),
                "{tag}"
            );
        }
    }

    #[test]
    fn the_automatic_group_is_the_automatic_list_except_locl() {
        let catalog = parse(FILE.0, FILE.1).expect("embedded data loads");
        for (tag, entry) in &catalog {
            let in_group = entry.group == FeatureGroup::Automatic;
            let in_list = AUTOMATIC.contains(&tag.as_str());
            if tag == "locl" {
                assert!(in_list && !in_group);
            } else {
                assert_eq!(in_group, in_list, "{tag}");
            }
        }
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/features/features.schema.json");
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
        let entry = &schema["$defs"]["feature"];
        assert_eq!(
            sorted(&entry["required"]),
            ["group", "name", "sample", "tag"]
        );
        assert_eq!(entry["additionalProperties"], false);
        // The schema's groups are the API's groups.
        for group in entry["properties"]["group"]["enum"]
            .as_array()
            .expect("enum")
        {
            let parsed: FeatureGroup =
                serde_json::from_value(group.clone()).expect("group is in FeatureGroup");
            assert_eq!(&serde_json::to_value(parsed).expect("json"), group);
        }
        let groups = sorted(&entry["properties"]["group"]["enum"]);
        assert_eq!(groups.len(), 8, "{groups:?}");
        let v: Value = serde_json::from_str(FILE.1).expect("JSON");
        assert_eq!(v["$schema"], SCHEMA_REF);
    }

    #[test]
    fn broken_data_is_rejected() {
        let file = |features: &str| {
            format!(r#"{{"$schema": "./features.schema.json", "features": [{features}]}}"#)
        };
        let good = |tag: &str| {
            format!(r#"{{"tag": "{tag}", "name": "N", "group": "stylistic", "sample": "S"}}"#)
        };
        let cases = [
            (
                format!("{}, {}", good("aaaa"), good("aaaa")),
                "aaaa: tag is out of order or listed twice",
            ),
            (
                format!("{}, {}", good("bbbb"), good("aaaa")),
                "aaaa: tag is out of order or listed twice",
            ),
            (good("TOOLONG"), "tag is not valid"),
            (good("SMCP"), "tag is not valid"),
            (
                r#"{"tag": "aaaa", "name": " ", "group": "stylistic", "sample": "S"}"#.to_owned(),
                "name is empty",
            ),
            (
                r#"{"tag": "aaaa", "name": "N", "group": "stylistic", "sample": " S"}"#.to_owned(),
                "sample is empty or has white space",
            ),
            (
                r#"{"tag": "aaaa", "name": "N", "group": "nope", "sample": "S"}"#.to_owned(),
                "unknown variant",
            ),
            (
                r#"{"tag": "aaaa", "name": "N", "group": "stylistic", "sample": "S", "x": 1}"#
                    .to_owned(),
                "unknown field",
            ),
        ];
        for (features, why) in cases {
            let err = parse("t.json", &file(&features)).expect_err(&features);
            let detail = err.detail().unwrap_or_default();
            assert!(detail.contains(why), "{features}: {detail}");
            assert!(detail.starts_with("data/features/t.json"), "{detail}");
        }
        for json in [
            "{".to_owned(),
            file(""),
            format!(
                r#"{{"$schema": "other.json", "features": [{}]}}"#,
                good("aaaa")
            ),
        ] {
            assert!(parse("t.json", &json).is_err(), "{json}");
        }
    }

    #[test]
    fn named_examples_from_the_story() {
        assert_eq!(feature_name("smcp").as_deref(), Some("Small capitals"));
        assert_eq!(feature_name("onum").as_deref(), Some("Oldstyle figures"));
        assert_eq!(feature_name("liga").as_deref(), Some("Standard ligatures"));
        assert_eq!(feature_name("kern").as_deref(), Some("Kerning"));
    }

    #[test]
    fn stylistic_sets_and_character_variants_are_numbered() {
        for n in 1..=20 {
            assert_eq!(
                feature_name(&format!("ss{n:02}")),
                Some(format!("Stylistic set {n}"))
            );
        }
        for n in 1..=99 {
            assert_eq!(
                feature_name(&format!("cv{n:02}")),
                Some(format!("Character variant {n}"))
            );
        }
    }

    #[test]
    fn only_numbered_sets_and_variants_are_named_by_the_font() {
        for tag in ["ss01", "ss20", "cv01", "cv99"] {
            assert!(named_by_font(tag), "{tag}");
        }
        for tag in [
            "ss00", "ss21", "ssty", "cv00", "cv100", "cv1", "smcp", "size", "",
        ] {
            assert!(!named_by_font(tag), "{tag:?}");
        }
    }

    #[test]
    fn the_font_name_replaces_the_generic_one() {
        let named = feature_info_named("ss01", Some("Single-storey a"));
        assert_eq!(named.name.as_deref(), Some("Single-storey a"));
        assert!(!named.automatic && !named.default_on);
        // Only the name is the font's; the group and the sample stay the catalog's.
        assert_eq!(named.group, Some(FeatureGroup::Stylistic));
        assert_eq!(named.sample, feature_info("ss01").sample);
        assert_eq!(feature_info_named("ss01", None), feature_info("ss01"));
        assert_eq!(
            feature_info("ss01").name.as_deref(),
            Some("Stylistic set 1")
        );
    }

    #[test]
    fn unregistered_tags_have_no_name() {
        for tag in [
            "ss00", "ss21", "ss99", "cv00", "cv1", "cv100", "ssxx", "cv+1", "abcd", "SMCP", "",
            "kern ",
        ] {
            let info = feature_info(tag);
            assert_eq!(info.name, None, "{tag:?}");
            assert_eq!(info.group, None, "{tag:?}");
            assert_eq!(info.sample, None, "{tag:?}");
            assert_eq!(info.tag, tag);
            assert!(!info.automatic && !info.default_on, "{tag:?}");
        }
    }

    #[test]
    fn shaper_features_are_automatic_and_registered() {
        assert!(sorted_unique(AUTOMATIC));
        for tag in AUTOMATIC {
            let info = feature_info(tag);
            assert!(info.automatic, "{tag}");
            assert!(!info.default_on, "{tag}");
            assert!(info.name.is_some(), "{tag}");
        }
        for tag in [
            "ccmp", "locl", "mark", "mkmk", "rlig", "init", "medi", "fina", "rvrn",
        ] {
            assert!(feature_info(tag).automatic, "{tag}");
        }
    }

    #[test]
    fn browser_defaults_are_on_and_the_rest_off() {
        assert!(sorted_unique(DEFAULT_ON));
        for tag in ["kern", "liga", "calt"] {
            let info = feature_info(tag);
            assert!(info.default_on && !info.automatic, "{tag}");
        }
        for tag in [
            "smcp", "onum", "ss01", "cv01", "dlig", "tnum", "zero", "frac",
        ] {
            let info = feature_info(tag);
            assert!(!info.default_on && !info.automatic, "{tag}");
        }
    }
}
