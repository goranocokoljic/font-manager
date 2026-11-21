//! The plan of a cleanup over a library made up in memory: every outcome a
//! plan can have, in one plan.

use std::collections::HashMap;

use fm_api::SourceId;
use fm_api::cleanup::{CleanupKeeper, CleanupScope, DuplicateAnswer, PlanCleanupRequest, RemoveAs};

use super::*;
use crate::duplicate_sets::SetFace;

fn file(path: &str, hash: &str, faces: &[(u32, &str, &str)], system: bool) -> SetFile {
    SetFile {
        path: path.to_owned(),
        size: 100,
        hash: hash.to_owned(),
        format: Some(fm_api::common::FontFormat::Ttf),
        source: SourceId(1),
        in_library_folder: false,
        system,
        faces: faces
            .iter()
            .map(|&(id, name, version)| SetFace {
                face: FaceId(id),
                name: name.to_owned(),
                version: Some(version.to_owned()),
            })
            .collect(),
    }
}

fn facts(languages: &[&str]) -> CopyFacts {
    CopyFacts {
        modified: None,
        glyph_count: None,
        variable: false,
        axes: Vec::new(),
        features: Vec::new(),
        activation: None,
        activation_documents: Vec::new(),
        favorite: false,
        rating: None,
        tags: Vec::new(),
        has_notes: false,
        collections: Vec::new(),
        sets: Vec::new(),
        languages_full: languages.iter().map(|&l| l.to_owned()).collect(),
        languages_locl: Vec::new(),
    }
}

/// A library with one set per outcome:
/// - `A`: identical, the second copy goes to the trash;
/// - `B`: identical, the second copy is active (its activation can move);
/// - `C`: another version, the older copy is active (it stays, no offer);
/// - `D`: identical, one copy in a system font folder;
/// - `E`: another version, the user keeps the copy that lacks one of my
///   languages, so the guard keeps the other;
/// - `F`: another version, each copy has a language the other lacks:
///   "keep both";
/// - `G1`/`G2`: a collection file with an older `G1` and a newer `G2`: the
///   `G1` set would remove it, the `G2` set keeps it;
/// - `H`: identical, the second copy is not on disk any more.
struct World {
    files: Vec<SetFile>,
    facts: HashMap<FaceId, CopyFacts>,
    active: HashMap<String, ActivationMode>,
    gone: Vec<String>,
    languages: Vec<String>,
    decided: HashMap<String, DuplicateAnswer>,
}

impl World {
    fn new() -> Self {
        let files = vec![
            file("/lib/a/A.ttf", "a1", &[(1, "A", "1.0")], false),
            file("/lib/z/A.ttf", "a1", &[(2, "A", "1.0")], false),
            file("/lib/a/B.ttf", "b1", &[(3, "B", "1.0")], false),
            file("/lib/z/B.ttf", "b1", &[(4, "B", "1.0")], false),
            file("/lib/a/C.ttf", "c2", &[(5, "C", "2.0")], false),
            file("/lib/z/C.ttf", "c1", &[(6, "C", "1.0")], false),
            file("/sys/D.ttf", "d1", &[(7, "D", "1.0")], true),
            file("/lib/z/D.ttf", "d1", &[(8, "D", "1.0")], false),
            file("/lib/a/E.ttf", "e2", &[(9, "E", "2.0")], false),
            file("/lib/z/E.ttf", "e1", &[(10, "E", "1.0")], false),
            file("/lib/a/F.ttf", "f2", &[(11, "F", "2.0")], false),
            file("/lib/z/F.ttf", "f1", &[(12, "F", "1.0")], false),
            file("/lib/a/G1.ttf", "g1", &[(13, "G1", "2.0")], false),
            file("/lib/a/G2.ttf", "g2", &[(14, "G2", "1.0")], false),
            file(
                "/lib/z/G.ttc",
                "g3",
                &[(15, "G1", "1.0"), (16, "G2", "2.0")],
                false,
            ),
            file("/lib/a/H.ttf", "h1", &[(17, "H", "1.0")], false),
            file("/lib/z/H.ttf", "h1", &[(18, "H", "1.0")], false),
        ];
        let mut facts = HashMap::new();
        for (face, langs) in [
            (9, &["de"][..]),
            (10, &["de", "sr"][..]),
            (11, &["de"][..]),
            (12, &["sr"][..]),
        ] {
            facts.insert(FaceId(face), facts_of(langs));
        }
        let mut active = HashMap::new();
        active.insert("/lib/z/B.ttf".to_owned(), ActivationMode::Session);
        active.insert("/lib/z/C.ttf".to_owned(), ActivationMode::Permanent);
        Self {
            files,
            facts,
            active,
            gone: vec!["/lib/z/H.ttf".to_owned()],
            languages: vec!["de".to_owned(), "sr".to_owned()],
            decided: HashMap::new(),
        }
    }

    fn plan(&self, req: &PlanCleanupRequest) -> ApiResult<PlannedFiles> {
        let policy = KeeperPolicy {
            rules: vec![
                fm_api::library::KeeperRule::NewestVersion,
                fm_api::library::KeeperRule::Path,
            ],
            ..KeeperPolicy::default()
        };
        let names = LanguageNames::default();
        let exists = |p: &str| !self.gone.iter().any(|g| g == p);
        let ctx = PlanContext {
            files: &self.files,
            roots: &[],
            facts: &self.facts,
            active: &self.active,
            policy: &policy,
            my_languages: &self.languages,
            names: &names,
            exists: &exists,
            decided: &self.decided,
        };
        plan(&ctx, req)
    }
}

fn facts_of(languages: &[&str]) -> CopyFacts {
    facts(languages)
}

fn row<'a>(p: &'a PlannedFiles, path: &str) -> &'a CleanupFile {
    p.files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("no row for {path}: {:#?}", p.files))
}

fn outcome(p: &PlannedFiles, path: &str) -> (CleanupAction, Option<StayReason>) {
    let r = row(p, path);
    (r.action, r.reason)
}

fn request() -> PlanCleanupRequest {
    PlanCleanupRequest {
        keepers: vec![CleanupKeeper {
            set: "name:E".into(),
            keep: "/lib/a/E.ttf".into(),
        }],
        ..PlanCleanupRequest::default()
    }
}

#[test]
fn a_plan_mixes_every_outcome_and_says_why() {
    let w = World::new();
    let p = w.plan(&request()).expect("plan");
    use CleanupAction::{Stays, Trash};
    use StayReason as R;
    let cases = [
        ("/lib/a/A.ttf", Stays, Some(R::Keeper)),
        ("/lib/z/A.ttf", Trash, None),
        ("/lib/z/B.ttf", Stays, Some(R::Active)),
        ("/lib/z/C.ttf", Stays, Some(R::Active)),
        ("/sys/D.ttf", Stays, Some(R::System)),
        ("/lib/z/D.ttf", Stays, Some(R::Keeper)),
        ("/lib/a/E.ttf", Stays, Some(R::Keeper)),
        ("/lib/z/E.ttf", Stays, Some(R::MyLanguages)),
        ("/lib/a/F.ttf", Stays, Some(R::KeepBoth)),
        ("/lib/z/F.ttf", Stays, Some(R::KeepBoth)),
        ("/lib/z/G.ttc", Stays, Some(R::KeptByOtherSet)),
        ("/lib/a/G1.ttf", Stays, Some(R::Keeper)),
        ("/lib/z/H.ttf", Stays, Some(R::NotThere)),
    ];
    for (path, action, reason) in cases {
        assert_eq!(
            outcome(&p, path),
            (action, reason),
            "{path}: {:#?}",
            row(&p, path)
        );
        assert!(!row(&p, path).sentence.is_empty(), "{path} says why");
    }
    assert_eq!(
        row(&p, "/lib/z/A.ttf").kept.as_deref(),
        Some("/lib/a/A.ttf")
    );
    assert!(row(&p, "/lib/z/E.ttf").sentence.contains("only copy"));
    // The copy of `A`, and the older `G2` (the collection file is kept for it).
    let leaving: Vec<&str> = p
        .files
        .iter()
        .filter(|f| f.action != Stays)
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(leaving, vec!["/lib/z/A.ttf", "/lib/a/G2.ttf"]);
    assert_eq!(
        row(&p, "/lib/a/G2.ttf").kept.as_deref(),
        Some("/lib/z/G.ttc")
    );
    // Each file once, even the collection file of two sets.
    let mut paths: Vec<&str> = p.files.iter().map(|f| f.path.as_str()).collect();
    let n = paths.len();
    paths.dedup();
    assert_eq!(paths.len(), n);
    assert_eq!(p.my_languages, MyLanguagesStatus::Set);
}

#[test]
fn only_an_active_identical_copy_is_offered_to_move_its_activation() {
    let w = World::new();
    let p = w.plan(&request()).expect("plan");
    let b = row(&p, "/lib/z/B.ttf");
    assert_eq!(
        b.activation,
        Some(ActivationMove {
            to: "/lib/a/B.ttf".into(),
            mode: ActivationMode::Session,
            planned: false
        })
    );
    assert!(b.sentence.contains("move its activation"), "{}", b.sentence);
    assert_eq!(
        row(&p, "/lib/z/C.ttf").activation,
        None,
        "another version: no offer"
    );

    let moved = w
        .plan(&PlanCleanupRequest {
            move_activations: true,
            ..request()
        })
        .expect("plan");
    let b = row(&moved, "/lib/z/B.ttf");
    assert_eq!(b.action, CleanupAction::Trash);
    assert!(b.activation.as_ref().is_some_and(|a| a.planned));
    assert_eq!(
        outcome(&moved, "/lib/z/C.ttf"),
        (CleanupAction::Stays, Some(StayReason::Active)),
        "an active copy of another version stays, as today"
    );
}

#[test]
fn hiding_plans_hide_instead_of_the_trash() {
    let w = World::new();
    let p = w
        .plan(&PlanCleanupRequest {
            remove_as: RemoveAs::Hide,
            ..request()
        })
        .expect("plan");
    assert_eq!(row(&p, "/lib/z/A.ttf").action, CleanupAction::Hide);
    assert!(row(&p, "/lib/z/A.ttf").sentence.starts_with("Is hidden"));
}

#[test]
fn a_folder_scope_removes_only_the_copies_in_it() {
    let w = World::new();
    let p = w
        .plan(&PlanCleanupRequest {
            scope: CleanupScope::Folder {
                path: "/lib/a".into(),
            },
            keepers: vec![CleanupKeeper {
                set: "identical:a1".into(),
                keep: "/lib/z/A.ttf".into(),
            }],
            ..PlanCleanupRequest::default()
        })
        .expect("plan");
    assert_eq!(
        outcome(&p, "/lib/a/A.ttf"),
        (CleanupAction::Trash, None),
        "the user keeps the copy outside the folder"
    );
    // `H`: the policy keeps the copy in the folder; the other is outside.
    assert_eq!(
        outcome(&p, "/lib/z/H.ttf"),
        (CleanupAction::Stays, Some(StayReason::OutsideScope))
    );
}

#[test]
fn a_list_of_sets_plans_those_sets_only_and_refuses_an_unknown_one() {
    let w = World::new();
    let p = w
        .plan(&PlanCleanupRequest {
            scope: CleanupScope::Sets {
                ids: vec!["identical:a1".into()],
            },
            ..PlanCleanupRequest::default()
        })
        .expect("plan");
    assert_eq!(p.sets, 1);
    assert_eq!(p.files.len(), 2);
    let err = w
        .plan(&PlanCleanupRequest {
            scope: CleanupScope::Sets {
                ids: vec!["name:Nope".into()],
            },
            ..PlanCleanupRequest::default()
        })
        .unwrap_err();
    assert_eq!(err.code, fm_api::ErrorCode::InvalidRequest);
    let err = w
        .plan(&PlanCleanupRequest {
            keepers: vec![CleanupKeeper {
                set: "identical:a1".into(),
                keep: "/lib/a/B.ttf".into(),
            }],
            ..PlanCleanupRequest::default()
        })
        .unwrap_err();
    assert_eq!(err.code, fm_api::ErrorCode::InvalidRequest);
}

#[test]
fn a_collection_file_takes_the_keeper_of_its_other_set_as_also_kept() {
    let mut w = World::new();
    // Now the `G2` set removes the collection file too.
    w.files.retain(|f| f.path != "/lib/a/G2.ttf");
    w.files
        .push(file("/lib/a/G2.ttf", "g2", &[(14, "G2", "3.0")], false));
    let p = w.plan(&request()).expect("plan");
    let g = row(&p, "/lib/z/G.ttc");
    assert_eq!(g.action, CleanupAction::Trash, "{g:#?}");
    assert_eq!(g.kept.as_deref(), Some("/lib/a/G1.ttf"));
    assert_eq!(g.also_kept, vec!["/lib/a/G2.ttf".to_owned()]);
}

#[test]
fn a_folder_pair_removes_only_the_copies_on_the_side_to_remove() {
    let w = World::new();
    let p = w
        .plan(&PlanCleanupRequest {
            scope: CleanupScope::FolderPair {
                keep: "/lib/a".into(),
                remove: "/lib/z".into(),
                other_levels: false,
            },
            ..PlanCleanupRequest::default()
        })
        .expect("plan");
    // `A`: the copy on the side to remove goes, the copy kept for it stays.
    assert_eq!(outcome(&p, "/lib/z/A.ttf"), (CleanupAction::Trash, None));
    assert_eq!(
        row(&p, "/lib/z/A.ttf").kept.as_deref(),
        Some("/lib/a/A.ttf")
    );
    assert_eq!(
        outcome(&p, "/lib/a/A.ttf"),
        (CleanupAction::Stays, Some(StayReason::Keeper))
    );
    // Nothing on the side to keep ever leaves.
    assert!(
        p.files
            .iter()
            .filter(|f| f.path.starts_with("/lib/a/"))
            .all(|f| f.action == CleanupAction::Stays),
        "{:#?}",
        p.files
    );
    // `B`: active, its activation is offered, not moved.
    assert_eq!(
        outcome(&p, "/lib/z/B.ttf"),
        (CleanupAction::Stays, Some(StayReason::Active))
    );
    assert!(
        row(&p, "/lib/z/B.ttf")
            .activation
            .as_ref()
            .is_some_and(|a| !a.planned)
    );
    // `C`: another version, and the pair compares identical copies only.
    assert_eq!(
        outcome(&p, "/lib/z/C.ttf"),
        (CleanupAction::Stays, Some(StayReason::OtherLevel))
    );
    // `D`: its other copy is a system one, not on the side to keep.
    assert_eq!(
        outcome(&p, "/lib/z/D.ttf"),
        (CleanupAction::Stays, Some(StayReason::OnlyHere))
    );
    // `H`: not on disk any more.
    assert_eq!(
        outcome(&p, "/lib/z/H.ttf"),
        (CleanupAction::Stays, Some(StayReason::NotThere))
    );
}

#[test]
fn a_source_scope_plans_the_sets_with_a_copy_in_that_source() {
    let mut w = World::new();
    for f in &mut w.files {
        if f.path == "/lib/z/A.ttf" {
            f.source = SourceId(2);
        }
    }
    let p = w
        .plan(&PlanCleanupRequest {
            scope: CleanupScope::Source {
                source: SourceId(2),
            },
            ..PlanCleanupRequest::default()
        })
        .expect("plan");
    assert_eq!(p.sets, 1, "{:#?}", p.files);
    assert_eq!(outcome(&p, "/lib/z/A.ttf"), (CleanupAction::Trash, None));
    assert_eq!(row(&p, "/lib/a/A.ttf").action, CleanupAction::Stays);
    assert!(
        p.files.iter().all(|f| f.path.ends_with("/A.ttf")),
        "{:#?}",
        p.files
    );
}
