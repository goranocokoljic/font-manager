//! The Duplicates module through the API (E56.11): the queue's filters and
//! orders, the overview, and the draft plan that a decided set leaves the
//! queue for, over fixture fonts.

use fm_api::cleanup_draft::{
    CleanupDraftUpdate, CopyChoice, CopyDecision, DuplicatesOverviewRequest, SetDecision,
};
use fm_api::common::FontFormat;
use fm_api::library::{DuplicateLevel, DuplicateSetFilter, DuplicateSetSort, RecommendationFilter};

use super::*;

/// The library with two sets: Bebas Neue three times as the same file (an
/// identical set) and Barlow as TTF and WOFF2 (the same version, another
/// file).
fn library(label: &str) -> Lib {
    let lib = Lib::new(label);
    let ttf = lib.root.join("bebasneue").join("BebasNeue-Regular.ttf");
    for dir in ["copy-a", "copy-b"] {
        let to = lib.root.join(dir).join("BebasNeue-Regular.ttf");
        std::fs::create_dir_all(to.parent().expect("folder")).expect("folder");
        std::fs::copy(&ttf, &to).expect("copy");
    }
    let barlow = lib.root.join("barlow").join("Barlow-BoldItalic.ttf");
    let woff2 = lib.root.join("web").join("Barlow-BoldItalic.woff2");
    std::fs::create_dir_all(woff2.parent().expect("folder")).expect("folder");
    let sfnt =
        crate::web_font::standalone(&std::fs::read(&barlow).expect("read"), 0).expect("sfnt");
    std::fs::write(
        &woff2,
        crate::web_font::encode(&sfnt, fm_api::export::WebFormat::Woff2).expect("woff2"),
    )
    .expect("write");
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    lib
}

fn list(lib: &Lib, filter: DuplicateSetFilter, sort: DuplicateSetSort) -> Vec<DuplicateSet> {
    let page = lib
        .api
        .list_duplicate_sets(ListDuplicateSetsRequest {
            page: Page {
                offset: 0,
                limit: 500,
            },
            filter,
            sort,
            ..Default::default()
        })
        .expect("sets");
    assert_eq!(page.total as usize, page.items.len());
    page.items
}

fn ids(sets: &[DuplicateSet]) -> Vec<String> {
    sets.iter().map(|s| s.id.clone()).collect()
}

#[test]
fn the_queue_filters_by_level_format_and_recommendation_and_sorts() {
    let lib = library("api-module-queue");
    let all = list(&lib, DuplicateSetFilter::default(), DuplicateSetSort::Name);
    assert_eq!(all.len(), 2, "{all:?}");
    let identical = all
        .iter()
        .find(|s| s.level == DuplicateLevel::Identical)
        .expect("the identical set");
    let barlow = all
        .iter()
        .find(|s| s.level == DuplicateLevel::SameVersion)
        .expect("the Barlow set");
    assert!(
        all.iter()
            .all(|s| s.copies.iter().all(|c| c.facts.is_some()))
    );

    let by_level = |levels: Vec<DuplicateLevel>| {
        ids(&list(
            &lib,
            DuplicateSetFilter {
                levels,
                ..Default::default()
            },
            DuplicateSetSort::Name,
        ))
    };
    assert_eq!(
        by_level(vec![DuplicateLevel::Identical]),
        vec![identical.id.clone()]
    );
    assert_eq!(
        by_level(vec![
            DuplicateLevel::SameVersion,
            DuplicateLevel::OtherVersion
        ]),
        vec![barlow.id.clone()]
    );
    let woff2 = list(
        &lib,
        DuplicateSetFilter {
            format: Some(FontFormat::Woff2),
            ..Default::default()
        },
        DuplicateSetSort::Name,
    );
    assert_eq!(ids(&woff2), vec![barlow.id.clone()]);

    // Both sets get a keeper and a copy to remove; neither is "keep both".
    let with = |r| {
        list(
            &lib,
            DuplicateSetFilter {
                recommendation: Some(r),
                ..Default::default()
            },
            DuplicateSetSort::Name,
        )
    };
    let recommended = with(RecommendationFilter::HasRecommendation);
    assert_eq!(recommended.len(), 2);
    assert!(recommended.iter().all(|s| s.recommendation.is_some()));
    assert!(with(RecommendationFilter::KeepBoth).is_empty());

    let by_copies = list(
        &lib,
        DuplicateSetFilter::default(),
        DuplicateSetSort::Copies,
    );
    assert_eq!(by_copies[0].id, identical.id, "three copies before two");
    let by_frees = list(&lib, DuplicateSetFilter::default(), DuplicateSetSort::Frees);
    let frees: Vec<u32> = by_frees.iter().map(DuplicateSet::frees).collect();
    assert!(frees[0] >= frees[1], "{frees:?}");
    assert!(frees[1] > 0);
    assert!(
        by_frees
            .iter()
            .all(|s| s.copies.iter().all(|c| c.facts.is_some())),
        "a page sorted by the recommendation still carries the facts"
    );
}

#[test]
fn a_decided_set_leaves_the_queue_and_the_draft_is_stored() {
    let lib = library("api-module-draft");
    let queue = || {
        list(
            &lib,
            DuplicateSetFilter {
                undecided: true,
                ..Default::default()
            },
            DuplicateSetSort::Name,
        )
    };
    let sets = queue();
    assert_eq!(sets.len(), 2);
    let barlow = sets
        .iter()
        .find(|s| s.level == DuplicateLevel::SameVersion)
        .expect("Barlow")
        .clone();
    let woff2 = barlow
        .copies
        .iter()
        .find(|c| c.path.ends_with(".woff2"))
        .expect("the WOFF2")
        .path
        .clone();
    let draft = lib
        .api
        .update_cleanup_draft(CleanupDraftUpdate::Decide {
            set: barlow.id.clone(),
            decision: SetDecision::Copies {
                copies: vec![CopyDecision {
                    path: woff2.clone(),
                    choice: CopyChoice::Hide,
                }],
            },
        })
        .expect("decide");
    assert_eq!(draft.decisions.len(), 1);
    assert_eq!(draft.decisions[0].level, DuplicateLevel::SameVersion);
    assert_eq!(queue().len(), 1, "the decided set left the queue");
    assert_eq!(
        list(&lib, DuplicateSetFilter::default(), DuplicateSetSort::Name).len(),
        2,
        "without `undecided` every set is listed"
    );
    assert_eq!(lib.api.get_cleanup_draft().expect("draft"), draft, "stored");

    // Nothing moved: the file is where it was.
    assert!(Path::new(&woff2).exists());

    let refused = lib.api.update_cleanup_draft(CleanupDraftUpdate::Decide {
        set: barlow.id.clone(),
        decision: SetDecision::Copies {
            copies: barlow
                .copies
                .iter()
                .map(|c| CopyDecision {
                    path: c.path.clone(),
                    choice: CopyChoice::Trash,
                })
                .collect(),
        },
    });
    assert_eq!(
        refused.expect_err("no copy kept").code,
        fm_api::ErrorCode::InvalidRequest
    );
    let gone = lib.api.update_cleanup_draft(CleanupDraftUpdate::Decide {
        set: "name:NoSuchFont".to_owned(),
        decision: SetDecision::KeepAll,
    });
    assert_eq!(
        gone.expect_err("unknown set").code,
        fm_api::ErrorCode::NotFound
    );

    lib.api
        .update_cleanup_draft(CleanupDraftUpdate::TakeBack { set: barlow.id })
        .expect("take back");
    assert_eq!(queue().len(), 2, "back in the queue");
}

#[test]
fn the_overview_counts_the_sets_the_extra_files_and_the_space() {
    let lib = library("api-module-overview");
    let overview = lib
        .api
        .duplicates_overview(DuplicatesOverviewRequest::default())
        .expect("overview");
    let ttf = std::fs::metadata(lib.root.join("bebasneue").join("BebasNeue-Regular.ttf"))
        .expect("size")
        .len();
    assert_eq!(overview.fonts, 2);
    assert_eq!(overview.identical.sets, 1);
    assert_eq!(overview.identical.files, 2, "two of three copies leave");
    assert_eq!(u64::from(overview.identical.frees), 2 * ttf);
    assert_eq!(overview.same_version.sets, 1);
    assert_eq!(overview.same_version.files, 1);
    assert_eq!(overview.other_version.sets, 0);
    assert_eq!(overview.extra_files, 3);
    assert_eq!(overview.keep_both, 0);
    assert_eq!(
        overview.my_languages,
        fm_api::settings::MyLanguagesStatus::NotSet
    );
    let missing = lib.api.duplicates_overview(DuplicatesOverviewRequest {
        scope: fm_api::library::DuplicateScope::Source {
            source: SourceId(999),
        },
    });
    assert_eq!(
        missing.expect_err("no source").code,
        fm_api::ErrorCode::NotFound
    );
}
