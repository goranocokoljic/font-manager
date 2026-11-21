use std::collections::HashMap;

use fm_api::common::{FontFormat, Page};
use fm_api::library::KeeperPolicy;
use fm_api::{FaceId, SourceId};
use serde::Deserialize;
use serde_json::Value;

use super::plan::{PairPlanRequest, PairStay, PolicyInput, removals};
use super::*;
use crate::duplicate_sets::SetFace;
use crate::keeper_policy::LanguageNames;

/// One row of `fixtures/duplicates/pair-cases.json`.
#[derive(Deserialize)]
struct Case {
    name: String,
    roots: Vec<CaseRoot>,
    files: Vec<CaseFile>,
    request: ListFolderPairsRequest,
    pairs: Value,
}

#[derive(Deserialize)]
struct CaseRoot {
    path: String,
    kind: ProtectedFolder,
}

#[derive(Deserialize)]
struct CaseFile {
    path: String,
    hash: String,
    format: Option<FontFormat>,
    source: u32,
    kind: String,
    #[serde(default)]
    size: Option<u32>,
    faces: Vec<CaseFace>,
}

#[derive(Deserialize)]
struct CaseFace {
    face: u32,
    name: String,
    version: Option<String>,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!(
        "../../../../fixtures/duplicates/pair-cases.json"
    ))
    .expect("pair-cases.json")
}

fn files_of(case: &Case) -> Vec<SetFile> {
    case.files
        .iter()
        .map(|f| SetFile {
            path: f.path.clone(),
            size: f.size.unwrap_or(1000),
            hash: f.hash.clone(),
            format: f.format,
            source: SourceId(f.source),
            in_library_folder: f.kind == "libraryFolder",
            system: f.kind == "system",
            faces: f
                .faces
                .iter()
                .map(|x| SetFace {
                    face: FaceId(x.face),
                    name: x.name.clone(),
                    version: x.version.clone(),
                })
                .collect(),
        })
        .collect()
}

fn roots_of(case: &Case) -> Vec<Root> {
    case.roots
        .iter()
        .map(|r| Root {
            path: PathBuf::from(&r.path),
            kind: r.kind,
        })
        .collect()
}

fn case(name: &str) -> (Vec<SetFile>, Vec<Root>) {
    let all = cases();
    let c = all
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("case {name}"));
    (files_of(c), roots_of(c))
}

fn all_pairs(files: &[SetFile], roots: &[Root]) -> Vec<FolderPair> {
    list(files, roots, &ListFolderPairsRequest::default()).items
}

/// A path as this OS writes it, for the case file's `/` paths.
fn p(path: &str) -> String {
    PathBuf::from(path).to_string_lossy().into_owned()
}

#[test]
fn the_rule_gives_the_answer_of_every_shared_case() {
    let all = cases();
    assert!(all.len() >= 9, "the table is there");
    for c in &all {
        let got = serde_json::to_value(list(&files_of(c), &roots_of(c), &c.request)).unwrap();
        assert_eq!(got, c.pairs, "case: {}", c.name);
    }
}

#[test]
fn a_copied_tree_is_one_contained_pair_at_its_top() {
    let (files, roots) = case("a copied tree is one pair at its top, not one per subfolder");
    let pairs = all_pairs(&files, &roots);
    assert_eq!(pairs.len(), 1, "{pairs:#?}");
    let pair = &pairs[0];
    assert_eq!(pair.a.path, p("/backup/old"));
    assert_eq!(pair.b.path, p("/lib/fonts"));
    for side in [&pair.a, &pair.b] {
        assert!(side.contained);
        assert_eq!(side.font_files, 6);
        assert_eq!(side.shared.identical, 6);
        assert_eq!(side.only_here, 0);
        assert_eq!(side.protected, None, "a folder below a source root");
    }
    assert_eq!(pair.frees, 6000);
}

#[test]
fn a_copied_tree_with_one_changed_file_keeps_the_changed_folder_apart() {
    let (files, roots) = case(
        "a copied tree with one changed file: the unchanged folder is contained, the changed one shares the rest",
    );
    let pairs = all_pairs(&files, &roots);
    let paths: Vec<(&str, &str)> = pairs
        .iter()
        .map(|x| (x.a.path.as_str(), x.b.path.as_str()))
        .collect();
    let (x, y) = (
        (p("/backup/tree/x"), p("/lib/tree/x")),
        (p("/backup/tree/y"), p("/lib/tree/y")),
    );
    assert_eq!(
        paths,
        [(x.0.as_str(), x.1.as_str()), (y.0.as_str(), y.1.as_str())],
        "the unchanged folder frees more and comes first; the tree does not climb"
    );
    assert!(pairs[0].a.contained && pairs[0].b.contained);
    let changed = &pairs[1];
    assert!(!changed.a.contained && !changed.b.contained);
    assert_eq!(changed.a.shared.identical, 4);
    assert_eq!(changed.a.shared.other_version, 1);
    assert_eq!(changed.a.only_here, 0);
    assert_eq!(changed.frees, 4000);
}

#[test]
fn two_folders_that_share_a_third_list_what_only_each_has() {
    let (files, roots) = case("two folders that share a third of their files");
    let pairs = all_pairs(&files, &roots);
    assert_eq!(pairs.len(), 1);
    let pair = &pairs[0];
    for side in [&pair.a, &pair.b] {
        assert_eq!(side.font_files, 15);
        assert_eq!(side.shared.identical, 5);
        assert_eq!(side.only_here, 10);
        assert_eq!(side.size, 30_000);
        assert_eq!(side.shared_size, 10_000);
        assert!(!side.contained);
        assert_eq!(side.protected, None);
    }
    // The second request names the files only one side has.
    let only = list_files(
        &files,
        &roots,
        &ListFolderPairFilesRequest {
            a: p("/lib/p"),
            b: p("/lib/q"),
            side: Some(PairSide::B),
            only_here: true,
            page: Page::default(),
        },
    )
    .unwrap();
    assert_eq!(only.total, 10);
    assert!(only.items.iter().all(|f| f.side == PairSide::B
        && f.level.is_none()
        && f.copy.is_none()
        && f.path.contains('Q')));
    let both = list_files(
        &files,
        &roots,
        &ListFolderPairFilesRequest {
            a: p("/lib/p"),
            b: p("/lib/q"),
            side: None,
            only_here: false,
            page: Page::default(),
        },
    )
    .unwrap();
    assert_eq!(both.total, 30);
    let shared = both
        .items
        .iter()
        .find(|f| f.path == p("/lib/p/P0.ttf"))
        .unwrap();
    assert_eq!(shared.level, Some(DuplicateLevel::Identical));
    assert_eq!(shared.copy.as_deref(), Some(p("/lib/q/P0.ttf").as_str()));
}

#[test]
fn a_folder_contained_in_two_others_is_two_pairs() {
    let (files, roots) = case("a folder contained in two others");
    let pairs = all_pairs(&files, &roots);
    let find = |a: &str, b: &str| {
        pairs
            .iter()
            .find(|x| x.a.path == p(a) && x.b.path == p(b))
            .unwrap_or_else(|| panic!("{a} {b}: {pairs:#?}"))
    };
    for other in ["/lib/d", "/lib/e"] {
        let pair = find("/lib/c", other);
        assert!(pair.a.contained, "c is contained in {other}");
        assert!(!pair.b.contained, "{other} has more");
        assert_eq!(pair.frees, 5000, "removing c frees its five files");
    }
    let de = find("/lib/d", "/lib/e");
    assert!(!de.a.contained && !de.b.contained);
    assert_eq!(pairs.len(), 3);
}

#[test]
fn the_threshold_hides_a_pair_with_fewer_shared_files_and_pages_count_every_pair() {
    let (files, roots) =
        case("four shared files are below the threshold, and data that is no font is not counted");
    assert!(all_pairs(&files, &roots).is_empty());
    let low = list(
        &files,
        &roots,
        &ListFolderPairsRequest {
            min_shared: Some(4),
            ..Default::default()
        },
    );
    assert_eq!(low.total, 1);
    assert_eq!(low.items[0].a.font_files, 4, "the readme is no font");
    let none = list(
        &files,
        &roots,
        &ListFolderPairsRequest {
            min_shared: Some(5),
            ..Default::default()
        },
    );
    assert_eq!(none.total, 0, "five is the default and four is below it");
    assert_eq!(FOLDER_PAIR_MIN_SHARED, 5);
}

#[test]
fn pairs_are_ordered_by_what_removing_the_smaller_side_frees() {
    let (files, roots) = case("a folder that holds the other, other levels, and a system folder");
    let pairs = all_pairs(&files, &roots);
    let frees: Vec<u32> = pairs.iter().map(|x| x.frees).collect();
    let mut sorted = frees.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(frees, sorted);
    // The folder that holds a copy of itself leaves the copy out of its side.
    let nested = pairs
        .iter()
        .find(|x| x.a.path == p("/lib/n") && x.b.path == p("/lib/n/copy"))
        .expect("the nested pair");
    assert_eq!(nested.a.font_files, 5);
    assert_eq!(nested.b.font_files, 5);
    assert_eq!(nested.a.shared.same_version, 3);
    assert_eq!(nested.a.shared.other_version, 2);
    assert_eq!(nested.frees, 0, "no identical copy: removing frees nothing");
    let system = pairs
        .iter()
        .find(|x| x.b.path == p("/sys"))
        .expect("the system pair");
    assert_eq!(system.b.protected, Some(ProtectedFolder::SystemFolder));
    assert_eq!(system.a.protected, None);
}

fn no_facts() -> HashMap<FaceId, fm_api::library::CopyFacts> {
    HashMap::new()
}

fn plan(
    files: &[SetFile],
    roots: &[Root],
    a: &str,
    b: &str,
    keep: PairSide,
    other_levels: bool,
    policy: &KeeperPolicy,
) -> fm_api::ApiResult<plan::PairRemovals> {
    let (facts, names) = (no_facts(), LanguageNames::default());
    removals(
        files,
        roots,
        &PairPlanRequest {
            a: p(a),
            b: p(b),
            keep,
            other_levels,
        },
        &PolicyInput {
            policy,
            my_languages: &[],
            names: &names,
            facts: &facts,
        },
    )
}

#[test]
fn keeping_one_side_plans_only_the_identical_copies_of_the_other() {
    let (files, roots) = case(
        "a copied tree with one changed file: the unchanged folder is contained, the changed one shares the rest",
    );
    let policy = KeeperPolicy::default();
    let got = plan(
        &files,
        &roots,
        "/backup/tree/y",
        "/lib/tree/y",
        PairSide::B,
        false,
        &policy,
    )
    .unwrap();
    assert_eq!(got.remove.len(), 4);
    assert!(
        got.remove
            .iter()
            .all(|r| r.level == DuplicateLevel::Identical
                && r.path.starts_with(&p("/backup/tree/y"))
                && r.kept_copy.starts_with(&p("/lib/tree/y")))
    );
    assert_eq!(got.stays.len(), 1);
    assert_eq!(got.stays[0].path, p("/backup/tree/y/Gy3.ttf"));
    assert_eq!(
        got.stays[0].why,
        PairStay::OtherLevel(DuplicateLevel::OtherVersion)
    );
}

#[test]
fn other_levels_go_through_the_policy_when_asked_for() {
    let (files, roots) = case(
        "a copied tree with one changed file: the unchanged folder is contained, the changed one shares the rest",
    );
    let policy = KeeperPolicy::default();
    // /backup holds the newer version: the policy keeps it over the kept
    // side's older copy.
    let newer_kept = plan(
        &files,
        &roots,
        "/backup/tree/y",
        "/lib/tree/y",
        PairSide::B,
        true,
        &policy,
    )
    .unwrap();
    assert_eq!(newer_kept.remove.len(), 4);
    assert_eq!(
        newer_kept.stays[0].why,
        PairStay::PolicyKeeps(Some(p("/backup/tree/y/Gy3.ttf")))
    );
    // Keeping /backup's copies: the older version in /lib loses to it.
    let older_removed = plan(
        &files,
        &roots,
        "/backup/tree/y",
        "/lib/tree/y",
        PairSide::A,
        true,
        &policy,
    )
    .unwrap();
    assert_eq!(older_removed.remove.len(), 5);
    let other = older_removed
        .remove
        .iter()
        .find(|r| r.level == DuplicateLevel::OtherVersion)
        .expect("the older version is planned");
    assert_eq!(other.path, p("/lib/tree/y/Gy3.ttf"));
    assert_eq!(other.kept_copy, p("/backup/tree/y/Gy3.ttf"));
}

#[test]
fn a_protected_folder_can_be_kept_and_never_removed() {
    let (files, roots) = case("a copied tree is one pair at its top, not one per subfolder");
    let policy = KeeperPolicy::default();
    // Climbed to the source roots' children; the roots themselves as a pair.
    let err = plan(
        &files,
        &roots,
        "/backup",
        "/lib",
        PairSide::A,
        false,
        &policy,
    )
    .unwrap_err();
    assert!(
        err.message.contains("never the side that is removed"),
        "{err:?}"
    );
    let kept = plan(
        &files,
        &roots,
        "/backup/old",
        "/lib",
        PairSide::B,
        false,
        &policy,
    )
    .unwrap();
    assert_eq!(kept.remove.len(), 6, "the root is the kept side");

    let (files, roots) = case("a folder that holds the other, other levels, and a system folder");
    let err = plan(
        &files,
        &roots,
        "/lib/n",
        "/sys",
        PairSide::A,
        false,
        &policy,
    )
    .unwrap_err();
    assert!(err.message.contains("system font folder"), "{err:?}");
    let ok = plan(
        &files,
        &roots,
        "/lib/n",
        "/sys",
        PairSide::B,
        false,
        &policy,
    )
    .unwrap();
    assert_eq!(ok.remove.len(), 5, "the system side is kept");
    assert!(ok.remove.iter().all(|r| r.path.starts_with(&p("/lib/n"))));
    assert!(
        !ok.remove
            .iter()
            .any(|r| r.path.starts_with(&p("/lib/n/copy"))),
        "the folder inside is the other side's business"
    );
}

#[test]
fn the_same_folder_twice_is_refused() {
    let (files, roots) = case("two folders that share a third of their files");
    let err = list_files(
        &files,
        &roots,
        &ListFolderPairFilesRequest {
            a: p("/lib/p"),
            b: p("/lib/p"),
            side: None,
            only_here: false,
            page: Page::default(),
        },
    )
    .unwrap_err();
    assert_eq!(err.code, fm_api::ErrorCode::InvalidRequest);
}

#[test]
fn files_that_are_not_fonts_are_counted_from_the_folder() {
    let dir = crate::test_support::TempDir::new("folder-pairs");
    let a = dir.path().join("a");
    let nested = a.join("copy");
    std::fs::create_dir_all(a.join("sub")).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(a.join("One.ttf"), b"x").unwrap();
    std::fs::write(a.join("readme.txt"), b"x").unwrap();
    std::fs::write(a.join("sub").join("notes.pdf"), b"x").unwrap();
    std::fs::write(nested.join("One.ttf"), b"x").unwrap();
    std::fs::write(nested.join("license.txt"), b"x").unwrap();
    assert_eq!(count_other_files(&a, &nested, 1), 2, "the copy is left out");
    assert_eq!(count_other_files(&nested, &a, 1), 1);
    assert_eq!(count_other_files(&dir.path().join("gone"), &a, 0), 0);
    assert!(a.join("readme.txt").exists(), "nothing is touched");
}
