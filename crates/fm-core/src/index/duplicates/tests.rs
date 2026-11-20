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

// --- E56.1: duplicate sets over the index ---------------------------------

use fm_api::library::{DuplicateLevel, DuplicateScope};

/// Each set as `level id: path, path`, in the order of the listing.
fn set_summary(index: &Index, scope: &DuplicateScope) -> Vec<String> {
    let files = index.duplicate_set_files().expect("set files");
    crate::duplicate_sets::sets(&files, scope)
        .iter()
        .map(|s| {
            let paths: Vec<&str> = s.copies.iter().map(|c| c.path.as_str()).collect();
            format!("{:?} {}: {}", s.level, s.id, paths.join(", "))
        })
        .collect()
}

#[test]
fn sets_count_only_the_files_the_library_shows() {
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
            file("/fonts/unread.ttf", 3, "B", Some("2")),
            file("/fonts/c.ttf", 4, "C", Some("1")),
        ],
    );
    let paused = add(
        &mut index,
        "/paused",
        SourceKind::Watched,
        &[file("/paused/c.ttf", 5, "C", Some("2"))],
    );
    index.set_source_paused(paused, true).expect("pause");
    index
        .conn
        .execute(
            "UPDATE font_file SET hash = NULL WHERE path = '/fonts/unread.ttf'",
            [],
        )
        .expect("update");
    let library = DuplicateScope::Library;
    assert!(
        set_summary(&index, &library).is_empty(),
        "{:?}",
        set_summary(&index, &library)
    );

    index.set_source_paused(paused, false).expect("resume");
    assert_eq!(
        set_summary(&index, &library),
        ["OtherVersion name:C: /fonts/c.ttf, /paused/c.ttf"]
    );
}

#[test]
fn every_level_comes_from_the_index_and_list_duplicates_keeps_its_groups() {
    let mut index = index();
    let mut otf = file("/fonts/Lato.otf", 3, "Lato-Regular", Some("Version 1.0"));
    otf.parsed.format = Some(FontFormat::Otf);
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[
            file("/fonts/a/Inter.ttf", 1, "Inter-Regular", Some("4.0")),
            file("/fonts/b/Inter.ttf", 1, "Inter-Regular", Some("4.0")),
            file("/fonts/Lato.ttf", 2, "Lato-Regular", Some("Version 1.000")),
            otf,
            file("/fonts/a/Mono.ttf", 4, "Mono", Some("1.0")),
            file("/fonts/b/Mono.ttf", 4, "Mono", Some("1.0")),
            file("/fonts/c/Mono.ttf", 5, "Mono", Some("2.0")),
        ],
    );
    let before = summary(&index);
    let files = index.duplicate_set_files().expect("set files");
    let sets = crate::duplicate_sets::sets(&files, &DuplicateScope::Library);
    let levels: Vec<(DuplicateLevel, bool)> =
        sets.iter().map(|s| (s.level, s.formats_differ)).collect();
    assert_eq!(
        levels,
        [
            (DuplicateLevel::Identical, false),
            (DuplicateLevel::SameVersion, true),
            (DuplicateLevel::OtherVersion, false),
        ]
    );
    let mono = &sets[2];
    assert_eq!(mono.copies.len(), 3, "one set for the three copies");
    assert_eq!(mono.copies[0].identical_with, ["/fonts/b/Mono.ttf"]);
    assert!(mono.copies[2].identical_with.is_empty());
    assert_eq!(mono.copies[0].format, Some(FontFormat::Ttf));
    // The old listing is not changed by the new one: it compares versions as
    // text (so not Lato's 1.0 and 1.000), and Mono's versions differ, so it
    // only knows Mono's identical pair.
    assert_eq!(summary(&index), before);
    assert_eq!(
        before,
        [
            "Exact Inter-Regular: /fonts/a/Inter.ttf, /fonts/b/Inter.ttf",
            "Exact Mono: /fonts/a/Mono.ttf, /fonts/b/Mono.ttf",
        ]
    );
}

#[test]
fn a_system_copy_is_marked_and_a_collection_says_which_other_sets_hold_it() {
    let mut index = index();
    let mut ttc = file("/fonts/Family.ttc", 1, "Fam-Regular", Some("1.0"));
    let mut bold = ttc.parsed.faces[0].clone();
    bold.index = 1;
    bold.postscript_name = Some("Fam-Bold".into());
    ttc.parsed.faces.push(bold);
    ttc.parsed.format = Some(FontFormat::Ttc);
    add(
        &mut index,
        "/fonts",
        SourceKind::Watched,
        &[ttc, file("/fonts/Fam-Bold.ttf", 2, "Fam-Bold", Some("2.0"))],
    );
    add(
        &mut index,
        "/System/Fonts",
        SourceKind::System,
        &[file("/System/Fonts/Fam.ttf", 3, "Fam-Regular", Some("1.0"))],
    );
    let files = index.duplicate_set_files().expect("set files");
    let sets = crate::duplicate_sets::sets(&files, &DuplicateScope::Library);
    let ids: Vec<&str> = sets.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["name:Fam-Bold", "name:Fam-Regular"]);
    let ttc_in = |i: usize| {
        sets[i]
            .copies
            .iter()
            .find(|c| c.path == "/fonts/Family.ttc")
            .expect("ttc")
            .clone()
    };
    assert_eq!(ttc_in(0).other_sets, ["name:Fam-Regular"]);
    assert_eq!(ttc_in(1).other_sets, ["name:Fam-Bold"]);
    assert_ne!(ttc_in(0).face, ttc_in(1).face, "one face per set");
    let system = sets[1]
        .copies
        .iter()
        .find(|c| c.path.starts_with("/System"))
        .expect("system copy");
    assert!(system.system);
    assert!(sets[1].copies.iter().filter(|c| c.system).count() == 1);
}
