use std::path::PathBuf;
use std::time::{Duration, Instant};

use fm_api::common::{Classification, Embedding, Health, Page};
use fm_api::library::SourceKind;
use fm_api::search::{FilterCount, FilterCounts, Filters, Supports, WeightRange, WidthRange};
use fm_api::{ErrorCode, SourceId};
use rusqlite::Connection;

use super::*;
use crate::index::{INDEX_FILE, SCHEMA_VERSION};
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::{FileHash, IndexedFile};
use crate::test_support::TempDir;

/// The search budget of the spec (criterion 1).
const BUDGET: Duration = Duration::from_millis(50);

struct Face<'a> {
    family: &'a str,
    style: &'a str,
    weight: u16,
    width: u16,
    italic: bool,
}

const fn f<'a>(family: &'a str, style: &'a str, weight: u16, width: u16, italic: bool) -> Face<'a> {
    Face {
        family,
        style,
        weight,
        width,
        italic,
    }
}

fn parsed(face: &Face<'_>, foundry: Option<&str>, designer: Option<&str>) -> ParsedFace {
    let mut p = ParsedFace::empty(0, Health::Ok);
    p.family = Some(face.family.to_owned());
    p.subfamily = Some(face.style.to_owned());
    p.postscript_name = Some(format!(
        "{}-{}",
        face.family.replace(' ', ""),
        face.style.replace(' ', "")
    ));
    p.weight = Some(face.weight);
    p.width = Some(face.width);
    p.italic_angle = Some(if face.italic { -12.0 } else { 0.0 });
    p.foundry = foundry.map(str::to_owned);
    p.designer = designer.map(str::to_owned);
    p
}

fn file(path: &str, faces: Vec<ParsedFace>) -> IndexedFile {
    IndexedFile {
        path: PathBuf::from(path),
        size: 1000,
        modified: None,
        hash: FileHash::from([7; 32]),
        parsed: ParsedFile {
            format: Some(fm_api::common::FontFormat::Ttf),
            health: Health::Ok,
            faces,
        },
        gone: false,
    }
}

fn single(face: &Face<'_>, foundry: Option<&str>, designer: Option<&str>) -> IndexedFile {
    let name = parsed(face, foundry, designer)
        .postscript_name
        .unwrap_or_default();
    file(
        &format!("/fonts/{name}.ttf"),
        vec![parsed(face, foundry, designer)],
    )
}

/// A small library: Roboto (five styles), Roboto Condensed, Helvetica Neue,
/// a variable Inter, Black Ops One and Source Sans under an odd file name.
fn library() -> Vec<IndexedFile> {
    let google = Some("Google");
    let robertson = Some("Christian Robertson");
    let mut files: Vec<IndexedFile> = [
        f("Roboto", "Regular", 400, 5, false),
        f("Roboto", "Italic", 400, 5, true),
        f("Roboto", "Light", 300, 5, false),
        f("Roboto", "Bold", 700, 5, false),
        f("Roboto", "Bold Italic", 700, 5, true),
        f("Roboto Condensed", "Regular", 400, 3, false),
        f("Roboto Condensed", "Bold", 700, 3, false),
    ]
    .iter()
    .map(|face| single(face, google, robertson))
    .collect();
    for face in [
        f("Helvetica Neue", "Regular", 400, 5, false),
        f("Helvetica Neue", "Bold", 700, 5, false),
    ] {
        files.push(single(&face, Some("Linotype"), Some("Max Miedinger")));
    }
    files.push(single(
        &f("Black Ops One", "Regular", 400, 5, false),
        None,
        None,
    ));
    let mut inter = parsed(&f("Inter", "Regular", 400, 5, false), Some("Rasmus"), None);
    inter.axes = vec![
        fm_api::common::Axis {
            tag: "wght".into(),
            name: "Weight".into(),
            min: 100.0,
            default: 400.0,
            max: 900.0,
        },
        fm_api::common::Axis {
            tag: "slnt".into(),
            name: "Slant".into(),
            min: -10.0,
            default: 0.0,
            max: 0.0,
        },
    ];
    files.push(file("/fonts/Inter[slnt,wght].ttf", vec![inter]));
    files.push(file(
        "/fonts/ssp-regular.otf",
        vec![parsed(
            &f("Source Sans", "Regular", 400, 5, false),
            Some("Adobe"),
            None,
        )],
    ));
    files
}

fn indexed(files: &[IndexedFile]) -> (Index, SourceId) {
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files).expect("persist");
    (index, source)
}

fn search(index: &Index, query: &str) -> SearchResponse {
    index
        .search(&SearchRequest {
            query: query.to_owned(),
            ..SearchRequest::default()
        })
        .expect("search")
}

fn names(res: &SearchResponse) -> Vec<&str> {
    res.families
        .iter()
        .map(|h| h.family.name.as_str())
        .collect()
}

/// The style names of the faces of the family `family` in the result.
fn styles(index: &Index, res: &SearchResponse, family: &str) -> Vec<String> {
    let hit = res
        .families
        .iter()
        .find(|h| h.family.name == family)
        .expect("family in results");
    hit.faces
        .iter()
        .map(|id| {
            index
                .conn
                .query_row("SELECT subfamily FROM face WHERE id = ?1", [id.0], |r| {
                    r.get(0)
                })
                .expect("face")
        })
        .collect()
}

fn face_id(index: &Index, postscript: &str) -> i64 {
    index
        .conn
        .query_row(
            "SELECT id FROM face WHERE postscript_name = ?1",
            [postscript],
            |r| r.get(0),
        )
        .expect("face id")
}

#[test]
fn every_searchable_field_finds_its_family() {
    let (index, _) = indexed(&library());
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO tag (id, name) VALUES (1, 'Headline');
             INSERT INTO face_tag VALUES ({}, 1);",
            face_id(&index, "SourceSans-Regular")
        ))
        .expect("tag");
    for (query, family) in [
        ("helvetica", "Helvetica Neue"),          // family
        ("robertson", "Roboto"),                  // designer
        ("linotype", "Helvetica Neue"),           // foundry
        ("HelveticaNeue-Bold", "Helvetica Neue"), // PostScript name
        ("headline", "Source Sans"),              // tag
        ("ssp", "Source Sans"),                   // file name
        ("neue", "Helvetica Neue"),               // a word inside the family
    ] {
        let res = search(&index, query);
        assert_eq!(
            names(&res).first(),
            Some(&family),
            "{query}: {:?}",
            names(&res)
        );
    }
    assert_eq!(
        names(&search(&index, "nothing-like-this")),
        Vec::<&str>::new()
    );
}

#[test]
fn results_are_ranked_and_grouped_by_family() {
    let (index, _) = indexed(&library());
    let res = search(&index, "roboto");
    assert_eq!(names(&res), ["Roboto", "Roboto Condensed"]);
    assert_eq!(res.total, 2);
    let roboto = &res.families[0];
    assert_eq!(roboto.faces.len(), 5);
    assert!(roboto.score > res.families[1].score);
    assert_eq!(roboto.family.style_count, 5);
    assert_eq!(roboto.family.foundry.as_deref(), Some("Google"));
    assert_eq!(roboto.family.formats, [fm_api::common::FontFormat::Ttf]);
    assert_eq!(roboto.family.activation, FamilyActivation::Inactive);
    assert!(!roboto.family.added.is_empty());
    assert_eq!(
        roboto.family.id,
        FamilyId(face_id(&index, "Roboto-Regular") as u32),
        "the family id is its first face's id"
    );
    // Faces of a family come in style order when they score the same.
    assert_eq!(
        styles(&index, &res, "Roboto"),
        ["Light", "Regular", "Italic", "Bold", "Bold Italic"]
    );
}

#[test]
fn a_typo_still_finds_the_family() {
    let (index, _) = indexed(&library());
    for (typo, family) in [
        ("robtoo", "Roboto"),
        ("helvetika", "Helvetica Neue"),
        ("hevletica neue", "Helvetica Neue"),
        // Shares no trigram with "inter": found by the fallback over all faces.
        ("inetr", "Inter"),
        ("linotpye", "Helvetica Neue"),
    ] {
        let res = search(&index, typo);
        assert_eq!(
            names(&res).first(),
            Some(&family),
            "{typo}: {:?}",
            names(&res)
        );
    }
    // An exact match outranks a typo match.
    let res = search(&index, "roboto");
    assert!(res.families[0].score > search(&index, "robtoo").families[0].score);
}

#[test]
fn style_words_narrow_styles() {
    let (index, _) = indexed(&library());
    let res = search(&index, "roboto bold");
    assert_eq!(names(&res), ["Roboto", "Roboto Condensed"]);
    assert_eq!(styles(&index, &res, "Roboto"), ["Bold", "Bold Italic"]);
    assert_eq!(styles(&index, &res, "Roboto Condensed"), ["Bold"]);
    // The family row still describes the whole family.
    assert_eq!(res.families[0].family.style_count, 5);

    let res = search(&index, "roboto bold italic");
    assert_eq!(names(&res), ["Roboto"]);
    assert_eq!(styles(&index, &res, "Roboto"), ["Bold Italic"]);

    let res = search(&index, "Roboto Regular");
    assert_eq!(styles(&index, &res, "Roboto"), ["Regular"]);

    let res = search(&index, "roboto condensed");
    assert_eq!(names(&res), ["Roboto Condensed"]);

    // A variable face qualifies when its axes reach the style.
    assert_eq!(names(&search(&index, "inter black italic")), ["Inter"]);
    // A style word that is part of a family name finds the family.
    assert_eq!(names(&search(&index, "black ops")), ["Black Ops One"]);
    // Only style words: every family with that style.
    assert_eq!(
        names(&search(&index, "light")),
        ["Inter", "Roboto"],
        "Light faces only"
    );
}

#[test]
fn an_empty_query_lists_every_family_by_name() {
    let (index, _) = indexed(&library());
    let res = search(&index, "  ");
    assert_eq!(
        names(&res),
        [
            "Black Ops One",
            "Helvetica Neue",
            "Inter",
            "Roboto",
            "Roboto Condensed",
            "Source Sans"
        ]
    );
}

#[test]
fn pages_window_the_families() {
    let (index, _) = indexed(&library());
    let res = index
        .search(&SearchRequest {
            query: "roboto".into(),
            page: Page {
                offset: 1,
                limit: 1,
            },
            ..SearchRequest::default()
        })
        .expect("search");
    assert_eq!(names(&res), ["Roboto Condensed"]);
    assert_eq!(res.total, 2);
}

#[test]
fn gone_files_and_paused_sources_are_not_searched() {
    let (mut index, source) = indexed(&library());
    index
        .conn
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path LIKE '%HelveticaNeue%'",
            [],
        )
        .expect("gone");
    assert!(search(&index, "helvetica").families.is_empty());
    index.set_source_paused(source, true).expect("pause");
    assert!(search(&index, "roboto").families.is_empty());
}

#[test]
fn search_follows_tags_writes_and_deletes() {
    let (mut index, source) = indexed(&library());
    let inter = face_id(&index, "Inter-Regular");
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO tag (id, name) VALUES (1, 'screen');
             INSERT INTO face_tag VALUES ({inter}, 1);"
        ))
        .expect("tag");
    assert_eq!(names(&search(&index, "screen")), ["Inter"]);
    index
        .conn
        .execute("UPDATE tag SET name = 'interface' WHERE id = 1", [])
        .expect("rename");
    assert!(search(&index, "screen").families.is_empty());
    assert_eq!(names(&search(&index, "interface")), ["Inter"]);
    index
        .conn
        .execute("DELETE FROM face_tag", [])
        .expect("untag");
    let res = search(&index, "interface");
    assert!(res.families.is_empty(), "{:?}", names(&res));

    // A file written again with a new family name is found under it.
    let mut renamed = single(&f("Helvetica Neue", "Bold", 700, 5, false), None, None);
    renamed.parsed.faces[0].family = Some("Nimbus Sans".into());
    index.persist_files(source, [&renamed]).expect("rewrite");
    assert_eq!(names(&search(&index, "nimbus")), ["Nimbus Sans"]);
    assert_eq!(
        styles(&index, &search(&index, "helvetica"), "Helvetica Neue"),
        ["Regular"]
    );

    // A face the file no longer has leaves the search.
    let mut emptied = renamed.clone();
    emptied.parsed.faces.clear();
    index.persist_files(source, [&emptied]).expect("empty");
    assert!(search(&index, "nimbus").families.is_empty());
}

#[test]
fn the_family_row_shows_activation() {
    let (index, _) = indexed(&library());
    index
        .conn
        .execute(
            "INSERT INTO activation VALUES (?1, 'session', NULL, '2026-10-18T10:00:00Z')",
            [face_id(&index, "Roboto-Bold")],
        )
        .expect("activate");
    let res = search(&index, "roboto");
    assert_eq!(res.families[0].family.activation, FamilyActivation::Partial);
    let res = search(&index, "inter");
    assert_eq!(
        res.families[0].family.activation,
        FamilyActivation::Inactive
    );
    assert!(res.families[0].family.is_variable);
}

/// `library()` with something for every filter: Roboto Regular has `smcp`,
/// Black Ops One is color, Source Sans is an OTF, a second source holds a
/// broken Lora; Helvetica Neue Bold is declared a web font, favorite, five
/// stars and tagged; Roboto Bold is active.
fn filter_library() -> (Index, SourceId, SourceId) {
    let mut files = library();
    for file in &mut files {
        let face = &mut file.parsed.faces[0];
        match face.postscript_name.as_deref() {
            Some("Roboto-Regular") => face.features = vec!["kern".into(), "smcp".into()],
            Some("Roboto-Bold") => face.features = vec!["kern".into()],
            Some("BlackOpsOne-Regular") => face.color = true,
            Some("SourceSans-Regular") => file.parsed.format = Some(FontFormat::Otf),
            _ => {}
        }
    }
    let (mut index, first) = indexed(&files);
    let second = index
        .add_source("/more", SourceKind::Watched)
        .expect("source")
        .id;
    let mut lora = parsed(&f("Lora", "Regular", 400, 5, false), None, None);
    lora.health = Health::Broken("The cmap table is cut off.".into());
    index
        .persist_files(second, &[file("/more/Lora-Regular.ttf", vec![lora])])
        .expect("persist");
    let bold = face_id(&index, "HelveticaNeue-Bold");
    index
        .conn
        .execute_batch(&format!(
            "INSERT INTO user_meta (face_id, license_type, favorite, rating)
             VALUES ({bold}, 'web', 1, 5);
             INSERT INTO tag (id, name) VALUES (1, 'grotesk');
             INSERT INTO face_tag VALUES ({bold}, 1);
             INSERT INTO collection (id, name) VALUES (1, 'Picks');
             INSERT INTO collection_face VALUES (1, {bold});
             INSERT INTO collection (id, name, is_smart, rules) VALUES (2, 'Smart', 1, '{{}}');
             INSERT INTO activation VALUES ({}, 'session', NULL, '2026-10-18T10:00:00Z');",
            face_id(&index, "Roboto-Bold")
        ))
        .expect("user data");
    (index, first, second)
}

fn filtered(index: &Index, query: &str, filters: Filters) -> SearchResponse {
    index
        .search(&SearchRequest {
            query: query.to_owned(),
            filters,
            ..SearchRequest::default()
        })
        .expect("search")
}

/// Family names, each with its matching styles.
fn found(index: &Index, res: &SearchResponse) -> Vec<(String, Vec<String>)> {
    names(res)
        .iter()
        .map(|n| ((*n).to_owned(), styles(index, res, n)))
        .collect()
}

fn one(family: &str, styles: &[&str]) -> Vec<(String, Vec<String>)> {
    vec![(
        family.to_owned(),
        styles.iter().map(|s| (*s).to_owned()).collect(),
    )]
}

#[test]
fn each_filter_narrows_the_results() {
    let (index, first, second) = filter_library();
    let only = |set: fn(&mut Filters)| {
        let mut filters = Filters::default();
        set(&mut filters);
        found(&index, &filtered(&index, "", filters))
    };
    assert_eq!(
        only(|f| f.variable = Some(true)),
        one("Inter", &["Regular"])
    );
    assert_eq!(
        only(|f| f.color = Some(true)),
        one("Black Ops One", &["Regular"])
    );
    assert_eq!(
        only(|f| f.formats = vec![FontFormat::Otf]),
        one("Source Sans", &["Regular"])
    );
    assert_eq!(
        only(|f| f.width = Some(WidthRange { min: 1, max: 4 })),
        one("Roboto Condensed", &["Regular", "Bold"])
    );
    assert_eq!(
        only(|f| f.features = vec!["smcp".into()]),
        one("Roboto", &["Regular"])
    );
    assert_eq!(
        only(|f| f.license = Some(LicenseType::Web)),
        one("Helvetica Neue", &["Bold"])
    );
    assert_eq!(only(|f| f.problems = Some(true)), one("Lora", &["Regular"]));
    assert_eq!(only(|f| f.active = Some(true)), one("Roboto", &["Bold"]));
    assert_eq!(
        only(|f| f.slant = Some(Slant::Italic)),
        one("Roboto", &["Italic", "Bold Italic"])
    );
    // A variable face passes the ranges its axes reach.
    let heavy = only(|f| f.weight = Some(WeightRange { min: 800, max: 900 }));
    assert_eq!(heavy, one("Inter", &["Regular"]));
    let res = filtered(
        &index,
        "",
        Filters {
            sources: vec![second],
            ..Filters::default()
        },
    );
    assert_eq!(names(&res), ["Lora"]);
    let res = filtered(
        &index,
        "",
        Filters {
            sources: vec![first],
            problems: Some(false),
            ..Filters::default()
        },
    );
    assert_eq!(res.total, 6);
    // Faces are not classified until E3.5: a class filter finds nothing.
    let res = filtered(
        &index,
        "",
        Filters {
            classification: vec![Classification::Sans],
            ..Filters::default()
        },
    );
    assert_eq!(res.total, 0);
    assert!(res.counts.expect("counts").classification.is_empty());
}

#[test]
fn filters_compose_with_and_and_apply_with_search() {
    let (index, _, _) = filter_library();
    let italic = Filters {
        slant: Some(Slant::Italic),
        ..Filters::default()
    };
    assert_eq!(
        found(&index, &filtered(&index, "roboto", italic.clone())),
        one("Roboto", &["Italic", "Bold Italic"])
    );
    let bold_italic = Filters {
        weight: Some(WeightRange { min: 600, max: 900 }),
        ..italic.clone()
    };
    assert_eq!(
        found(&index, &filtered(&index, "roboto", bold_italic)),
        one("Roboto", &["Bold Italic"])
    );
    // Active is Roboto Bold, upright: with italic, nothing is left.
    let active_italic = Filters {
        active: Some(true),
        ..italic
    };
    assert_eq!(filtered(&index, "roboto", active_italic).total, 0);
    // The filters and the text must both hold.
    let smcp = Filters {
        features: vec!["smcp".into(), "kern".into()],
        weight: Some(WeightRange { min: 300, max: 500 }),
        ..Filters::default()
    };
    assert_eq!(
        found(&index, &filtered(&index, "roboto", smcp.clone())),
        one("Roboto", &["Regular"])
    );
    assert_eq!(filtered(&index, "helvetica", smcp).total, 0);
    // Style words of the query and filters narrow together.
    let upright = Filters {
        slant: Some(Slant::Upright),
        ..Filters::default()
    };
    assert_eq!(
        found(&index, &filtered(&index, "roboto bold", upright)),
        vec![
            ("Roboto".to_owned(), vec!["Bold".to_owned()]),
            ("Roboto Condensed".to_owned(), vec!["Bold".to_owned()]),
        ]
    );
    // The family row still describes the whole family.
    let res = filtered(
        &index,
        "",
        Filters {
            active: Some(true),
            ..Filters::default()
        },
    );
    assert_eq!(res.families[0].family.style_count, 5);
}

#[test]
fn tags_collections_foundry_favorites_and_rating_filter_too() {
    let (index, _, _) = filter_library();
    let helvetica = one("Helvetica Neue", &["Bold"]);
    for filters in [
        Filters {
            tags: vec![TagId(1)],
            ..Filters::default()
        },
        Filters {
            collection: Some(CollectionId(1)),
            ..Filters::default()
        },
        Filters {
            favorite: Some(true),
            ..Filters::default()
        },
        Filters {
            min_rating: Some(4),
            ..Filters::default()
        },
    ] {
        assert_eq!(
            found(&index, &filtered(&index, "", filters.clone())),
            helvetica,
            "{filters:?}"
        );
    }
    let res = filtered(
        &index,
        "",
        Filters {
            foundry: Some("Linotype".into()),
            ..Filters::default()
        },
    );
    assert_eq!(names(&res), ["Helvetica Neue"]);
}

#[test]
fn a_smart_collection_applies_and_an_unknown_one_is_refused() {
    let (mut index, _, _) = filter_library();
    // Collection 2 is smart with an empty rule (phase-1 filters `{}`): every
    // face, where it used to be `Unsupported` (E14.2 criterion 2).
    let every = filtered(&index, "", Filters::default()).total;
    let smart = |id: u32| Filters {
        collection: Some(CollectionId(id)),
        ..Filters::default()
    };
    assert_eq!(filtered(&index, "", smart(2)).total, every);
    let web = index
        .create_collection(&fm_api::organization::CreateCollectionRequest {
            name: "Web or kern".into(),
            parent: None,
            rules: None,
            rule_group: Some(fm_api::organization::RuleGroup {
                op: fm_api::organization::RuleOp::Or,
                rules: vec![
                    fm_api::organization::Rule::Filters {
                        filters: Filters {
                            license: Some(LicenseType::Web),
                            ..Filters::default()
                        },
                    },
                    fm_api::organization::Rule::Filters {
                        filters: Filters {
                            features: vec!["kern".into()],
                            ..Filters::default()
                        },
                    },
                ],
            }),
        })
        .expect("smart");
    let res = filtered(&index, "", smart(web.id.0));
    assert_eq!(
        found(&index, &res),
        vec![
            ("Helvetica Neue".to_owned(), vec!["Bold".to_owned()]),
            (
                "Roboto".to_owned(),
                vec!["Regular".to_owned(), "Bold".to_owned()]
            ),
        ]
    );
    // With text and the other filters, as any collection.
    let res = filtered(
        &index,
        "roboto",
        Filters {
            collection: Some(web.id),
            weight: Some(WeightRange { min: 600, max: 900 }),
            ..Filters::default()
        },
    );
    assert_eq!(found(&index, &res), one("Roboto", &["Bold"]));
    // The counts of the other filters keep the collection.
    let counts = res.counts.expect("counts");
    assert!(counts.weight.iter().all(|c| c.count <= 1), "{counts:?}");
    let refused = |filters: Filters| {
        index
            .search(&SearchRequest {
                filters,
                ..SearchRequest::default()
            })
            .expect_err("refused")
            .code()
    };
    assert_eq!(
        refused(Filters {
            collection: Some(CollectionId(99)),
            ..Filters::default()
        }),
        ErrorCode::NotFound
    );
}

/// Every counted value, as the filters that choose it on top of `base`.
fn chosen(counts: &FilterCounts, base: &Filters) -> Vec<(String, u32, Filters)> {
    let mut out = Vec::new();
    let mut push = |label: String, count: u32, set: &dyn Fn(&mut Filters)| {
        let mut filters = base.clone();
        set(&mut filters);
        out.push((label, count, filters));
    };
    for c in &counts.weight {
        let class = c.value;
        push(format!("weight {class}"), c.count, &|f| {
            f.weight = Some(WeightRange {
                min: if class <= 100 { 1 } else { class - 50 },
                max: if class >= 900 { 1000 } else { class + 49 },
            });
        });
    }
    for c in &counts.width {
        let class = c.value;
        push(format!("width {class}"), c.count, &|f| {
            f.width = Some(WidthRange {
                min: class,
                max: class,
            });
        });
    }
    for c in &counts.slant {
        let v = c.value;
        push(format!("{v:?}"), c.count, &|f| f.slant = Some(v));
    }
    for c in &counts.variable {
        let v = c.value;
        push(format!("variable {v}"), c.count, &|f| f.variable = Some(v));
    }
    for c in &counts.color {
        let v = c.value;
        push(format!("color {v}"), c.count, &|f| f.color = Some(v));
    }
    for c in &counts.formats {
        let v = c.value;
        push(format!("{v:?}"), c.count, &|f| f.formats = vec![v]);
    }
    for c in &counts.features {
        let v = c.value.clone();
        push(format!("feature {v}"), c.count, &|f| {
            f.features.push(v.clone())
        });
    }
    for c in &counts.license {
        let v = c.value;
        push(format!("{v:?}"), c.count, &|f| f.license = Some(v));
    }
    for c in &counts.sources {
        let v = c.value;
        push(format!("source {v}"), c.count, &|f| f.sources = vec![v]);
    }
    for c in &counts.problems {
        let v = c.value;
        push(format!("problems {v}"), c.count, &|f| f.problems = Some(v));
    }
    for c in &counts.active {
        let v = c.value;
        push(format!("active {v}"), c.count, &|f| f.active = Some(v));
    }
    out
}

#[test]
fn each_filter_value_counts_its_results() {
    let (index, _, second) = filter_library();
    let res = filtered(&index, "", Filters::default());
    let counts = res.counts.expect("counts");
    let count = |list: &[FilterCount<bool>], v: bool| {
        list.iter().find(|c| c.value == v).map_or(0, |c| c.count)
    };
    assert_eq!(count(&counts.variable, true), 1);
    assert_eq!(count(&counts.variable, false), 6);
    assert_eq!(count(&counts.color, true), 1);
    assert_eq!(count(&counts.problems, true), 1);
    assert_eq!(count(&counts.active, true), 1);
    assert_eq!(
        counts
            .sources
            .iter()
            .find(|c| c.value == second)
            .map(|c| c.count),
        Some(1)
    );
    assert_eq!(
        counts.features,
        [
            FilterCount {
                value: "kern".to_owned(),
                count: 1
            },
            FilterCount {
                value: "smcp".to_owned(),
                count: 1
            }
        ]
    );
    // Every count is the total the search returns with that value chosen,
    // with no filter, with filters, and with a query.
    let cases = [
        ("", Filters::default()),
        (
            "",
            Filters {
                slant: Some(Slant::Upright),
                variable: Some(false),
                ..Filters::default()
            },
        ),
        (
            "roboto",
            Filters {
                weight: Some(WeightRange { min: 600, max: 900 }),
                ..Filters::default()
            },
        ),
        (
            "bold",
            Filters {
                features: vec!["kern".into()],
                ..Filters::default()
            },
        ),
    ];
    for (query, base) in cases {
        let counts = filtered(&index, query, base.clone())
            .counts
            .expect("counts");
        let chosen = chosen(&counts, &base);
        assert!(!chosen.is_empty(), "{query:?} {base:?}");
        for (label, count, filters) in chosen {
            let total = filtered(&index, query, filters).total;
            assert_eq!(count, total, "{query:?} {base:?}: {label}");
        }
    }
    // Counts follow the text: no variable family is named Roboto.
    let counts = filtered(&index, "roboto", Filters::default())
        .counts
        .expect("counts");
    assert_eq!(count(&counts.variable, true), 0);
    assert_eq!(count(&counts.variable, false), 2);
}

#[test]
fn odd_query_text_is_escaped() {
    let (index, _) = indexed(&library());
    for query in [
        "\"rob",
        "rob* OR",
        "NEAR(a b)",
        "-col:x",
        "'; DROP TABLE face; --",
        "ü",
    ] {
        index
            .search(&SearchRequest {
                query: query.into(),
                ..SearchRequest::default()
            })
            .expect(query);
    }
    assert_eq!(
        names(&search(&index, "\"roboto\"")).first(),
        Some(&"Roboto")
    );
}

#[test]
fn a_version_2_index_migrates_into_search() {
    let dir = TempDir::new("search-migrate");
    let path = dir.path().join(INDEX_FILE);
    {
        let conn = Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..2] {
            conn.execute_batch(sql).expect("v1, v2");
        }
        conn.execute_batch(
            "PRAGMA user_version = 2;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (id, source_id, path, hash, format, size, mtime, health)
             VALUES (1, 1, '/fonts/old.otf', NULL, 'otf', 10, 1700000000, '\"ok\"');
             INSERT INTO face (file_id, collection_index, family, subfamily, postscript_name,
                               weight, width, slant, parsed)
             VALUES (1, 0, 'Old Style', 'Regular', 'OldStyle-Regular', 400, 5, 'upright',
                     '{\"foundry\": \"Oldtype\", \"designer\": \"Ada Penn\"}'),
                    (1, 1, 'Bad Json', 'Regular', 'BadJson-Regular', 400, 5, 'upright',
                     'not json');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    let res = search(&index, "old style");
    assert_eq!(names(&res), ["Old Style"]);
    assert!(!res.families[0].family.added.is_empty());
    // Foundry and designer come from the stored JSON; damaged JSON is skipped.
    assert_eq!(res.families[0].family.foundry.as_deref(), Some("Oldtype"));
    assert_eq!(names(&search(&index, "penn")), ["Old Style"]);
    assert_eq!(names(&search(&index, "bad json")), ["Bad Json"]);
}

#[test]
fn a_version_3_index_migrates_into_the_filters() {
    let dir = TempDir::new("search-migrate-v3");
    let path = dir.path().join(INDEX_FILE);
    {
        let conn = Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..3] {
            conn.execute_batch(sql).expect("v1 to v3");
        }
        conn.execute_batch(
            "PRAGMA user_version = 3;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (id, source_id, path, hash, format, size, mtime, health)
             VALUES (1, 1, '/fonts/old.ttc', randomblob(32), 'ttc', 10, 1700000000,
                     '{\"kind\":\"ok\"}');
             INSERT INTO face (file_id, collection_index, family, subfamily, postscript_name,
                               weight, width, slant, parsed)
             VALUES (1, 0, 'Fine Old', 'Regular', 'FineOld-Regular', 400, 5, 'upright',
                     '{\"health\": {\"kind\": \"ok\"}}'),
                    (1, 1, 'Cracked Old', 'Regular', 'CrackedOld-Regular', 400, 5, 'upright',
                     '{\"health\": {\"kind\": \"broken\", \"reason\": \"cut off\"}}'),
                    (1, 2, 'Bad Json', 'Regular', 'BadJson-Regular', 400, 5, 'upright',
                     'not json');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    let only = |problems| {
        let res = filtered(
            &index,
            "",
            Filters {
                problems: Some(problems),
                ..Filters::default()
            },
        );
        names(&res)
            .iter()
            .map(|n| (*n).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(only(true), ["Cracked Old"]);
    assert_eq!(only(false), ["Bad Json", "Fine Old"]);
    // Color is unknown until the next scan reads the files again.
    let res = filtered(
        &index,
        "",
        Filters {
            color: Some(false),
            ..Filters::default()
        },
    );
    assert_eq!(res.total, 3);
    let files = index.load_files().expect("load");
    assert!(files.files().all(|f| f.modified.is_none()), "read again");
}

#[test]
fn another_process_writing_refreshes_the_search() {
    let dir = TempDir::new("search-other-process");
    let path = dir.path().join(INDEX_FILE);
    let mut index = Index::open(&path).expect("open").index;
    let source = index
        .add_source("/fonts", SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, &library()).expect("persist");
    assert_eq!(names(&search(&index, "helvetica")), ["Helvetica Neue"]);
    let other = Connection::open(&path).expect("second connection");
    other
        .execute(
            "UPDATE font_file SET gone = 1 WHERE path LIKE '%HelveticaNeue%'",
            [],
        )
        .expect("write");
    assert!(search(&index, "helvetica").families.is_empty());
}

/// Feature tags for synthetic faces: each has 10 to 29 of them.
const FEATURES: [&str; 30] = [
    "aalt", "c2sc", "calt", "case", "ccmp", "dlig", "dnom", "frac", "kern", "liga", "lnum", "locl",
    "mark", "mkmk", "numr", "onum", "ordn", "pnum", "salt", "sinf", "smcp", "ss01", "ss02", "ss03",
    "sups", "tnum", "zero", "subs", "swsh", "titl",
];

/// `n` families of five styles each, with made-up distinct names.
pub(super) fn synthetic(families: usize) -> Vec<IndexedFile> {
    const SYLLABLES: [&str; 16] = [
        "ka", "ro", "mi", "ten", "sa", "lu", "vor", "ne", "di", "ga", "pel", "zu", "fi", "ber",
        "ox", "an",
    ];
    let styles = [
        ("Regular", 400, false),
        ("Italic", 400, true),
        ("Light", 300, false),
        ("Bold", 700, false),
        ("Bold Italic", 700, true),
    ];
    let mut files = Vec::with_capacity(families * styles.len());
    for i in 0..families {
        let name: String = [i % 16, (i / 16) % 16, (i / 256) % 16]
            .iter()
            .map(|&s| SYLLABLES[s])
            .collect();
        let family = format!(
            "{}{} {}",
            name[..1].to_uppercase(),
            &name[1..],
            ["Sans", "Serif", "Mono", "Display"][i % 4]
        );
        for (style, weight, italic) in styles {
            let face = f(&family, style, weight, 5, italic);
            let mut file = single(
                &face,
                Some(["Foundry A", "Type Co"][i % 2]),
                Some("Designer"),
            );
            file.path = PathBuf::from(format!("/fonts/{i:05}/{style}.ttf"));
            file.parsed.faces[0].features = FEATURES[..10 + i % 20]
                .iter()
                .map(|t| (*t).to_owned())
                .collect();
            // Latin; every other family Latin Extended-A, every fourth Cyrillic.
            file.parsed.faces[0].code_points = (0x20..0x7F)
                .chain(0xA0..0x100)
                .chain((0x100..0x180).filter(|_| i % 2 == 0))
                .chain((0x400..0x460).filter(|_| i % 4 == 0))
                .collect();
            files.push(file);
        }
    }
    files
}

fn time_queries(index: &Index, label: &str) {
    // The first query after a write reads the rows again: reported, not
    // budgeted (it happens once per library change, not per keystroke).
    index
        .conn
        .execute("UPDATE source SET paused = 0", [])
        .expect("touch");
    let start = Instant::now();
    search(index, "sans");
    eprintln!(
        "{label}: first query after a write in {:?}",
        start.elapsed()
    );
    for query in [
        "sans",
        "kar",
        "roten bold",
        "kartne",
        "ka",
        "bold italic",
        "type co",
        "",
    ] {
        // Warm the statement cache and the page cache, as a running app has.
        search(index, query);
        let start = Instant::now();
        let res = search(index, query);
        let elapsed = start.elapsed();
        eprintln!("{label}: {query:?} → {} families in {elapsed:?}", res.total);
        assert!(elapsed < BUDGET, "{label}: {query:?} took {elapsed:?}");
    }
    let filters = [
        (
            "",
            Filters {
                slant: Some(Slant::Italic),
                ..Filters::default()
            },
        ),
        (
            "sans",
            Filters {
                weight: Some(WeightRange { min: 600, max: 900 }),
                variable: Some(false),
                features: vec!["smcp".into()],
                ..Filters::default()
            },
        ),
        (
            "bold",
            Filters {
                formats: vec![FontFormat::Ttf, FontFormat::Otf],
                problems: Some(false),
                active: Some(false),
                ..Filters::default()
            },
        ),
        // E3.8 criterion 3: filtering by language.
        ("", languages(&["pl"])),
        ("", languages(&["sr-Latn", "sr-Cyrl", "de"])),
        ("sans", text("Ђорђе Đorđe ŁĄ")),
    ];
    for (query, filters) in filters {
        filtered(index, query, filters.clone());
        let start = Instant::now();
        let res = filtered(index, query, filters.clone());
        let elapsed = start.elapsed();
        eprintln!(
            "{label}: {query:?} {filters:?} → {} families in {elapsed:?}",
            res.total
        );
        assert!(
            elapsed < BUDGET,
            "{label}: {query:?} {filters:?} took {elapsed:?}"
        );
    }
}

/// Twenty smart collections over every rule shape: AND/OR, nested groups,
/// each filter dimension, a rule through another smart collection.
fn twenty_smart_collections(index: &mut Index) -> Vec<CollectionId> {
    use fm_api::organization::{CreateCollectionRequest, Rule, RuleGroup, RuleOp};
    let leaf = |filters: Filters| Rule::Filters { filters };
    let shapes: Vec<RuleGroup> = vec![
        RuleGroup::from(Filters {
            slant: Some(Slant::Italic),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            weight: Some(WeightRange { min: 600, max: 900 }),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            added_within_days: Some(30),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            features: vec!["smcp".into(), "onum".into()],
            ..Filters::default()
        }),
        RuleGroup::from(languages(&["pl"])),
        RuleGroup::from(languages(&["sr-Latn", "sr-Cyrl", "de"])),
        RuleGroup::from(text("Ђорђе Đorđe ŁĄ")),
        RuleGroup::from(Filters {
            foundry: Some("Type Co".into()),
            variable: Some(false),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            active: Some(false),
            problems: Some(false),
            ..Filters::default()
        }),
        RuleGroup::from(Filters {
            formats: vec![FontFormat::Ttf, FontFormat::Otf],
            ..Filters::default()
        }),
        RuleGroup {
            op: RuleOp::Or,
            rules: vec![
                leaf(Filters {
                    slant: Some(Slant::Italic),
                    ..Filters::default()
                }),
                leaf(languages(&["ru"])),
            ],
        },
        RuleGroup {
            op: RuleOp::And,
            rules: vec![
                leaf(Filters {
                    weight: Some(WeightRange { min: 100, max: 400 }),
                    ..Filters::default()
                }),
                Rule::Group {
                    group: RuleGroup {
                        op: RuleOp::Or,
                        rules: vec![
                            leaf(Filters {
                                features: vec!["liga".into()],
                                ..Filters::default()
                            }),
                            leaf(Filters {
                                foundry: Some("Foundry A".into()),
                                ..Filters::default()
                            }),
                        ],
                    },
                },
            ],
        },
    ];
    let mut ids: Vec<CollectionId> = Vec::new();
    for i in 0..20 {
        let rule_group = match shapes.get(i) {
            Some(shape) => shape.clone(),
            // The rest go through earlier smart collections.
            None => RuleGroup {
                op: if i.is_multiple_of(2) {
                    RuleOp::And
                } else {
                    RuleOp::Or
                },
                rules: vec![
                    leaf(Filters {
                        collection: Some(ids[i - 8]),
                        ..Filters::default()
                    }),
                    leaf(Filters {
                        collection: Some(ids[i - 5]),
                        ..Filters::default()
                    }),
                ],
            },
        };
        let c = index
            .create_collection(&CreateCollectionRequest {
                name: format!("Smart {i}"),
                parent: None,
                rules: None,
                rule_group: Some(rule_group),
            })
            .expect("smart collection");
        ids.push(c.id);
    }
    ids
}

/// E14.2 criterion 5: a search filtered by each smart collection, and the
/// collection list with twenty of them, each in the budget.
fn time_smart_collections(index: &mut Index, label: &str) {
    let ids = twenty_smart_collections(index);
    index.collections().expect("warm");
    let start = Instant::now();
    let list = index.collections().expect("list");
    let elapsed = start.elapsed();
    eprintln!(
        "{label}: list_collections with {} smart in {elapsed:?}",
        list.len()
    );
    assert!(
        elapsed < BUDGET,
        "{label}: list_collections took {elapsed:?}"
    );
    for (query, id) in ids.iter().flat_map(|id| [("", *id), ("sans", *id)]) {
        let filters = Filters {
            collection: Some(id),
            ..Filters::default()
        };
        filtered(index, query, filters.clone());
        let start = Instant::now();
        let res = filtered(index, query, filters);
        let elapsed = start.elapsed();
        eprintln!(
            "{label}: {query:?} in smart {id} → {} families in {elapsed:?}",
            res.total
        );
        assert!(
            elapsed < BUDGET,
            "{label}: {query:?} in smart {id} took {elapsed:?}"
        );
    }
}

/// Criterion 1 without the reference library: 10,000 faces in 2,000
/// families. Timed in release builds only; a debug build runs the queries
/// for correctness.
#[test]
fn ten_thousand_fonts_search_in_under_50_ms() {
    let files = synthetic(2_000);
    assert_eq!(files.len(), 10_000);
    let (mut index, _) = indexed(&files);
    let res = search(&index, "kamiten sans");
    assert!(res.total >= 1);
    if cfg!(debug_assertions) {
        // The twenty rules still compile and count.
        twenty_smart_collections(&mut index);
        assert_eq!(index.collections().expect("list").len(), 20);
        eprintln!("debug build: timing skipped");
        return;
    }
    time_queries(&index, "synthetic 10k");
    time_smart_collections(&mut index, "synthetic 10k");
}

/// Criterion 1 on the reference library (10k fonts): skipped without it.
#[test]
fn the_reference_library_searches_in_under_50_ms() {
    let Some(library) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
        eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
        return;
    };
    let library = PathBuf::from(library);
    let mut scanned = crate::scanner::FileIndex::new();
    let report = scanned.rescan(
        &library,
        &crate::scanner::discover(&library).expect("discover"),
    );
    let mut index = Index::open_in_memory().expect("open");
    let source = index
        .add_source(&library.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index
        .apply_rescan(source, &scanned, &report)
        .expect("persist");
    time_queries(&index, "reference library");
    time_smart_collections(&mut index, "reference library");
}

/// The base characters of the embedded preset `code`.
fn base(code: &str) -> Vec<u32> {
    crate::charsets::load()
        .expect("presets")
        .into_iter()
        .find(|c| c.code == code)
        .map(|c| c.base.iter().map(|&ch| u32::from(ch)).collect())
        .expect("known preset")
}

fn languages(codes: &[&str]) -> Filters {
    Filters {
        supports: Some(Supports::Languages {
            codes: codes.iter().map(|&c| c.to_owned()).collect(),
        }),
        ..Filters::default()
    }
}

fn text(text: &str) -> Filters {
    Filters {
        supports: Some(Supports::Text { text: text.into() }),
        ..Filters::default()
    }
}

/// A face of `family` mapping `code_points`, with `locl` language systems.
fn covering(family: &str, style: &str, code_points: Vec<u32>, locl: &[&str]) -> IndexedFile {
    let weight = if style == "Bold" { 700 } else { 400 };
    let mut file = single(&f(family, style, weight, 5, false), None, None);
    let face = &mut file.parsed.faces[0];
    face.code_points = code_points;
    face.code_points.sort_unstable();
    face.code_points.dedup();
    face.locl_languages = locl.iter().map(|&l| l.to_owned()).collect();
    file
}

/// `code_points` without the characters of `drop`.
fn without(code_points: &[u32], drop: &str) -> Vec<u32> {
    code_points
        .iter()
        .copied()
        .filter(|&cp| !drop.chars().any(|c| u32::from(c) == cp))
        .collect()
}

/// Latin, Serbian Cyrillic and near misses:
/// - Full: English, Romanian, Serbian Latin and Cyrillic, `locl` for
///   Serbian and Romanian; its Bold lacks ж;
/// - Latin: English and Serbian Latin;
/// - Almost: English and Serbian Cyrillic but ђ and ћ, `locl` for Serbian;
/// - Five: English but five letters;
/// - Six: English but six letters.
fn coverage_library() -> Index {
    let en = base("en");
    let sr_latn = base("sr-Latn");
    let sr_cyrl = base("sr-Cyrl");
    let all: Vec<u32> = [en.clone(), sr_latn.clone(), sr_cyrl.clone(), base("ro")].concat();
    let files = vec![
        covering("Full", "Regular", all.clone(), &["cyrl/SRB", "latn/ROM"]),
        covering("Full", "Bold", without(&all, "ж"), &[]),
        covering("Latin", "Regular", [en.clone(), sr_latn].concat(), &[]),
        covering(
            "Almost",
            "Regular",
            [en.clone(), without(&sr_cyrl, "ђћ")].concat(),
            &["cyrl/SRB"],
        ),
        covering("Five", "Regular", without(&en, "vwxyz"), &[]),
        covering("Six", "Regular", without(&en, "uvwxyz"), &[]),
    ];
    indexed(&files).0
}

/// Family names with the characters their best face misses.
fn misses(res: &SearchResponse) -> Vec<(&str, String)> {
    res.families
        .iter()
        .map(|h| {
            let missing = h
                .missing
                .as_ref()
                .expect("a supports search reports missing");
            (h.family.name.as_str(), missing.chars.concat())
        })
        .collect()
}

fn full(name: &str) -> (&str, String) {
    (name, String::new())
}

#[test]
fn languages_filter_keeps_faces_that_cover_every_language() {
    let index = coverage_library();
    let res = filtered(&index, "", languages(&["sr-Latn"]));
    assert_eq!(misses(&res), [full("Full"), full("Latin")]);
    let res = filtered(&index, "", languages(&["sr-Latn", "sr-Cyrl"]));
    assert_eq!(misses(&res), [full("Full")], "AND: both scripts");
    assert_eq!(
        styles(&index, &res, "Full"),
        ["Regular", "Bold"],
        "the Bold lacks one letter: a near miss, after the Regular"
    );
    let res = filtered(&index, "", languages(&["sr-Cyrl"]));
    assert_eq!(misses(&res), [full("Full"), ("Almost", "ђћ".to_owned())]);
    let res = filtered(&index, "almost", languages(&["sr-Cyrl"]));
    assert_eq!(names(&res), ["Almost"], "text and coverage combine");
}

#[test]
fn pasted_text_filters_by_its_characters() {
    let index = coverage_library();
    let res = filtered(&index, "", text("Ђорђе ћути\n"));
    assert_eq!(
        misses(&res),
        [full("Full"), ("Almost", "ђћ".to_owned())],
        "every character once, in code point order; whitespace not asked for"
    );
    let res = filtered(&index, "", text("Đorđe ćuti"));
    assert_eq!(
        misses(&res),
        [
            full("Full"),
            full("Latin"),
            ("Almost", "ćĐđ".to_owned()),
            ("Five", "ćĐđ".to_owned()),
            ("Six", "ućĐđ".to_owned()),
        ],
        "fewest missing first"
    );
    assert!(
        res.families
            .iter()
            .all(|h| h.missing.as_ref().is_some_and(|m| !m.has_locl)),
        "text has no language, so no locl"
    );
    let res = filtered(&index, "", text("  \n"));
    assert_eq!(res.total, 5, "nothing asked for: every family");
    assert!(res.families.iter().all(|h| h.missing.is_none()));
}

#[test]
fn near_misses_list_the_exact_missing_characters() {
    let index = coverage_library();
    let res = filtered(&index, "", languages(&["en"]));
    assert_eq!(
        misses(&res),
        [
            full("Almost"),
            full("Full"),
            full("Latin"),
            ("Five", "vwxyz".to_owned()),
        ],
        "complete families first; six missing is not a result"
    );
    let res = filtered(&index, "", text("ж"));
    assert_eq!(
        styles(&index, &res, "Full"),
        ["Regular", "Bold"],
        "best face first"
    );
    assert!(misses(&res).contains(&full("Full")), "the Regular has ж");
    let res = filtered(&index, "", text("жђћ"));
    assert_eq!(
        misses(&res),
        [
            full("Full"),
            ("Almost", "ђћ".to_owned()),
            ("Five", "жђћ".to_owned()),
            ("Latin", "жђћ".to_owned()),
            ("Six", "жђћ".to_owned()),
        ],
        "a family reports its best face"
    );
}

#[test]
fn locl_is_reported_for_the_chosen_language() {
    let index = coverage_library();
    let locl = |filters: Filters| -> Vec<(String, bool)> {
        filtered(&index, "", filters)
            .families
            .iter()
            .map(|h| {
                (
                    h.family.name.clone(),
                    h.missing.as_ref().is_some_and(|m| m.has_locl),
                )
            })
            .collect()
    };
    assert_eq!(
        locl(languages(&["sr-Cyrl"])),
        [("Full".to_owned(), true), ("Almost".to_owned(), true)]
    );
    assert_eq!(
        locl(languages(&["sr-Latn"])),
        [("Full".to_owned(), false), ("Latin".to_owned(), false)],
        "Serbian locl under cyrl is not Serbian Latin"
    );
    let mixed = locl(languages(&["en", "ro"]));
    assert!(
        mixed.contains(&("Full".to_owned(), true)),
        "{mixed:?}: ROM under latn, for one of the languages"
    );
}

#[test]
fn faces_without_stored_coverage_do_not_pass() {
    let index = coverage_library();
    index
        .conn
        .execute(
            "UPDATE face SET cmap = NULL WHERE postscript_name = 'Latin-Regular'",
            [],
        )
        .expect("clear");
    let res = filtered(&index, "", languages(&["sr-Latn"]));
    assert_eq!(names(&res), ["Full"], "not computed: not proven");
    index
        .conn
        .execute(
            "UPDATE face SET cmap = x'0102' WHERE postscript_name = 'Full-Regular'",
            [],
        )
        .expect("damage");
    let res = filtered(&index, "", languages(&["sr-Latn"]));
    assert_eq!(
        styles(&index, &res, "Full"),
        ["Bold"],
        "damaged: not proven"
    );
}

#[test]
fn an_unknown_language_is_refused() {
    let index = coverage_library();
    let err = index
        .search(&SearchRequest {
            filters: languages(&["en", "xx-Nope"]),
            ..SearchRequest::default()
        })
        .expect_err("unknown preset");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().contains("xx-Nope"));
}

#[test]
fn counts_keep_the_supports_filter() {
    let index = coverage_library();
    let res = filtered(&index, "", languages(&["sr-Cyrl"]));
    let counts = res.counts.expect("counts");
    assert_eq!(
        counts.weight,
        [
            FilterCount {
                value: 400,
                count: 2
            },
            FilterCount {
                value: 700,
                count: 1
            }
        ],
        "Full and Almost pass; Latin, Five and Six are not counted"
    );
    let res = filtered(
        &index,
        "",
        Filters {
            weight: Some(WeightRange { min: 650, max: 750 }),
            ..languages(&["sr-Cyrl"])
        },
    );
    assert_eq!(names(&res), ["Full"]);
}

const OFL_TEXT: &str =
    "This Font Software is licensed under the SIL Open Font License, Version 1.1.";

/// Libre (OFL, installable), Apache Sans (Apache by its URL, editable),
/// Mention (only names the OFL, restricted) and Plain (no license text,
/// preview and print).
fn license_library() -> Vec<IndexedFile> {
    let face = |family: &str, license: Option<&str>, url: Option<&str>, fs_type: u16| {
        let mut p = parsed(&f(family, "Regular", 400, 5, false), None, None);
        p.license = license.map(str::to_owned);
        p.license_url = url.map(str::to_owned);
        p.fs_type = Some(fs_type);
        let name = p.postscript_name.clone().unwrap_or_default();
        file(&format!("/fonts/{name}.ttf"), vec![p])
    };
    vec![
        face("Libre", Some(OFL_TEXT), None, 0x0000),
        face(
            "Apache Sans",
            None,
            Some("http://www.apache.org/licenses/LICENSE-2.0"),
            0x0008,
        ),
        face(
            "Mention",
            Some("Unlike our SIL Open Font License fonts, this one is commercial."),
            Some("https://example.com/eula"),
            0x0002,
        ),
        face("Plain", None, None, 0x0004),
    ]
}

fn stored_open_license(index: &Index, postscript: &str) -> Option<String> {
    index
        .conn
        .query_row(
            "SELECT open_license FROM face WHERE postscript_name = ?1",
            [postscript],
            |r| r.get(0),
        )
        .expect("open license")
}

#[test]
fn open_licenses_are_stored_at_scan_and_shown_in_every_family_row() {
    let (index, _) = indexed(&license_library());
    assert_eq!(
        stored_open_license(&index, "Libre-Regular").as_deref(),
        Some("ofl")
    );
    assert_eq!(
        stored_open_license(&index, "ApacheSans-Regular").as_deref(),
        Some("apache")
    );
    assert_eq!(stored_open_license(&index, "Mention-Regular"), None);
    assert_eq!(stored_open_license(&index, "Plain-Regular"), None);

    let expected = vec![
        ("Apache Sans".to_owned(), Some(OpenLicense::Apache)),
        ("Libre".to_owned(), Some(OpenLicense::Ofl)),
        ("Mention".to_owned(), None),
        ("Plain".to_owned(), None),
    ];
    let res = search(&index, "");
    let searched: Vec<_> = res
        .families
        .iter()
        .map(|h| (h.family.name.clone(), h.family.open_license))
        .collect();
    assert_eq!(searched, expected, "search rows");
    let listed: Vec<_> = index
        .list_families(&ListFamiliesRequest::default())
        .expect("list")
        .items
        .into_iter()
        .map(|f| (f.name, f.open_license))
        .collect();
    assert_eq!(listed, expected, "list_families rows");
}

#[test]
fn face_detail_fills_embedding_and_open_license() {
    let (index, _) = indexed(&license_library());
    let detail = |ps: &str| {
        index
            .face_detail(FaceId(u32::try_from(face_id(&index, ps)).expect("id")))
            .expect("detail")
            .license
    };
    let libre = detail("Libre-Regular");
    assert_eq!(libre.open_license, Some(OpenLicense::Ofl));
    assert_eq!(libre.embedding, Embedding::Installable);
    assert_eq!(libre.description.as_deref(), Some(OFL_TEXT));
    let apache = detail("ApacheSans-Regular");
    assert_eq!(apache.open_license, Some(OpenLicense::Apache));
    assert_eq!(apache.embedding, Embedding::Editable);
    let mention = detail("Mention-Regular");
    assert_eq!(mention.open_license, None);
    assert_eq!(mention.embedding, Embedding::Restricted);
    assert_eq!(
        detail("Plain-Regular").embedding,
        Embedding::PreviewAndPrint
    );
}

#[test]
fn embedding_and_open_license_filter_and_count() {
    let (index, _) = indexed(&license_library());
    let only = |filters: Filters| names(&filtered(&index, "", filters)).join(", ");
    assert_eq!(
        only(Filters {
            open_license: Some(true),
            ..Filters::default()
        }),
        "Apache Sans, Libre"
    );
    assert_eq!(
        only(Filters {
            open_license: Some(false),
            ..Filters::default()
        }),
        "Mention, Plain"
    );
    assert_eq!(
        only(Filters {
            embedding: Some(Embedding::Restricted),
            ..Filters::default()
        }),
        "Mention"
    );
    assert_eq!(
        only(Filters {
            embedding: Some(Embedding::Installable),
            open_license: Some(true),
            ..Filters::default()
        }),
        "Libre"
    );

    let counts = filtered(&index, "", Filters::default())
        .counts
        .expect("counts");
    let open: Vec<_> = counts
        .open_license
        .iter()
        .map(|c| (c.value, c.count))
        .collect();
    assert_eq!(open, [(true, 2), (false, 2)]);
    let embedding: Vec<_> = counts
        .embedding
        .iter()
        .map(|c| (c.value, c.count))
        .collect();
    assert_eq!(
        embedding,
        [
            (Embedding::Installable, 1),
            (Embedding::Editable, 1),
            (Embedding::PreviewAndPrint, 1),
            (Embedding::Restricted, 1),
        ]
    );
}

#[test]
fn a_version_11_index_fills_open_licenses_from_the_stored_text() {
    let dir = TempDir::new("search-migrate-v11");
    let path = dir.path().join(INDEX_FILE);
    {
        let conn = Connection::open(&path).expect("open");
        for sql in &crate::index::schema::MIGRATIONS[..11] {
            conn.execute_batch(sql).expect("v1 to v11");
        }
        conn.execute_batch(
            "PRAGMA user_version = 11;
             INSERT INTO source (id, path, kind) VALUES (1, '/fonts', 'watched');
             INSERT INTO font_file (id, source_id, path, hash, format, size, mtime, health)
             VALUES (1, 1, '/fonts/old.ttc', randomblob(32), 'ttc', 10, 1700000000,
                     '{\"kind\":\"ok\"}');
             INSERT INTO face (file_id, collection_index, family, subfamily, postscript_name,
                               weight, width, slant, fstype, parsed)
             VALUES (1, 0, 'Old Libre', 'Regular', 'OldLibre-Regular', 400, 5, 'upright', 0,
                     '{\"license\": \"Licensed under the SIL Open Font License, Version 1.1\"}'),
                    (1, 1, 'Old Url', 'Regular', 'OldUrl-Regular', 400, 5, 'upright', 2,
                     '{\"licenseUrl\": \"http://scripts.sil.org/OFL\"}'),
                    (1, 2, 'Old Mention', 'Regular', 'OldMention-Regular', 400, 5, 'upright', 0,
                     '{\"license\": \"See the OFL FAQ.\"}'),
                    (1, 3, 'Bad Json', 'Regular', 'BadJson-Regular', 400, 5, 'upright', 0,
                     'not json');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    let res = search(&index, "");
    let rows: Vec<_> = res
        .families
        .iter()
        .map(|h| (h.family.name.as_str(), h.family.open_license))
        .collect();
    assert_eq!(
        rows,
        [
            ("Bad Json", None),
            ("Old Libre", Some(OpenLicense::Ofl)),
            ("Old Mention", None),
            ("Old Url", Some(OpenLicense::Ofl)),
        ]
    );
    assert_eq!(
        stored_open_license(&index, "OldLibre-Regular").as_deref(),
        Some("ofl")
    );
}

/// Declares `license_type` for the family of `postscript`.
fn declare(index: &mut Index, postscript: &str, license_type: LicenseType) {
    let face = FaceId(u32::try_from(face_id(index, postscript)).expect("id"));
    let family = index.family_of(face).expect("family").expect("visible").0;
    index
        .declare_license(
            family,
            &crate::index::DeclaredLicense {
                license_type: Some(license_type),
                seats: None,
                purchase_date: None,
                receipt: None,
            },
        )
        .expect("declare");
}

/// `license_library` with Mention (restricted) declared Web and Apache Sans
/// (open, editable) declared Desktop: OK for web are Libre (OFL) and
/// Mention (declared Web); Apache Sans (declared Desktop) and Plain
/// (preview and print) are not.
#[test]
fn web_ok_filters_counts_and_explains_each_face() {
    let (mut index, _) = indexed(&license_library());
    declare(&mut index, "Mention-Regular", LicenseType::Web);
    declare(&mut index, "ApacheSans-Regular", LicenseType::Desktop);
    let only = |web_ok| {
        names(&filtered(
            &index,
            "",
            Filters {
                web_ok: Some(web_ok),
                ..Filters::default()
            },
        ))
        .join(", ")
    };
    assert_eq!(only(true), "Libre, Mention");
    assert_eq!(only(false), "Apache Sans, Plain");

    let counts = filtered(&index, "", Filters::default())
        .counts
        .expect("counts");
    let web: Vec<_> = counts.web_ok.iter().map(|c| (c.value, c.count)).collect();
    assert_eq!(web, [(true, 2), (false, 2)]);

    let why = |ps: &str| {
        index
            .face_detail(FaceId(u32::try_from(face_id(&index, ps)).expect("id")))
            .expect("detail")
            .license
            .web_use
            .map(|w| (w.ok, w.reason))
    };
    assert_eq!(
        why("Libre-Regular"),
        Some((true, "Open license (OFL)".into()))
    );
    assert_eq!(
        why("Mention-Regular"),
        Some((true, "Declared web license".into()))
    );
    assert_eq!(
        why("ApacheSans-Regular"),
        Some((false, "Declared desktop license".into()))
    );
    assert_eq!(
        why("Plain-Regular"),
        Some((
            false,
            "Embedding preview & print only, no web license declared".into()
        ))
    );
}

/// E14.1 criterion 3: the counts list every foundry, A to Z, counted
/// without the foundry already chosen (the rule editor's foundry choices).
#[test]
fn foundry_counts_list_each_foundry_a_to_z() {
    let (index, _, _) = filter_library();
    let counts = filtered(&index, "", Filters::default())
        .counts
        .expect("counts")
        .foundry;
    let linotype = counts
        .iter()
        .find(|c| c.value == "Linotype")
        .expect("Linotype counted");
    assert_eq!(linotype.count, 1);
    let mut sorted = counts.clone();
    sorted.sort_by(|a, b| a.value.cmp(&b.value));
    assert_eq!(counts, sorted);
    let chosen = filtered(
        &index,
        "",
        Filters {
            foundry: Some("Linotype".into()),
            ..Filters::default()
        },
    );
    assert_eq!(chosen.counts.expect("counts").foundry, counts);
    let narrowed = filtered(
        &index,
        "",
        Filters {
            favorite: Some(true),
            ..Filters::default()
        },
    );
    let only: Vec<_> = narrowed
        .counts
        .expect("counts")
        .foundry
        .into_iter()
        .map(|c| (c.value, c.count))
        .collect();
    assert_eq!(only, [("Linotype".to_owned(), 1)]);
}
