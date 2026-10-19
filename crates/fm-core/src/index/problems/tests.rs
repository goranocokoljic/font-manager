use std::fs;
use std::path::{Path, PathBuf};

use fm_api::library::{ProblemKind, SourceKind};

use crate::index::{INDEX_FILE, Index};
use crate::scanner::{FileIndex, discover};
use crate::test_support::TempDir;

const HEALTHY: &str = "google-static/asap/Asap_Condensed-ExtraLightItalic.ttf";
const TRUNCATED: &str = "derived/agdasima/agdasima-truncated.ttf";
const TYPE1: &str = "type1/inconsolata/Inconsolata-zi4r.pfb";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn copy_fixture(root: &Path, rel: &str) -> PathBuf {
    let to = root.join(Path::new(rel).file_name().expect("name"));
    fs::create_dir_all(root).expect("folder");
    fs::copy(fixtures().join(rel), &to).expect("copy fixture");
    to
}

/// Scans `root` as a new watched source of `index`, as the app does.
fn scan_source(index: &mut Index, root: &Path) {
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    let mut files = FileIndex::new();
    let rescan = files.rescan(root, &discover(root).expect("discover"));
    index
        .apply_rescan(source, &files, &rescan)
        .expect("persist");
}

fn kinds(index: &Index) -> Vec<ProblemKind> {
    index
        .problems()
        .expect("problems")
        .iter()
        .map(|p| p.kind)
        .collect()
}

#[test]
fn a_healthy_library_has_no_problems() {
    let dir = TempDir::new("problems-healthy");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, HEALTHY);
    let mut index = Index::open_in_memory().expect("open");
    scan_source(&mut index, &fonts);
    assert!(index.problems().expect("problems").is_empty());
}

#[test]
fn the_fixtures_broken_fonts_are_problems_entries() {
    let dir = TempDir::new("problems-broken");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, HEALTHY);
    let truncated = copy_fixture(&fonts, TRUNCATED);
    let type1 = copy_fixture(&fonts, TYPE1);
    let junk = fonts.join("junk.otf");
    fs::write(&junk, b"OTTO and then nothing a font needs").expect("junk");
    let mut index = Index::open_in_memory().expect("open");
    scan_source(&mut index, &fonts);

    let problems = index.problems().expect("problems");
    let entry = |path: &Path| {
        problems
            .iter()
            .find(|p| Path::new(&p.path) == path)
            .unwrap_or_else(|| panic!("no entry for {}: {problems:?}", path.display()))
    };
    for path in [&truncated, &junk] {
        let p = entry(path);
        assert_eq!(p.kind, ProblemKind::Broken, "{}", path.display());
        assert!(!p.message.is_empty());
    }
    assert_eq!(entry(&type1).kind, ProblemKind::Unsupported);
    assert_eq!(
        problems.len(),
        3,
        "the healthy font is not listed: {problems:?}"
    );
}

#[test]
fn broken_files_of_a_paused_source_or_gone_files_are_not_listed() {
    let dir = TempDir::new("problems-paused");
    let fonts = dir.path().join("fonts");
    let truncated = copy_fixture(&fonts, TRUNCATED);
    let mut index = Index::open_in_memory().expect("open");
    scan_source(&mut index, &fonts);
    assert_eq!(kinds(&index), vec![ProblemKind::Broken]);

    let source = index.sources().expect("sources")[0].id;
    index.set_source_paused(source, true).expect("pause");
    assert!(kinds(&index).is_empty());
    index.set_source_paused(source, false).expect("resume");

    fs::remove_file(&truncated).expect("remove");
    let mut files = index.load_files().expect("load");
    let rescan = files.rescan(&fonts, &discover(&fonts).expect("discover"));
    index
        .apply_rescan(source, &files, &rescan)
        .expect("persist");
    assert!(kinds(&index).is_empty(), "a gone file is not a problem");
}

#[test]
fn a_missing_source_folder_is_a_problems_entry_not_an_error() {
    let dir = TempDir::new("problems-missing");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, HEALTHY);
    let mut index = Index::open_in_memory().expect("open");
    scan_source(&mut index, &fonts);
    assert!(kinds(&index).is_empty());

    fs::remove_dir_all(&fonts).expect("remove the folder");
    let problems = index.problems().expect("a missing folder is not an error");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].kind, ProblemKind::MissingSource);
    assert_eq!(Path::new(&problems[0].path), fonts);
    assert!(problems[0].message.contains("cannot be found"));

    // A file where the folder was is not a folder either.
    fs::write(&fonts, b"").expect("file in its place");
    let problems = index.problems().expect("problems");
    assert_eq!(problems[0].kind, ProblemKind::MissingSource);
    assert!(problems[0].message.contains("not a folder"));
}

#[test]
fn a_corrupted_database_is_a_problems_entry_not_a_crash() {
    let dir = TempDir::new("problems-corrupt");
    let fonts = dir.path().join("fonts");
    copy_fixture(&fonts, HEALTHY);
    let db = dir.path().join("data").join(INDEX_FILE);
    {
        let mut index = Index::open(&db).expect("open").index;
        scan_source(&mut index, &fonts);
        assert!(index.problems().expect("problems").is_empty());
    }
    fs::write(&db, b"this is not a database, it is garbage").expect("corrupt");

    let opened = Index::open(&db).expect("a damaged file is replaced, not an error");
    assert!(opened.rebuilt.is_some());
    let problems = opened.index.problems().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    let p = &problems[0];
    assert_eq!(p.kind, ProblemKind::IndexRebuilt);
    assert_eq!(Path::new(&p.path), db);
    let kept = p.other.as_deref().expect("where the damaged file went");
    assert!(
        Path::new(kept).is_file(),
        "the damaged file is kept: {kept}"
    );
    assert!(!p.message.is_empty());
    // The sources came back from the backup, so the next scan refills it.
    assert_eq!(opened.index.sources().expect("sources").len(), 1);

    drop(opened);
    let reopened = Index::open(&db).expect("open again");
    assert!(
        reopened.index.problems().expect("problems").is_empty(),
        "the next session opens the fresh index cleanly"
    );
}

#[test]
fn every_kind_of_problem_comes_together_in_order() {
    let dir = TempDir::new("problems-all");
    let gone = dir.path().join("gone");
    let fonts = dir.path().join("fonts");
    copy_fixture(&gone, HEALTHY);
    copy_fixture(&fonts, TRUNCATED);
    let db = dir.path().join("data").join(INDEX_FILE);
    {
        let mut index = Index::open(&db).expect("open").index;
        scan_source(&mut index, &gone);
    }
    fs::write(&db, b"garbage").expect("corrupt");
    fs::remove_dir_all(&gone).expect("remove");
    let mut index = Index::open(&db).expect("rebuilt").index;
    scan_source(&mut index, &fonts);
    assert_eq!(
        kinds(&index),
        vec![
            ProblemKind::IndexRebuilt,
            ProblemKind::MissingSource,
            ProblemKind::Broken
        ]
    );
}

#[test]
fn a_duplicate_copy_is_an_entry_pointing_at_the_keeper() {
    let dir = TempDir::new("problems-duplicates");
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    let a = copy_fixture(&first, HEALTHY);
    let b = copy_fixture(&second, HEALTHY);
    let mut index = Index::open_in_memory().expect("open");
    scan_source(&mut index, &first);
    scan_source(&mut index, &second);
    let problems = index.problems().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    let p = &problems[0];
    assert_eq!(p.kind, ProblemKind::DuplicateFile);
    // Equal versions, neither in the library folder: the keeper is the path A to Z.
    assert_eq!(p.path, b.display().to_string());
    assert_eq!(p.other.as_deref(), Some(a.display().to_string().as_str()));
    assert!(p.face.is_some());
}
