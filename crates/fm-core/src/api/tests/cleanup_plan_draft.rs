//! The cleanup draft as one plan through the API (E56.15): the choices per
//! copy, an answer that moves nothing, and the draft after the apply; and
//! the counts of the ways into the module.

use fm_api::cleanup::{ApplyCleanupRequest, CleanupAction, DuplicateAnswer, StayReason};
use fm_api::cleanup_draft::{
    CleanupDraftUpdate, CopyChoice, CopyDecision, DuplicateSummaryRequest, PlanDraftRequest,
    SetDecision,
};

use super::cleanup::{apply, library, session};
use super::*;

/// The set whose copies include `file` (a file name of [`super::cleanup::COPIED`]).
fn set_of(lib: &Lib, file: &str) -> DuplicateSet {
    lib.api
        .list_duplicate_sets(ListDuplicateSetsRequest::default())
        .expect("sets")
        .items
        .into_iter()
        .find(|s| s.copies.iter().any(|c| c.path.ends_with(file)))
        .unwrap_or_else(|| panic!("a set of {file}"))
}

fn decide(lib: &Lib, set: &DuplicateSet, decision: SetDecision) {
    lib.api
        .update_cleanup_draft(CleanupDraftUpdate::Decide {
            set: set.id.clone(),
            decision,
        })
        .expect("decide");
}

/// Keeps the copy outside `zcopy/` and gives the one in it `choice`.
fn zcopy(set: &DuplicateSet, choice: CopyChoice) -> SetDecision {
    SetDecision::Copies {
        copies: set
            .copies
            .iter()
            .map(|c| CopyDecision {
                path: c.path.clone(),
                choice: if c.path.contains("zcopy") {
                    choice
                } else {
                    CopyChoice::Keep
                },
            })
            .collect(),
    }
}

#[test]
fn the_draft_is_one_plan_and_applying_it_empties_the_draft() {
    let dir = TempDir::new("api-plan-draft-trash");
    let (lib, made) = library(
        "api-plan-draft",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let bebas = set_of(&lib, "BebasNeue-Regular.ttf");
    let barlow = set_of(&lib, "Barlow-BoldItalic.ttf");
    let archivo = set_of(&lib, "Archivo-Bold.woff2");
    decide(&lib, &bebas, zcopy(&bebas, CopyChoice::Trash));
    decide(&lib, &barlow, zcopy(&barlow, CopyChoice::Hide));
    decide(&lib, &archivo, SetDecision::KeepAll);

    let plan = lib
        .api
        .plan_cleanup_draft(PlanDraftRequest::default())
        .expect("plan");
    let row = |name: &str| {
        plan.files
            .iter()
            .find(|f| f.path.contains("zcopy") && f.path.ends_with(name))
            .unwrap_or_else(|| panic!("{name} in {plan:#?}"))
    };
    assert_eq!(row("BebasNeue-Regular.ttf").action, CleanupAction::Trash);
    assert_eq!(row("Barlow-BoldItalic.ttf").action, CleanupAction::Hide);
    assert!(
        plan.files
            .iter()
            .filter(|f| !f.path.contains("zcopy"))
            .all(|f| f.reason == Some(StayReason::Keeper)),
        "{plan:#?}"
    );
    assert!(
        !plan.files.iter().any(|f| f.set == archivo.id),
        "keep both moves nothing"
    );
    assert_eq!(plan.answers.len(), 1);
    assert_eq!(plan.answers[0].set, archivo.id);
    assert_eq!(plan.answers[0].answer, DuplicateAnswer::KeepBoth);
    let bebas_copy = made
        .iter()
        .find(|p| p.ends_with("BebasNeue-Regular.ttf"))
        .expect("made");
    let size = u32::try_from(std::fs::metadata(bebas_copy).expect("size").len()).expect("small");
    assert_eq!(plan.frees, size, "only the trashed copy frees space");
    assert!(made.iter().all(|p| p.is_file()), "a plan touches nothing");

    let (_job, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        },
    );
    let s = session(&lib, &events);
    assert_eq!(s.trashed, 1, "{s:#?}");
    assert_eq!(s.hidden, 1, "{s:#?}");
    assert!(!bebas_copy.exists(), "the trashed copy is in the trash");
    let draft = lib.api.get_cleanup_draft().expect("draft");
    assert!(draft.decisions.is_empty(), "{draft:#?}");
    let answers = lib.api.list_duplicate_decisions().expect("decisions");
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].set, archivo.id);
}

#[test]
fn the_summary_counts_the_sets_the_copies_of_a_scope_and_the_families() {
    let dir = TempDir::new("api-dup-summary-trash");
    let (lib, _made) = library(
        "api-dup-summary",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let all = lib
        .api
        .duplicate_summary(DuplicateSummaryRequest::default())
        .expect("summary");
    assert_eq!(all.fonts, 3, "{all:#?}");
    assert_eq!(all.copies, 6, "{all:#?}");
    assert_eq!(all.families.len(), 3, "{all:#?}");
    assert!(
        all.families
            .iter()
            .all(|f| f.copies == 2 && f.sets.len() == 1)
    );
    // Each family id is the one the library lists.
    let families = lib
        .api
        .list_families(ListFamiliesRequest {
            page: Page {
                offset: 0,
                limit: 500,
            },
            ..Default::default()
        })
        .expect("families");
    for f in &all.families {
        assert!(
            families.items.iter().any(|x| x.id == f.family),
            "{f:?} is a listed family"
        );
    }
    let zcopy = lib
        .api
        .duplicate_summary(DuplicateSummaryRequest {
            scope: DuplicateScope::Folder {
                path: lib.root.join("zcopy").display().to_string(),
            },
        })
        .expect("summary");
    assert_eq!(zcopy.fonts, 3);
    assert_eq!(zcopy.copies, 3, "only the copies in the folder");
    assert!(
        lib.api
            .duplicate_summary(DuplicateSummaryRequest {
                scope: DuplicateScope::Source {
                    source: SourceId(999)
                },
            })
            .is_err(),
        "an unknown source is NotFound"
    );
}
