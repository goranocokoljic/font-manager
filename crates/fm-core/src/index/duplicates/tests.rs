use std::path::PathBuf;

use fm_api::common::{FontFormat, Health};
use fm_api::library::SourceKind;

use super::*;
use crate::parser::{ParsedFace, ParsedFile};
use crate::scanner::IndexedFile;

/// A one-face file at `path` with the hash byte `hash`, PostScript name
/// `name` and version `version`; its size is unique to the path.
fn file(path: &str, hash: u8, name: &str, version: Option<&str>) -> IndexedFile {
    let mut face = ParsedFace::empty(0, Health::Ok);
    face.family = Some("Family".to_owned());
    face.subfamily = Some("Regular".to_owned());
    face.postscript_name = Some(name.to_owned());
    face.version = version.map(Into::into);
    face.weight = Some(400);
    face.width = Some(5);
    IndexedFile {
        path: PathBuf::from(path),
        size: 1000 + path.len() as u64,
        modified: None,
        hash: FileHash::from([hash; 32]),
        parsed: ParsedFile {
            format: Some(FontFormat::Ttf),
            health: Health::Ok,
            faces: vec![face],
        },
        gone: false,
    }
}

fn index() -> Index {
    Index::open_in_memory().expect("open")
}

fn add(index: &mut Index, folder: &str, kind: SourceKind, files: &[IndexedFile]) -> SourceId {
    let source = index.add_source(folder, kind).expect("source").id;
    index.persist_files(source, files).expect("persist");
    source
}

/// Each group as `kind name: path, path` with the keeper first.
fn summary(index: &Index) -> Vec<String> {
    index
        .duplicates()
        .expect("duplicates")
        .iter()
        .map(|g| {
            let paths: Vec<&str> = g.copies.iter().map(|c| c.path.as_str()).collect();
            format!(
                "{:?} {}: {}",
                g.kind,
                g.postscript_name.as_deref().unwrap_or("-"),
                paths.join(", ")
            )
        })
        .collect()
}

#[test]
fn an_empty_library_has_no_duplicates() {
    assert!(index().duplicates().expect("duplicates").is_empty());
}

#[test]
fn identical_files_at_different_paths_are_an_exact_group_with_paths_and_sizes() {
    let mut index = index();
    let source = add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file(
                "/fonts/b/Inter.ttf",
                1,
                "Inter-Regular",
                Some("Version 4.0"),
            ),
            file(
                "/fonts/a/Inter.ttf",
                1,
                "Inter-Regular",
                Some("Version 4.0"),
            ),
            file(
                "/fonts/Roboto.ttf",
                2,
                "Roboto-Regular",
                Some("Version 3.0"),
            ),
        ],
    );
    let groups = index.duplicates().expect("duplicates");
    assert_eq!(groups.len(), 1, "{groups:?}");
    let g = &groups[0];
    assert_eq!(g.kind, DuplicateKind::Exact);
    assert_eq!(g.postscript_name.as_deref(), Some("Inter-Regular"));
    let copies: Vec<(&str, u32, bool)> = g
        .copies
        .iter()
        .map(|c| (c.path.as_str(), c.size, c.keep))
        .collect();
    // Same version, neither in the library folder: the path decides.
    assert_eq!(
        copies,
        [
            ("/fonts/a/Inter.ttf", 1018, true),
            ("/fonts/b/Inter.ttf", 1018, false),
        ]
    );
    for c in &g.copies {
        assert_eq!(c.hash, FileHash::from([1; 32]).to_hex());
        assert_eq!(c.version.as_deref(), Some("Version 4.0"));
        assert_eq!(c.source, source);
        assert!(!c.in_library_folder);
        assert!(c.face.is_some());
    }
    assert_ne!(g.copies[0].face, g.copies[1].face);
}

#[test]
fn the_same_postscript_name_and_version_in_different_files_is_a_near_group() {
    let mut index = index();
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file("/fonts/Inter.otf", 1, "Inter-Regular", Some("Version 4.0")),
            file(
                "/fonts/web/Inter.ttf",
                2,
                "Inter-Regular",
                Some(" Version 4.0 "),
            ),
            // Another version of the same face is not a duplicate.
            file(
                "/fonts/old/Inter.ttf",
                3,
                "Inter-Regular",
                Some("Version 3.19"),
            ),
            // Names compare exactly.
            file("/fonts/inter.ttf", 4, "inter-regular", Some("Version 4.0")),
        ],
    );
    assert_eq!(
        summary(&index),
        ["Near Inter-Regular: /fonts/Inter.otf, /fonts/web/Inter.ttf"]
    );
    let groups = index.duplicates().expect("duplicates");
    assert_eq!(
        groups[0].copies[1].version.as_deref(),
        Some("Version 4.0"),
        "the version is trimmed"
    );
}

#[test]
fn a_missing_version_matches_only_another_missing_version() {
    let mut index = index();
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file("/fonts/a.ttf", 1, "Mono", None),
            file("/fonts/b.ttf", 2, "Mono", Some("")),
            file("/fonts/c.ttf", 3, "Mono", Some("1.0")),
        ],
    );
    // An empty version is no version; "1.0" stands alone.
    assert_eq!(summary(&index), ["Near Mono: /fonts/a.ttf, /fonts/b.ttf"]);
    let groups = index.duplicates().expect("duplicates");
    assert!(groups[0].copies.iter().all(|c| c.version.is_none()));
}

#[test]
fn identical_copies_inside_a_near_group_are_not_listed_twice() {
    let mut index = index();
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file("/fonts/a.ttf", 1, "Inter-Regular", Some("4.0")),
            file("/fonts/copy/a.ttf", 1, "Inter-Regular", Some("4.0")),
            file("/fonts/b.ttf", 2, "Inter-Regular", Some("4.0")),
        ],
    );
    assert_eq!(
        summary(&index),
        ["Near Inter-Regular: /fonts/a.ttf, /fonts/b.ttf, /fonts/copy/a.ttf"]
    );
    let groups = index.duplicates().expect("duplicates");
    let hashes: Vec<&str> = groups[0].copies.iter().map(|c| c.hash.as_str()).collect();
    assert_eq!(
        hashes[0], hashes[2],
        "the identical copies show the same hash"
    );
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn the_copy_in_the_library_folder_is_the_keeper() {
    let mut index = index();
    add(
        &mut index,
        "/a-downloads",
        SourceKind::Watched,
        &[file(
            "/a-downloads/Inter.ttf",
            1,
            "Inter-Regular",
            Some("4.0"),
        )],
    );
    let library = add(
        &mut index,
        "/z-library",
        SourceKind::LibraryFolder,
        &[file(
            "/z-library/Inter.ttf",
            1,
            "Inter-Regular",
            Some("4.0"),
        )],
    );
    let groups = index.duplicates().expect("duplicates");
    assert_eq!(groups.len(), 1);
    let keeper = &groups[0].copies[0];
    assert_eq!(keeper.path, "/z-library/Inter.ttf");
    assert_eq!(keeper.source, library);
    assert!(keeper.keep && keeper.in_library_folder);
    assert!(!groups[0].copies[1].keep && !groups[0].copies[1].in_library_folder);
}

#[test]
fn the_newest_version_is_the_keeper_of_an_exact_group() {
    // An identical file read twice gives the same version, so write the
    // second version straight into the row: the rule is the contract's.
    let mut index = index();
    add(
        &mut index,
        "/lib",
        SourceKind::LibraryFolder,
        &[file(
            "/lib/Inter.ttf",
            1,
            "Inter-Regular",
            Some("Version 2.9"),
        )],
    );
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[file(
            "/fonts/Inter.ttf",
            1,
            "Inter-Regular",
            Some("Version 2.9"),
        )],
    );
    index
        .conn
        .execute(
            "UPDATE face SET parsed = json_set(parsed, '$.version', 'Version 2.10')
             WHERE file_id = (SELECT id FROM font_file WHERE path = '/fonts/Inter.ttf')",
            [],
        )
        .expect("update");
    assert_eq!(
        summary(&index),
        ["Exact Inter-Regular: /fonts/Inter.ttf, /lib/Inter.ttf"]
    );
}

#[test]
fn gone_paused_and_unread_files_are_not_duplicates() {
    let mut index = index();
    let mut gone = file("/fonts/gone.ttf", 1, "A", Some("1"));
    gone.gone = true;
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file("/fonts/a.ttf", 1, "A", Some("1")),
            gone,
            file("/fonts/b.ttf", 2, "B", Some("1")),
            file("/fonts/unread.ttf", 2, "B", Some("1")),
            file("/fonts/c.ttf", 3, "C", Some("1")),
        ],
    );
    let paused = add(
        &mut index,
        "/paused",
        SourceKind::Watched,
        &[file("/paused/c.ttf", 3, "C", Some("1"))],
    );
    index.set_source_paused(paused, true).expect("pause");
    index
        .conn
        .execute(
            "UPDATE font_file SET hash = NULL WHERE path = '/fonts/unread.ttf'",
            [],
        )
        .expect("update");
    assert!(summary(&index).is_empty(), "{:?}", summary(&index));

    index.set_source_paused(paused, false).expect("resume");
    assert_eq!(summary(&index), ["Exact C: /fonts/c.ttf, /paused/c.ttf"]);
}

#[test]
fn identical_broken_files_are_an_exact_group_without_a_face() {
    let mut index = index();
    let broken = |path: &str| {
        let mut f = file(path, 9, "", None);
        f.parsed = ParsedFile {
            format: None,
            health: Health::Broken("not a font".into()),
            faces: Vec::new(),
        };
        f
    };
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[broken("/fonts/x.ttf"), broken("/fonts/y.ttf")],
    );
    let groups = index.duplicates().expect("duplicates");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].kind, DuplicateKind::Exact);
    assert_eq!(groups[0].postscript_name, None);
    assert!(
        groups[0]
            .copies
            .iter()
            .all(|c| c.face.is_none() && c.version.is_none())
    );
}

#[test]
fn a_collection_with_the_name_twice_is_one_copy() {
    let mut index = index();
    let mut ttc = file("/fonts/Family.ttc", 1, "Family-Regular", Some("1.0"));
    let mut second = ttc.parsed.faces[0].clone();
    second.index = 1;
    ttc.parsed.faces.push(second);
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            ttc,
            file("/fonts/Family.ttf", 2, "Family-Regular", Some("1.0")),
        ],
    );
    assert_eq!(
        summary(&index),
        ["Near Family-Regular: /fonts/Family.ttc, /fonts/Family.ttf"]
    );
}

#[test]
fn a_non_utf8_path_is_listed() {
    let mut index = index();
    let odd = fm_platform::fs::path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("path");
    assert!(odd.to_str().is_none(), "{odd:?} should not be UTF-8");
    let mut a = file("/fonts/a.ttf", 1, "A", None);
    a.path = odd.clone();
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[a, file("/fonts/b.ttf", 1, "A", None)],
    );
    let groups = index.duplicates().expect("duplicates");
    assert_eq!(groups.len(), 1);
    let paths: Vec<&str> = groups[0].copies.iter().map(|c| c.path.as_str()).collect();
    assert!(paths.contains(&odd.to_string_lossy().as_ref()), "{paths:?}");
    assert!(paths.contains(&"/fonts/b.ttf"));
}
