use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use serde_json::Value;
use skrifa::{GlyphId, GlyphNames};

use super::*;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/subsets")
}

/// Latin, Latin Extended, Cyrillic and Greek, `liga` with `fi` and `ffi`,
/// and a `wght` axis.
const MULISH: &str = "google-repo/mulish/Mulish[wght].ttf";
const CFF_OTF: &str = "fontshare/asap/Asap-SemiBold.otf";

fn source(rel: &str) -> Sfnt {
    web_font::standalone(&std::fs::read(fixture(rel)).unwrap(), 0).unwrap()
}

fn presets_keep(names: &[&str]) -> Keep {
    code_points(&Subset::Presets {
        names: names.iter().map(|n| (*n).to_string()).collect(),
    })
    .unwrap()
}

fn mapped(sfnt: &[u8]) -> Vec<u32> {
    skrifa::FontRef::new(sfnt)
        .unwrap()
        .charmap()
        .mappings()
        .map(|(cp, _)| cp)
        .collect()
}

fn invalid(r: FmResult<Keep>) -> String {
    let e = r.unwrap_err();
    assert_eq!(e.code(), ErrorCode::InvalidRequest, "{e:?}");
    e.message().to_string()
}

#[test]
fn the_data_file_is_valid_and_has_the_named_presets() {
    let all = presets().expect("presets.json is valid");
    let names: Vec<_> = all.iter().map(|p| p.name.as_str()).collect();
    for want in ["latin", "latin-ext", "cyrillic", "greek", "vietnamese"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    let v: Value = serde_json::from_str(FILE).expect("JSON");
    assert!(
        v["source"]
            .as_str()
            .is_some_and(|s| s.starts_with("https://fonts.googleapis.com/")),
        "the ranges name where they come from"
    );
}

#[test]
fn the_embedded_file_is_the_file_on_disk() {
    let disk = std::fs::read_to_string(data_dir().join("presets.json")).expect("read");
    assert_eq!(disk.replace("\r\n", "\n"), FILE.replace("\r\n", "\n"));
}

#[test]
fn the_schema_and_the_loader_agree() {
    let text = std::fs::read_to_string(data_dir().join("presets.schema.json")).expect("schema");
    let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
    let keys = |v: &Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&schema["properties"]),
        ["$schema", "presets", "source"]
    );
    assert_eq!(schema["properties"]["$schema"]["const"], SCHEMA_REF);
    assert_eq!(schema["additionalProperties"], false);
    let preset = &schema["$defs"]["preset"];
    assert_eq!(keys(&preset["properties"]), ["name", "ranges"]);
    assert_eq!(preset["additionalProperties"], false);
    assert_eq!(preset["required"], serde_json::json!(["name", "ranges"]));

    // Every range of the data file matches the schema's pattern, and the
    // loader refuses one the pattern refuses.
    let pattern = preset["properties"]["ranges"]["items"]["pattern"]
        .as_str()
        .expect("pattern");
    assert_eq!(
        pattern,
        "^U[+]([0-9A-F]{1,6}(-[0-9A-F]{1,6})?|[0-9A-F]{0,5}[?]{1,6})$"
    );
    let data: Value = serde_json::from_str(FILE).expect("JSON");
    for p in data["presets"].as_array().expect("presets") {
        for r in p["ranges"].as_array().expect("ranges") {
            let r = r.as_str().expect("string");
            assert!(unicode_range(r).is_some(), "{r}");
            assert!(!r.chars().any(|c| c.is_ascii_lowercase()), "{r}");
        }
    }
}

fn file(presets: &str) -> String {
    format!(
        r#"{{"$schema": "./presets.schema.json", "source": "https://fonts.googleapis.com/css2",
           "presets": [{presets}]}}"#
    )
}

#[test]
fn the_loader_refuses_a_file_that_breaks_the_rules() {
    let ok = r#"{"name": "latin", "ranges": ["U+0000-00FF"]}"#;
    assert!(parse(&file(ok)).is_ok());
    for (bad, why) in [
        (file(&format!("{ok}, {ok}")), "listed twice"),
        (
            file(r#"{"name": "Latin", "ranges": ["U+0041"]}"#),
            "lower-case",
        ),
        (
            file(r#"{"name": "latin", "ranges": []}"#),
            "ranges is empty",
        ),
        (
            file(r#"{"name": "latin", "ranges": ["U+00ff"]}"#),
            "upper case",
        ),
        (
            file(r#"{"name": "latin", "ranges": ["U+00FF-0000"]}"#),
            "malformed",
        ),
        (
            file(r#"{"name": "latin", "ranges": ["U+110000"]}"#),
            "malformed",
        ),
        (
            file(r#"{"name": "latin", "ranges": ["0041"]}"#),
            "malformed",
        ),
        (
            file(r#"{"name": "latin", "ranges": ["U+0041"], "x": 1}"#),
            "unknown field",
        ),
        (file(""), "presets is empty"),
        (ok_with_schema("./other.json"), "$schema"),
    ] {
        let e = parse(&bad).unwrap_err();
        assert!(e.detail().is_some_and(|d| d.contains(why)), "{why}: {e:?}");
    }
}

fn ok_with_schema(schema: &str) -> String {
    format!(
        r#"{{"$schema": "{schema}", "source": "https://fonts.googleapis.com/css2",
           "presets": [{{"name": "latin", "ranges": ["U+0041"]}}]}}"#
    )
}

#[test]
fn css_unicode_ranges_parse_like_css() {
    assert_eq!(unicode_range("U+0041"), Some((0x41, 0x41)));
    assert_eq!(unicode_range("U+0000-00FF"), Some((0, 0xFF)));
    assert_eq!(unicode_range("u+0-10ffff"), Some((0, 0x10_FFFF)));
    assert_eq!(unicode_range("  U+1F600  "), Some((0x1_F600, 0x1_F600)));
    assert_eq!(unicode_range("U+4??"), Some((0x400, 0x4FF)));
    assert_eq!(unicode_range("U+??"), Some((0, 0xFF)));
    assert_eq!(unicode_range("U+10????"), Some((0x10_0000, 0x10_FFFF)));
    for bad in [
        "",
        "U+",
        "0041",
        "U+00FF-0000",
        "U+110000",
        "U+0041-110000",
        "U+4?1",
        "U+?4",
        "U+0000000",
        "U+11????",
        "U+???????",
        "U+GG",
        "U+00-",
        "U+-00FF",
        "U+0041 0042",
    ] {
        assert_eq!(unicode_range(bad), None, "{bad:?}");
    }
}

#[test]
fn presets_ranges_and_text_become_code_points() {
    let latin = presets_keep(&["latin"]);
    assert!(latin.contains(u32::from('A')) && latin.contains(0xFFFD));
    assert!(!latin.contains(u32::from('Ж')) && !latin.contains(u32::from('Ω')));
    let two = presets_keep(&["latin", "greek"]);
    assert!(two.contains(u32::from('A')) && two.contains(u32::from('Ω')));
    assert!(!two.contains(u32::from('Ж')));

    let ranges = code_points(&Subset::Unicode {
        ranges: vec!["U+0041-0043, U+4??".into(), "u+1f600".into()],
    })
    .unwrap();
    assert_eq!(
        ranges.ranges(),
        [(0x41, 0x43), (0x400, 0x4FF), (0x1_F600, 0x1_F600)]
    );

    let text = code_points(&Subset::Text {
        text: "Aä€Ж".into(),
    })
    .unwrap();
    assert_eq!(
        text.ranges(),
        [
            (0x20, 0x20),
            (0x41, 0x41),
            (0xE4, 0xE4),
            (0x416, 0x416),
            (0x20AC, 0x20AC)
        ]
    );
}

#[test]
fn bad_subsets_are_refused_in_plain_words() {
    let m = invalid(code_points(&Subset::Presets {
        names: vec!["latin".into(), "klingon".into()],
    }));
    assert!(m.contains("\"klingon\"") && m.contains("latin-ext"), "{m}");
    invalid(code_points(&Subset::Presets { names: Vec::new() }));
    let m = invalid(code_points(&Subset::Unicode {
        ranges: vec!["U+0041".into(), "U+ZZ".into()],
    }));
    assert!(m.contains("\"U+ZZ\"") && m.contains("U+0000-00FF"), "{m}");
    invalid(code_points(&Subset::Unicode {
        ranges: vec![" , ".into()],
    }));
    invalid(code_points(&Subset::Text {
        text: String::new(),
    }));
}

#[test]
fn keep_merges_ranges_and_answers_membership() {
    let k = Keep::from_ranges([(10, 20), (0, 5), (6, 8), (15, 30), (40, 40)]);
    assert_eq!(k.ranges(), [(0, 8), (10, 30), (40, 40)]);
    for (cp, want) in [
        (0, true),
        (8, true),
        (9, false),
        (30, true),
        (31, false),
        (40, true),
        (41, false),
    ] {
        assert_eq!(k.contains(cp), want, "{cp}");
    }
}

#[test]
fn the_subset_cmap_is_exactly_the_requested_characters_the_face_maps() {
    let src = source(MULISH);
    let src_bytes = web_font::sfnt_bytes(&src).unwrap();
    let keep = presets_keep(&["latin"]);
    let cut = cut(&src, &keep).unwrap();
    let want: Vec<u32> = mapped(&src_bytes)
        .into_iter()
        .filter(|&cp| keep.contains(cp))
        .collect();
    assert!(want.len() > 150, "Mulish maps most of Latin");
    assert_eq!(mapped(&web_font::sfnt_bytes(&cut).unwrap()), want);

    // A text keeps its characters and the space, nothing else.
    let text = code_points(&Subset::Text {
        text: "fiЖ".into()
    })
    .unwrap();
    let cut = super::cut(&src, &text).unwrap();
    assert_eq!(
        mapped(&web_font::sfnt_bytes(&cut).unwrap()),
        [0x20, u32::from('f'), u32::from('i'), u32::from('Ж')]
    );
}

/// The glyph names and advances harfrust gives `text` in `sfnt`.
fn shape(sfnt: &[u8], text: &str) -> Vec<(String, i32)> {
    let font = harfrust::Font::new(sfnt.to_vec(), 0).unwrap();
    let shaper = harfrust::ShaperFont::new(&font);
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::new()).unwrap();
    let names = GlyphNames::new(&skrifa::FontRef::new(sfnt).unwrap());
    buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, pos)| {
            let name = names
                .get(GlyphId::new(info.glyph_id))
                .map(|n| n.to_string())
                .unwrap_or_default();
            (name, pos.x_advance)
        })
        .collect()
}

#[test]
fn layout_closure_keeps_ligatures_with_the_same_glyphs_and_advances() {
    let src = source(MULISH);
    let cut = cut(&src, &presets_keep(&["latin"])).unwrap();
    let text = "office ffi fi";
    let before = shape(&web_font::sfnt_bytes(&src).unwrap(), text);
    let after = shape(&web_font::sfnt_bytes(&cut).unwrap(), text);
    assert!(
        before.len() < text.chars().count(),
        "the fixture forms ligatures: {before:?}"
    );
    assert!(before.iter().all(|(n, _)| !n.is_empty()), "{before:?}");
    assert_eq!(after, before);
}

#[test]
fn a_variable_font_stays_variable_with_the_same_axes() {
    let src = source(MULISH);
    let cut = cut(&src, &presets_keep(&["latin"])).unwrap();
    let axes = |s: &Sfnt| -> Vec<(String, f32, f32, f32)> {
        let bytes = web_font::sfnt_bytes(s).unwrap();
        skrifa::FontRef::new(&bytes)
            .unwrap()
            .axes()
            .iter()
            .map(|a| {
                (
                    a.tag().to_string(),
                    a.min_value(),
                    a.default_value(),
                    a.max_value(),
                )
            })
            .collect()
    };
    let before = axes(&src);
    assert_eq!(before.len(), 1, "{before:?}");
    assert_eq!(axes(&cut), before);
    let tags: Vec<_> = cut.tables.iter().map(|t| t.tag).collect();
    for t in [b"fvar", b"gvar", b"STAT"] {
        assert!(tags.contains(t), "{} kept", String::from_utf8_lossy(t));
    }
}

#[test]
fn a_latin_subset_is_smaller_than_the_whole_font() {
    let src = source(MULISH);
    let cut = cut(&src, &presets_keep(&["latin"])).unwrap();
    let whole = web_font::encode(&src, fm_api::export::WebFormat::Woff2).unwrap();
    let part = web_font::encode(&cut, fm_api::export::WebFormat::Woff2).unwrap();
    assert!(part.len() < whole.len(), "{} < {}", part.len(), whole.len());
}

#[test]
fn a_face_with_none_of_the_characters_is_refused() {
    let src = source(MULISH);
    for subset in [
        Subset::Unicode {
            ranges: vec!["U+4E00-9FFF".into()],
        },
        // The space added to a text does not count.
        Subset::Text {
            text: "日本".into(),
        },
    ] {
        let e = cut(&src, &code_points(&subset).unwrap()).unwrap_err();
        assert_eq!(e.code(), ErrorCode::InvalidRequest, "{e:?}");
        assert!(e.message().contains("none of the characters"), "{e:?}");
    }
    // A space the user typed is a character they asked for.
    let space = code_points(&Subset::Text {
        text: "日 本".into(),
    })
    .unwrap();
    assert!(cut(&src, &space).is_ok());
}

#[test]
fn a_cff_font_is_refused_rather_than_written_without_outlines() {
    let e = cut(&source(CFF_OTF), &presets_keep(&["latin"])).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Unsupported, "{e:?}");
    assert!(e.message().contains("CFF"), "{e:?}");
}

#[test]
fn the_mock_has_the_same_presets_as_the_data_file() {
    let file: Vec<(&str, Vec<(u32, u32)>)> = presets()
        .unwrap()
        .iter()
        .map(|p| (p.name.as_str(), p.ranges.clone()))
        .collect();
    let mock: Vec<(&str, Vec<(u32, u32)>)> = fm_api::mock::SUBSET_PRESETS
        .iter()
        .map(|(n, r)| (*n, r.to_vec()))
        .collect();
    assert_eq!(mock, file);
}

#[test]
fn the_mock_validates_subsets_the_same_way() {
    let presets = |names: &[&str]| Subset::Presets {
        names: names.iter().map(|n| (*n).to_string()).collect(),
    };
    let unicode = |ranges: &[&str]| Subset::Unicode {
        ranges: ranges.iter().map(|r| (*r).to_string()).collect(),
    };
    let text = |t: &str| Subset::Text { text: t.into() };
    let cases = [
        presets(&["latin"]),
        presets(&["latin", "greek", "vietnamese", " cyrillic "]),
        presets(&["latin-ext", "cyrillic-ext", "greek-ext"]),
        presets(&[]),
        presets(&["Latin"]),
        presets(&["klingon"]),
        unicode(&["U+0000-00FF"]),
        unicode(&["U+0041-0043, U+4??", "u+1f600", ""]),
        unicode(&["U+??", "U+10????", "U+0-10FFFF"]),
        unicode(&[]),
        unicode(&[" , "]),
        unicode(&["U+00FF-0000"]),
        unicode(&["U+110000"]),
        unicode(&["U+4?1"]),
        unicode(&["U+0000000"]),
        unicode(&["0041"]),
        unicode(&["U+GG"]),
        text("office ffi fi"),
        text("Aä€Ж日"),
        text(""),
        text(" "),
    ];
    for case in cases {
        let core = code_points(&case);
        let mock = fm_api::mock::subset_ranges(&case);
        match (core, mock) {
            (Ok(c), Ok((ranges, space_added))) => {
                assert_eq!(c.ranges(), ranges.as_slice(), "{case:?}");
                assert_eq!(c.space_added, space_added, "{case:?}");
            }
            (Err(c), Err(m)) => {
                assert_eq!(c.code(), m.code, "{case:?}");
                assert_eq!(c.message(), m.message, "{case:?}");
            }
            (c, m) => panic!("{case:?}: core {c:?}, mock {m:?}"),
        }
    }
}

#[test]
fn a_damaged_font_is_an_error_not_a_panic() {
    // `loca` without `glyf`: skera `expect`s the glyf table.
    let mut no_glyf = source(MULISH);
    no_glyf.tables.retain(|t| &t.tag != b"glyf");
    let e = cut(&no_glyf, &presets_keep(&["latin"])).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Unsupported, "{e:?}");
    assert!(e.message().contains("damaged"), "{e:?}");
}

#[test]
fn a_face_whose_license_forbids_subsetting_is_refused() {
    let mut src = source(MULISH);
    let os2 = src
        .tables
        .iter_mut()
        .find(|t| &t.tag == b"OS/2")
        .expect("OS/2");
    // fsType: installable, plus bit 8, no subsetting.
    os2.data[8..10].copy_from_slice(&0x0100_u16.to_be_bytes());
    let e = cut(&src, &presets_keep(&["latin"])).unwrap_err();
    assert_eq!(e.code(), ErrorCode::PermissionDenied, "{e:?}");
    assert!(e.message().contains("does not allow subsetting"), "{e:?}");
}
