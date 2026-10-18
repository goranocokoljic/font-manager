use std::path::PathBuf;
use std::time::{Duration, Instant};

use fm_api::common::{Health, Page};
use fm_api::library::SourceKind;
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

#[test]
fn filters_are_refused_until_they_are_built() {
    let (index, _) = indexed(&library());
    let mut req = SearchRequest::default();
    req.filters.variable = Some(true);
    let err = index.search(&req).expect_err("filters");
    assert_eq!(err.code(), ErrorCode::Unsupported);
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
             INSERT INTO face (file_id, family, subfamily, postscript_name, weight, width, slant)
             VALUES (1, 'Old Style', 'Regular', 'OldStyle-Regular', 400, 5, 'upright');",
        )
        .expect("old rows");
    }
    let index = Index::open(&path).expect("open").index;
    assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    let res = search(&index, "old style");
    assert_eq!(names(&res), ["Old Style"]);
    assert!(!res.families[0].family.added.is_empty());
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

/// `n` families of five styles each, with made-up distinct names.
fn synthetic(families: usize) -> Vec<IndexedFile> {
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
}

/// Criterion 1 without the reference library: 10,000 faces in 2,000
/// families. Timed in release builds only; a debug build runs the queries
/// for correctness.
#[test]
fn ten_thousand_fonts_search_in_under_50_ms() {
    let files = synthetic(2_000);
    assert_eq!(files.len(), 10_000);
    let (index, _) = indexed(&files);
    let res = search(&index, "kamiten sans");
    assert!(res.total >= 1);
    if cfg!(debug_assertions) {
        eprintln!("debug build: timing skipped");
        return;
    }
    time_queries(&index, "synthetic 10k");
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
}
