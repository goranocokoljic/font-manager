//! The named themes: `data/themes/*.json` (E47.1).
//!
//! The files are embedded at build time, so the list needs no file access.
//! [`load`] parses them and checks every rule of `theme.schema.json` plus the
//! ones a schema cannot say: the id is the file name and unique, and the colors
//! reach their contrast (WCAG AA): on-accent on accent 4.5:1, for words; accent
//! on the three surfaces 3:1, for the focus ring.

use std::sync::LazyLock;

use fm_api::settings::{DEFAULT_THEME_ID, ThemeDefinition, ThemeTokens};
use serde::Deserialize;

use crate::error::{FmError, FmResult};

/// The value each data file has in `$schema`.
const SCHEMA_REF: &str = "./theme.schema.json";

/// The least contrast of words on the accent.
const MIN_TEXT_CONTRAST: f64 = 4.5;

/// The least contrast of the accent as a focus ring on a surface.
const MIN_RING_CONTRAST: f64 = 3.0;

/// The data files, embedded at build time, in list order: the default first.
const FILES: [(&str, &str); 4] = [
    (
        "paper.json",
        include_str!("../../../data/themes/paper.json"),
    ),
    (
        "graphite.json",
        include_str!("../../../data/themes/graphite.json"),
    ),
    (
        "indigo.json",
        include_str!("../../../data/themes/indigo.json"),
    ),
    ("moss.json", include_str!("../../../data/themes/moss.json")),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    #[serde(rename = "$schema")]
    schema: String,
    id: String,
    name: String,
    light: RawTokens,
    dark: RawTokens,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawTokens {
    accent: String,
    on_accent: String,
    bg: String,
    surface: String,
    surface_2: String,
    border: String,
}

static THEMES: LazyLock<FmResult<Vec<ThemeDefinition>>> = LazyLock::new(|| parse_all(&FILES));

/// The themes the app ships, parsed once; the default first.
pub fn load() -> FmResult<&'static [ThemeDefinition]> {
    THEMES.as_ref().map(Vec::as_slice).map_err(Clone::clone)
}

/// Whether the app ships a theme with this id.
pub fn is_known(id: &str) -> FmResult<bool> {
    Ok(load()?.iter().any(|t| t.id == id))
}

/// Parses and checks the files, then checks the list as a whole.
fn parse_all(files: &[(&str, &str)]) -> FmResult<Vec<ThemeDefinition>> {
    let mut themes: Vec<ThemeDefinition> = Vec::with_capacity(files.len());
    for (name, json) in files {
        let theme = parse(name, json)?;
        if themes.iter().any(|t| t.id == theme.id) {
            return Err(damaged(name, &format!("id {} is used twice", theme.id)));
        }
        themes.push(theme);
    }
    if themes.first().is_none_or(|t| t.id != DEFAULT_THEME_ID) {
        return Err(FmError::internal(format!(
            "data/themes: {DEFAULT_THEME_ID} must come first"
        )));
    }
    Ok(themes)
}

/// Parses and checks one theme file.
fn parse(file: &str, json: &str) -> FmResult<ThemeDefinition> {
    let raw: RawTheme = serde_json::from_str(json).map_err(|e| damaged(file, &e.to_string()))?;
    if raw.schema != SCHEMA_REF {
        return Err(damaged(file, &format!("$schema must be \"{SCHEMA_REF}\"")));
    }
    if !is_id(&raw.id) {
        return Err(damaged(file, "id must be lowercase letters, digits and -"));
    }
    if file.strip_suffix(".json") != Some(raw.id.as_str()) {
        return Err(damaged(file, "id must be the file name without .json"));
    }
    if raw.name.is_empty() || raw.name.trim() != raw.name {
        return Err(damaged(file, "name is empty or has spaces around it"));
    }
    let light = tokens(raw.light).map_err(|why| damaged(file, &format!("light: {why}")))?;
    let dark = tokens(raw.dark).map_err(|why| damaged(file, &format!("dark: {why}")))?;
    Ok(ThemeDefinition {
        id: raw.id,
        name: raw.name,
        light,
        dark,
    })
}

/// Checks the colors and the contrast of one token set.
fn tokens(raw: RawTokens) -> Result<ThemeTokens, String> {
    let accent = opaque("accent", &raw.accent)?;
    let on_accent = opaque("on-accent", &raw.on_accent)?;
    let surfaces = [
        ("bg", opaque("bg", &raw.bg)?),
        ("surface", opaque("surface", &raw.surface)?),
        ("surface-2", opaque("surface-2", &raw.surface_2)?),
    ];
    let has_alpha = raw
        .border
        .strip_prefix('#')
        .is_some_and(|digits| digits.len() == 8 && is_hex(digits));
    if !has_alpha {
        opaque("border", &raw.border)?;
    }
    let ratio = contrast(on_accent, accent);
    if ratio < MIN_TEXT_CONTRAST {
        return Err(format!(
            "on-accent on accent is {ratio:.2}:1, below {MIN_TEXT_CONTRAST}:1"
        ));
    }
    for (name, surface) in surfaces {
        let ratio = contrast(accent, surface);
        if ratio < MIN_RING_CONTRAST {
            return Err(format!(
                "accent on {name} is {ratio:.2}:1, below {MIN_RING_CONTRAST}:1"
            ));
        }
    }
    Ok(ThemeTokens {
        accent: raw.accent,
        on_accent: raw.on_accent,
        bg: raw.bg,
        surface: raw.surface,
        surface_2: raw.surface_2,
        border: raw.border,
    })
}

/// The schema's `^[a-z][a-z0-9-]*$`.
fn is_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Lowercase hex digits only.
fn is_hex(digits: &str) -> bool {
    digits
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The red, green and blue of a `#rrggbb` color.
fn opaque(name: &str, color: &str) -> Result<[u8; 3], String> {
    let refuse = || format!("{name} {color:?} is not #rrggbb in lowercase");
    let digits = color.strip_prefix('#').ok_or_else(refuse)?;
    if digits.len() != 6 || !is_hex(digits) {
        return Err(refuse());
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).map_err(|_| refuse());
    Ok([channel(0)?, channel(2)?, channel(4)?])
}

/// The relative luminance of an sRGB color (WCAG 2).
fn luminance(color: [u8; 3]) -> f64 {
    let linear = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

/// The contrast ratio of two colors (WCAG 2), from 1 to 21.
fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn damaged(file: &str, detail: &str) -> FmError {
    FmError::internal(format!("data/themes/{file}: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::*;

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/themes")
    }

    /// A valid theme file named `test.json`.
    fn valid() -> Value {
        let set = json!({
            "accent": "#000000",
            "on-accent": "#ffffff",
            "bg": "#ffffff",
            "surface": "#fefefe",
            "surface-2": "#fdfdfd",
            "border": "#00000017"
        });
        json!({
            "$schema": "./theme.schema.json",
            "id": "test",
            "name": "Test",
            "light": set,
            "dark": set
        })
    }

    fn parsed(file: &Value) -> FmResult<ThemeDefinition> {
        parse("test.json", &file.to_string())
    }

    /// The message and the detail of the refusal of `file`.
    fn refused(file: &Value) -> String {
        parsed(file).expect_err("refused").to_string()
    }

    #[test]
    fn the_app_ships_paper_first_then_graphite_indigo_and_moss() {
        let themes = load().expect("the theme files are valid");
        let list: Vec<(&str, &str)> = themes
            .iter()
            .map(|t| (t.id.as_str(), t.name.as_str()))
            .collect();
        assert_eq!(
            list,
            [
                ("paper", "Paper"),
                ("graphite", "Graphite"),
                ("indigo", "Indigo"),
                ("moss", "Moss")
            ]
        );
        assert_eq!(themes[0].id, DEFAULT_THEME_ID);
    }

    #[test]
    fn every_pair_of_every_shipped_theme_reaches_its_contrast() {
        for theme in load().expect("valid") {
            for (mode, set) in [("light", &theme.light), ("dark", &theme.dark)] {
                let color = |c: &str| opaque(c, c).expect("opaque");
                let accent = color(&set.accent);
                let ratio = contrast(color(&set.on_accent), accent);
                assert!(ratio >= 4.5, "{} {mode}: on-accent {ratio:.2}", theme.id);
                for surface in [&set.bg, &set.surface, &set.surface_2] {
                    let ratio = contrast(accent, color(surface));
                    assert!(ratio >= 3.0, "{} {mode}: {surface} {ratio:.2}", theme.id);
                }
            }
        }
    }

    #[test]
    fn contrast_is_the_wcag_ratio() {
        assert!((contrast([0, 0, 0], [255, 255, 255]) - 21.0).abs() < 1e-9);
        assert!((contrast([255, 255, 255], [0, 0, 0]) - 21.0).abs() < 1e-9);
        assert!((contrast([9, 9, 9], [9, 9, 9]) - 1.0).abs() < 1e-9);
        // #777777 on white is the well-known near miss of AA.
        let gray = contrast([0x77, 0x77, 0x77], [255, 255, 255]);
        assert!((gray - 4.48).abs() < 0.01, "{gray}");
    }

    #[test]
    fn a_pair_just_above_the_limit_passes_and_just_below_it_is_refused() {
        let mut file = valid();
        // On white: #767676 is 4.54:1, #777777 is 4.48:1.
        file["light"]["on-accent"] = json!("#ffffff");
        file["light"]["accent"] = json!("#767676");
        assert!(parsed(&file).is_ok());
        file["light"]["accent"] = json!("#777777");
        assert!(refused(&file).contains("light: on-accent on accent"));

        // The dark set is checked as well.
        let mut file = valid();
        file["dark"]["accent"] = json!("#777777");
        assert!(refused(&file).contains("dark: on-accent on accent"));
    }

    #[test]
    fn an_accent_that_vanishes_on_any_surface_is_refused() {
        // #767676 is 4.54:1 on white, so it passes with on-accent; on a light
        // gray surface it stays above 3:1, on a mid gray it does not.
        for surface in ["bg", "surface", "surface-2"] {
            let mut file = valid();
            file["light"]["accent"] = json!("#767676");
            file["light"][surface] = json!("#f0f0f0");
            assert!(parsed(&file).is_ok(), "{surface} is still 3:1");
            file["light"][surface] = json!("#909090");
            assert!(
                refused(&file).contains(&format!("accent on {surface}")),
                "{surface}"
            );
        }
    }

    #[test]
    fn a_broken_file_is_refused_with_its_name_and_the_reason() {
        type Edit = fn(&mut Value);
        let cases: [(&str, Edit); 12] = [
            ("$schema", |f| f["$schema"] = json!("./other.json")),
            ("id must be lowercase", |f| f["id"] = json!("Test")),
            ("id must be the file name", |f| f["id"] = json!("other")),
            ("name is empty", |f| f["name"] = json!("")),
            ("name is empty", |f| f["name"] = json!(" Test")),
            ("accent", |f| f["light"]["accent"] = json!("#ABCDEF")),
            ("accent", |f| f["dark"]["accent"] = json!("black")),
            ("bg", |f| f["dark"]["bg"] = json!("#fff")),
            // Only the border may have alpha.
            ("surface", |f| f["light"]["surface"] = json!("#ffffff80")),
            ("border", |f| f["light"]["border"] = json!("#00000")),
            ("missing field", |f| {
                f["light"].as_object_mut().expect("set").remove("border");
            }),
            ("unknown field", |f| f["extra"] = json!(1)),
        ];
        for (why, edit) in cases {
            let mut file = valid();
            edit(&mut file);
            let error = refused(&file);
            assert!(error.contains("data/themes/test.json"), "{why}: {error}");
            assert!(error.contains(why), "{why}: {error}");
        }
        assert!(
            parsed(&valid()).is_ok(),
            "the cases start from a valid file"
        );
    }

    #[test]
    fn the_list_refuses_an_id_twice_and_a_default_that_is_not_first() {
        let paper = FILES[0];
        let moss = FILES[3];
        assert!(parse_all(&[paper, moss]).is_ok());
        let twice = parse_all(&[paper, moss, moss]).expect_err("twice");
        assert!(twice.to_string().contains("used twice"));
        let late = parse_all(&[moss, paper]).expect_err("late");
        assert!(late.to_string().contains("must come first"));
        assert!(parse_all(&[]).is_err(), "no default at all");
    }

    #[test]
    fn known_ids_are_the_shipped_ones() {
        assert!(is_known("moss").expect("valid"));
        assert!(!is_known("Moss").expect("valid"));
        assert!(!is_known("").expect("valid"));
    }

    #[test]
    fn every_data_file_is_embedded() {
        let mut on_disk: Vec<String> = std::fs::read_dir(data_dir())
            .expect("data/themes exists")
            .map(|e| {
                e.expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|n| n.ends_with(".json") && n != "theme.schema.json")
            .collect();
        on_disk.sort();
        let mut embedded: Vec<&str> = FILES.iter().map(|(n, _)| *n).collect();
        embedded.sort_unstable();
        assert_eq!(
            on_disk, embedded,
            "add new data files to FILES in themes.rs"
        );
    }

    #[test]
    fn embedded_files_are_the_files_on_disk() {
        for (name, embedded) in FILES {
            let disk = std::fs::read_to_string(data_dir().join(name)).expect("read");
            assert_eq!(disk.replace("\r\n", "\n"), embedded.replace("\r\n", "\n"));
        }
    }

    #[test]
    fn the_schema_and_the_loader_agree() {
        let text = std::fs::read_to_string(data_dir().join("theme.schema.json")).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(
            keys(&schema["properties"]),
            ["$schema", "dark", "id", "light", "name"]
        );
        assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
        assert_eq!(schema["additionalProperties"], false);
        let tokens = &schema["$defs"]["tokens"];
        let names = [
            "accent",
            "bg",
            "border",
            "on-accent",
            "surface",
            "surface-2",
        ];
        assert_eq!(keys(&tokens["properties"]), names);
        assert_eq!(tokens["additionalProperties"], false);
        assert_eq!(
            tokens["required"].as_array().expect("required").len(),
            names.len()
        );
        assert_eq!(schema["$defs"]["opaque"]["pattern"], "^#[0-9a-f]{6}$");
        assert_eq!(schema["properties"]["id"]["pattern"], "^[a-z][a-z0-9-]*$");
        assert_eq!(
            tokens["properties"]["border"]["pattern"],
            "^#[0-9a-f]{6}([0-9a-f]{2})?$"
        );
    }
}
