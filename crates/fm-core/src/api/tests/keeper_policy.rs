//! The keeper policy through the API (E56.4): `list_duplicate_sets`,
//! `list_duplicates`, `list_problems` and `preview_keeper_policy` read the
//! saved policy and the language guard, over fixture fonts.

use std::collections::BTreeSet;

use fm_api::library::{
    DuplicateGroup, KeeperOutcome, KeeperPolicy, KeeperRule, KeptBecause,
    PreviewKeeperPolicyRequest,
};
use fm_api::settings::MyLanguagesStatus;

use super::*;

/// The library with its folder added and scanned.
fn scan(lib: &Lib) -> SourceId {
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    source.id
}

fn paths(group: &DuplicateGroup) -> BTreeSet<String> {
    group.copies.iter().map(|c| c.path.clone()).collect()
}

#[test]
fn with_no_language_the_default_policy_gives_the_keeper_of_choose_keeper_for_every_set() {
    let lib = Lib::new("api-keeper-default");
    let ttf = lib.root.join("bebasneue").join("BebasNeue-Regular.ttf");
    // An identical copy, and the same font as WOFF2 in a folder that sorts
    // first: the format rule, not the path, must put the TTF first.
    let copy = lib.root.join("copy").join("BebasNeue-Regular.ttf");
    let woff2 = lib.root.join("a-web").join("BebasNeue-Regular.woff2");
    for p in [&copy, &woff2] {
        std::fs::create_dir_all(p.parent().expect("folder")).expect("folder");
    }
    std::fs::copy(&ttf, &copy).expect("copy");
    let sfnt = crate::web_font::standalone(&std::fs::read(&ttf).expect("read"), 0).expect("sfnt");
    std::fs::write(
        &woff2,
        crate::web_font::encode(&sfnt, fm_api::export::WebFormat::Woff2).expect("woff2"),
    )
    .expect("write");
    scan(&lib);

    let groups = lib.api.list_duplicates().expect("duplicates");
    assert!(
        groups
            .iter()
            .any(|g| g.copies.iter().any(|c| Path::new(&c.path) == woff2)),
        "the WOFF2 is a duplicate: {groups:?}"
    );
    let sets = lib
        .api
        .list_duplicate_sets(ListDuplicateSetsRequest {
            page: Page {
                offset: 0,
                limit: 500,
            },
            ..Default::default()
        })
        .expect("sets")
        .items;
    let mut compared = 0;
    for group in &groups {
        let mut expected = group.clone();
        expected.choose_keeper();
        assert_eq!(group.copies[0].path, expected.copies[0].path, "{group:?}");
        assert!(group.copies[0].keep);
        if let Some(set) = sets.iter().find(|s| {
            s.copies
                .iter()
                .map(|c| c.path.clone())
                .collect::<BTreeSet<_>>()
                == paths(group)
        }) {
            let rec = set.recommendation.as_ref().expect("recommendation");
            assert_eq!(
                rec.keeper.as_deref(),
                Some(expected.copies[0].path.as_str())
            );
            assert_eq!(rec.my_languages, MyLanguagesStatus::NotSet);
            compared += 1;
        }
    }
    assert!(compared >= 1, "a set that list_duplicates also lists");
    let bebas = groups
        .iter()
        .find(|g| g.copies.iter().any(|c| Path::new(&c.path) == woff2))
        .expect("the Bebas group");
    assert_ne!(
        Path::new(&bebas.copies[0].path),
        woff2,
        "TrueType before WOFF2"
    );
}

#[test]
fn my_languages_and_the_guard_decide_the_keeper_in_every_listing() {
    let lib = Lib::new("api-keeper-guard");
    let original = lib.root.join("asapsharp").join("AsapSharp[wdth,wght].ttf");
    // The same font cut down to ASCII, in a folder that sorts first.
    let cut = lib.root.join("a-cut").join("AsapSharp.ttf");
    std::fs::create_dir_all(cut.parent().expect("folder")).expect("folder");
    let sfnt =
        crate::web_font::standalone(&std::fs::read(&original).expect("read"), 0).expect("sfnt");
    let ascii = crate::subset::cut(&sfnt, &crate::subset::Keep::from_ranges([(0x00, 0x7E)]))
        .expect("subset");
    std::fs::write(&cut, crate::web_font::sfnt_bytes(&ascii).expect("bytes")).expect("write");
    let source = scan(&lib);
    let set = || {
        lib.api
            .list_duplicate_sets(ListDuplicateSetsRequest::default())
            .expect("sets")
            .items
            .into_iter()
            .find(|s| s.copies.iter().any(|c| Path::new(&c.path) == original))
            .expect("the set of the cut font")
    };
    let group = || {
        lib.api
            .list_duplicates()
            .expect("duplicates")
            .into_iter()
            .find(|g| g.copies.iter().any(|c| Path::new(&c.path) == original))
            .expect("the group of the cut font")
    };
    let problem_paths = || -> Vec<String> {
        lib.api
            .list_problems()
            .expect("problems")
            .into_iter()
            .filter(|p| p.kind == ProblemKind::DuplicateFace)
            .map(|p| p.path)
            .collect()
    };
    let original_path = original.display().to_string();
    let cut_path = cut.display().to_string();

    // No language: the path decides.
    let rec = set().recommendation.expect("recommendation");
    assert_eq!(rec.keeper.as_deref(), Some(cut_path.as_str()));
    assert_eq!(rec.reason.as_ref().map(|r| r.rule), Some(KeeperRule::Path));
    assert_eq!(group().copies[0].path, cut_path);
    assert!(problem_paths().contains(&original_path));

    // German set: the copy that covers it is the keeper, with its reason.
    lib.api
        .update_settings(SettingsUpdate {
            my_languages: Some(vec!["de".into()]),
            ..Default::default()
        })
        .expect("languages");
    let rec = set().recommendation.expect("recommendation");
    assert_eq!(rec.keeper.as_deref(), Some(original_path.as_str()));
    let reason = rec.reason.expect("reason");
    assert_eq!(reason.rule, KeeperRule::MyLanguages);
    assert_eq!(
        reason.sentence,
        "Kept: covers German, the other copy does not."
    );
    assert_eq!(group().copies[0].path, original_path);
    let problems = problem_paths();
    assert!(problems.contains(&cut_path) && !problems.contains(&original_path));

    // A policy without the language rule: the path picks the cut copy, but
    // the guard keeps the only copy with German, and neither is a problem.
    let saved = lib
        .api
        .update_settings(SettingsUpdate {
            keeper_policy: Some(KeeperPolicy {
                rules: vec![KeeperRule::Path, KeeperRule::NewestVersion],
                ..KeeperPolicy::default()
            }),
            ..Default::default()
        })
        .expect("policy");
    assert_eq!(
        saved.keeper_policy.rules,
        [KeeperRule::NewestVersion, KeeperRule::Path],
        "stored normalized"
    );
    let file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(lib.paths.config_file()).expect("file"))
            .expect("json");
    assert_eq!(
        file["keeperPolicy"]["rules"],
        serde_json::json!(["newestVersion", "path"])
    );
    let rec = set().recommendation.expect("recommendation");
    assert_eq!(rec.keeper.as_deref(), Some(cut_path.as_str()));
    assert_eq!(rec.outcome, KeeperOutcome::Keeper);
    assert!(rec.remove.is_empty(), "{rec:?}");
    let guarded = rec
        .keep
        .iter()
        .find(|k| k.path == original_path)
        .expect("the guarded copy");
    assert_eq!(guarded.because, KeptBecause::Languages);
    assert_eq!(guarded.languages_only_here, ["de"]);
    let g = group();
    assert_eq!(g.copies[0].path, cut_path);
    assert!(g.copies.iter().all(|c| c.keep), "{g:?}");
    let problems = problem_paths();
    assert!(!problems.contains(&cut_path) && !problems.contains(&original_path));

    // A preview of the default policy changes this set back; nothing saves.
    let preview = lib
        .api
        .preview_keeper_policy(PreviewKeeperPolicyRequest {
            scope: DuplicateScope::Source { source },
            policy: KeeperPolicy::default(),
        })
        .expect("preview");
    assert!(preview.sets >= 1);
    assert!(preview.changed >= 1, "{preview:?}");
    assert_eq!(preview.my_languages, MyLanguagesStatus::Set);
    assert_eq!(
        preview.with_keeper + preview.keep_both,
        preview.sets,
        "no system copies here"
    );
    let same = lib
        .api
        .preview_keeper_policy(PreviewKeeperPolicyRequest {
            scope: DuplicateScope::Library,
            policy: saved.keeper_policy.clone(),
        })
        .expect("preview");
    assert_eq!(same.changed, 0);
    assert_eq!(
        lib.api.get_settings().expect("settings").keeper_policy,
        saved.keeper_policy
    );

    let err = lib
        .api
        .preview_keeper_policy(PreviewKeeperPolicyRequest {
            scope: DuplicateScope::Source {
                source: SourceId(9999),
            },
            policy: KeeperPolicy::default(),
        })
        .expect_err("unknown source");
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = lib
        .api
        .preview_keeper_policy(PreviewKeeperPolicyRequest {
            scope: DuplicateScope::Library,
            policy: KeeperPolicy {
                preferred_folders: vec!["relative/folder".into()],
                ..KeeperPolicy::default()
            },
        })
        .expect_err("a relative folder");
    assert_eq!(err.code, ErrorCode::InvalidRequest);
}
