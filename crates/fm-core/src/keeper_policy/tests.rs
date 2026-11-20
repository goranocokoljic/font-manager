use fm_api::SourceId;
use fm_api::activation::ActivationMode;
use fm_api::library::{DuplicateCopy, DuplicateKind, DuplicateLevel, DuplicateSetCopy};

use super::*;
use crate::duplicate_sets::SetFace;

fn facts() -> CopyFacts {
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
        languages_full: Vec::new(),
        languages_locl: Vec::new(),
    }
}

fn copy(path: &str) -> Candidate {
    Candidate {
        path: path.to_owned(),
        version: Some("1.000".to_owned()),
        format: Some(FontFormat::Ttf),
        facts: Some(facts()),
        ..Candidate::default()
    }
}

fn covering(path: &str, languages: &[&str]) -> Candidate {
    let mut c = copy(path);
    if let Some(f) = c.facts.as_mut() {
        f.languages_full = languages.iter().map(|l| (*l).to_owned()).collect();
    }
    c
}

fn policy(rules: &[KeeperRule]) -> KeeperPolicy {
    KeeperPolicy {
        rules: rules.to_vec(),
        ..KeeperPolicy::default()
    }
}

fn langs(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|l| (*l).to_owned()).collect()
}

fn run(copies: &[Candidate], policy: &KeeperPolicy, my: &[&str]) -> KeeperRecommendation {
    let my = langs(my);
    let names = LanguageNames::load(&my).expect("names");
    recommend(copies, policy, &my, &names)
}

fn kept(rec: &KeeperRecommendation) -> Vec<(&str, KeptBecause)> {
    rec.keep
        .iter()
        .map(|k| (k.path.as_str(), k.because))
        .collect()
}

fn removed(rec: &KeeperRecommendation) -> Vec<&str> {
    rec.remove.iter().map(|r| r.path.as_str()).collect()
}

/// The keeper of `a` and `b` under the single rule `rule` (then the path),
/// and the rule that decided.
fn decide(rule: KeeperRule, a: Candidate, b: Candidate) -> (String, KeeperRule) {
    let rec = run(&[a, b], &policy(&[rule]), &["de", "sr-Cyrl"]);
    assert_eq!(rec.remove.len(), 1, "{rec:?}");
    let reason = rec.reason.expect("two ranked copies have a reason");
    (rec.keeper.expect("keeper"), reason.rule)
}

#[test]
fn each_rule_prefers_the_copy_it_names_over_one_that_sorts_first_by_path() {
    let with = |f: &dyn Fn(&mut Candidate)| {
        let mut c = copy("/z.ttf");
        f(&mut c);
        c
    };
    let fact = |f: &dyn Fn(&mut CopyFacts)| {
        with(&|c: &mut Candidate| {
            if let Some(x) = c.facts.as_mut() {
                f(x);
            }
        })
    };
    let cases: Vec<(KeeperRule, Candidate)> = vec![
        (
            KeeperRule::MyLanguages,
            fact(&|f| f.languages_full = langs(&["sr-Cyrl"])),
        ),
        (
            KeeperRule::MyLanguagesLocl,
            fact(&|f| f.languages_locl = langs(&["sr-Cyrl"])),
        ),
        (
            KeeperRule::NewestVersion,
            with(&|c| c.version = Some("Version 1.10".into())),
        ),
        (
            KeeperRule::Format,
            with(&|c| c.format = Some(FontFormat::Otf)),
        ),
        (KeeperRule::Variable, fact(&|f| f.variable = true)),
        (KeeperRule::MostGlyphs, fact(&|f| f.glyph_count = Some(900))),
        (
            KeeperRule::PreferredFolder,
            with(&|c| c.path = "/keep/z.ttf".into()),
        ),
        (
            KeeperRule::LibraryFolder,
            with(&|c| c.in_library_folder = true),
        ),
        (KeeperRule::UserMetadata, fact(&|f| f.favorite = true)),
        (
            KeeperRule::ActiveOrRequested,
            fact(&|f| f.activation_documents = vec!["Poster.indd".into()]),
        ),
    ];
    for (rule, winner) in cases {
        let mut other = copy("/a.ttf");
        other.version = Some("Version 1.9".into());
        if let Some(f) = other.facts.as_mut() {
            f.glyph_count = Some(300);
        }
        let mut rules = policy(&[rule]);
        rules.preferred_folders = vec!["/keep".into()];
        let rec = run(&[other, winner.clone()], &rules, &["de", "sr-Cyrl"]);
        assert_eq!(
            rec.keeper.as_deref(),
            Some(winner.path.as_str()),
            "{rule:?}"
        );
        assert_eq!(rec.reason.as_ref().map(|r| r.rule), Some(rule), "{rule:?}");
    }
}

#[test]
fn a_tie_goes_to_the_next_rule_and_a_won_rule_wins_the_set() {
    let mut otf = copy("/z.otf");
    otf.format = Some(FontFormat::Otf);
    let mut lib = copy("/a.ttf");
    lib.in_library_folder = true;
    // Same version: the format decides, though the TTF is in the library
    // folder and first by path.
    let rules = [
        KeeperRule::NewestVersion,
        KeeperRule::Format,
        KeeperRule::LibraryFolder,
    ];
    let (keeper, rule) = {
        let rec = run(&[lib.clone(), otf.clone()], &policy(&rules), &[]);
        (
            rec.keeper.expect("keeper"),
            rec.reason.expect("reason").rule,
        )
    };
    assert_eq!((keeper.as_str(), rule), ("/z.otf", KeeperRule::Format));
    // The library folder first: it wins the set, whatever comes after.
    let rec = run(
        &[otf, lib],
        &policy(&[KeeperRule::LibraryFolder, KeeperRule::Format]),
        &[],
    );
    assert_eq!(rec.keeper.as_deref(), Some("/a.ttf"));
}

#[test]
fn the_path_is_always_the_last_rule_and_decides_a_full_tie() {
    let p = policy(&[KeeperRule::Path, KeeperRule::NewestVersion]).normalized();
    assert_eq!(p.rules, [KeeperRule::NewestVersion, KeeperRule::Path]);
    assert_eq!(policy(&[]).normalized().rules, [KeeperRule::Path]);
    let (keeper, rule) = decide(KeeperRule::Format, copy("/b.ttf"), copy("/a.ttf"));
    assert_eq!((keeper.as_str(), rule), ("/a.ttf", KeeperRule::Path));
    // The same copies in another order give the same answer.
    let (again, _) = decide(KeeperRule::Format, copy("/a.ttf"), copy("/b.ttf"));
    assert_eq!(again, "/a.ttf");
}

#[test]
fn the_default_policy_with_no_language_is_the_keeper_rule_of_list_duplicates() {
    // Every order of these copies gives the same keeper as choose_keeper.
    let mut copies = vec![
        copy("/lib/b.woff2"),
        copy("/c.ttf"),
        copy("/a.ttf"),
        copy("/lib/d.otf"),
    ];
    copies[0].format = Some(FontFormat::Woff2);
    copies[0].in_library_folder = true;
    copies[3].format = Some(FontFormat::Otf);
    copies[3].version = Some("0.9".into());
    copies[3].in_library_folder = true;
    let rec = run(&copies, &KeeperPolicy::default(), &[]);
    let mut group = DuplicateGroup {
        kind: DuplicateKind::Near,
        postscript_name: None,
        copies: copies
            .iter()
            .map(|c| DuplicateCopy {
                path: c.path.clone(),
                size: 1,
                hash: String::new(),
                face: None,
                version: c.version.clone(),
                source: SourceId(1),
                in_library_folder: c.in_library_folder,
                keep: false,
            })
            .collect(),
    };
    group.choose_keeper();
    assert_eq!(rec.keeper.as_deref(), Some(group.copies[0].path.as_str()));
    assert_eq!(
        rec.keeper.as_deref(),
        Some("/a.ttf"),
        "TrueType before WOFF2"
    );
    assert_eq!(rec.my_languages, MyLanguagesStatus::NotSet);
}

#[test]
fn the_reason_names_the_language_the_other_copy_lacks() {
    let rec = run(
        &[
            covering("/a.ttf", &["de"]),
            covering("/b.ttf", &["de", "sr-Cyrl"]),
        ],
        &KeeperPolicy::default(),
        &["sr-Cyrl", "de"],
    );
    let reason = rec.reason.clone().expect("reason");
    assert_eq!(rec.keeper.as_deref(), Some("/b.ttf"));
    assert_eq!(reason.rule, KeeperRule::MyLanguages);
    assert_eq!(reason.kept, ["sr-Cyrl", "de"]);
    assert_eq!(reason.other, ["de"]);
    assert_eq!(reason.other_path, "/a.ttf");
    assert_eq!(
        reason.sentence,
        "Kept: covers Serbian (Cyrillic), the other copy does not."
    );
    assert_eq!(rec.keep[0].sentence, reason.sentence);
    assert_eq!(rec.remove[0].reason, reason);
}

#[test]
fn every_rule_has_a_sentence_that_names_its_values() {
    let mut newer = copy("/b.ttf");
    newer.version = Some("2.000".into());
    let reason = run(&[copy("/a.ttf"), newer], &KeeperPolicy::default(), &[])
        .reason
        .expect("reason");
    assert_eq!(reason.rule, KeeperRule::NewestVersion);
    assert_eq!(
        (reason.kept.as_slice(), reason.other.as_slice()),
        (&["2.000".to_owned()][..], &["1.000".to_owned()][..])
    );
    assert_eq!(
        reason.sentence,
        "Kept: version 2.000, the other copy has version 1.000."
    );

    let mut otf = copy("/b.otf");
    otf.format = Some(FontFormat::Otf);
    let reason = run(&[copy("/a.ttf"), otf], &KeeperPolicy::default(), &[])
        .reason
        .expect("reason");
    assert_eq!(reason.kept, ["OpenType CFF"]);
    assert_eq!(
        reason.sentence,
        "Kept: OpenType CFF comes before TrueType in your order of formats."
    );

    let mut meta = copy("/b.ttf");
    if let Some(f) = meta.facts.as_mut() {
        f.favorite = true;
        f.tags = vec!["Display".into()];
    }
    let reason = run(
        &[copy("/a.ttf"), meta],
        &policy(&[KeeperRule::UserMetadata]),
        &[],
    )
    .reason
    .expect("reason");
    assert_eq!(reason.kept, ["favorite", "tags"]);
    assert_eq!(
        reason.sentence,
        "Kept: carries your favorite and tags, the other copy none of it."
    );
}

#[test]
fn the_guard_keeps_the_only_copy_of_a_language_whatever_the_order() {
    // The rules prefer the newer copy, and the languages are not a rule at
    // all; still the older copy, the only one with Serbian, is kept.
    let mut newer = covering("/new.ttf", &["de"]);
    newer.version = Some("2.000".into());
    let older = covering("/old.ttf", &["de", "sr-Cyrl"]);
    let plain = covering("/plain.ttf", &[]);
    let rec = run(
        &[newer, older, plain],
        &policy(&[KeeperRule::NewestVersion]),
        &["de", "sr-Cyrl"],
    );
    assert_eq!(rec.keeper.as_deref(), Some("/new.ttf"));
    assert_eq!(
        kept(&rec),
        [
            ("/new.ttf", KeptBecause::Policy),
            ("/old.ttf", KeptBecause::Languages)
        ]
    );
    assert_eq!(rec.keep[1].languages_only_here, ["sr-Cyrl"]);
    // Another copy covers everything: one keeper, not "keep both".
    assert_eq!(rec.outcome, KeeperOutcome::Keeper);
    // A copy that covers nothing of its own is not protected.
    assert_eq!(removed(&rec), ["/plain.ttf"]);
}

#[test]
fn a_set_where_no_copy_covers_everything_is_keep_both_with_each_sides_languages() {
    let rec = run(
        &[
            covering("/a.ttf", &["de"]),
            covering("/b.ttf", &["sr-Cyrl"]),
        ],
        &KeeperPolicy::default(),
        &["sr-Cyrl", "de"],
    );
    assert_eq!(rec.outcome, KeeperOutcome::KeepBoth);
    assert!(rec.remove.is_empty());
    let only: Vec<(&str, &[String])> = rec
        .keep
        .iter()
        .map(|k| (k.path.as_str(), k.languages_only_here.as_slice()))
        .collect();
    assert_eq!(
        only,
        [
            ("/a.ttf", &langs(&["de"])[..]),
            ("/b.ttf", &langs(&["sr-Cyrl"])[..])
        ]
    );
    assert_eq!(
        rec.keep[1].sentence,
        "Kept: covers Serbian (Cyrillic), the other copies kept do not."
    );
}

#[test]
fn with_no_language_set_the_guard_and_the_language_rules_do_nothing() {
    let rec = run(
        &[
            covering("/b.ttf", &["sr-Cyrl"]),
            covering("/a.ttf", &["de"]),
        ],
        &policy(&[KeeperRule::MyLanguages]),
        &[],
    );
    assert_eq!(rec.my_languages, MyLanguagesStatus::NotSet);
    assert_eq!(rec.outcome, KeeperOutcome::Keeper);
    assert_eq!(rec.keeper.as_deref(), Some("/a.ttf"), "decided by the path");
    assert_eq!(removed(&rec), ["/b.ttf"]);
}

#[test]
fn a_language_outside_mine_does_not_protect_a_copy() {
    let rec = run(
        &[
            covering("/a.ttf", &["de"]),
            covering("/b.ttf", &["de", "fr"]),
        ],
        &policy(&[KeeperRule::Path]),
        &["de"],
    );
    assert_eq!(rec.keeper.as_deref(), Some("/a.ttf"));
    assert_eq!(removed(&rec), ["/b.ttf"]);
}

#[test]
fn system_copies_are_outside_the_policy_and_always_kept() {
    let mut system = copy("/a-system.ttf");
    system.system = true;
    system.version = Some("9.0".into());
    let rec = run(
        &[system.clone(), copy("/b.ttf")],
        &KeeperPolicy::default(),
        &[],
    );
    assert_eq!(
        rec.keeper.as_deref(),
        Some("/b.ttf"),
        "never the system copy"
    );
    assert_eq!(
        kept(&rec),
        [
            ("/b.ttf", KeptBecause::Policy),
            ("/a-system.ttf", KeptBecause::System)
        ]
    );
    assert!(rec.remove.is_empty());
    assert!(rec.reason.is_none(), "one copy is ranked");

    let mut other = system.clone();
    other.path = "/b-system.ttf".into();
    let rec = run(&[system, other], &KeeperPolicy::default(), &[]);
    assert_eq!(rec.outcome, KeeperOutcome::AllSystem);
    assert_eq!(rec.keeper, None);
    assert_eq!(rec.keep.len(), 2);
    assert!(rec.remove.is_empty());
}

#[test]
fn a_file_that_alone_holds_a_font_is_never_recommended_for_removal() {
    let mut ttc = copy("/z.ttc");
    ttc.holds = vec![HeldFont {
        name: "Inter-Bold".into(),
        holders: vec!["/z.ttc".into()],
    }];
    let rec = run(
        &[copy("/a.ttf"), ttc, copy("/m.ttf")],
        &KeeperPolicy::default(),
        &[],
    );
    assert_eq!(
        kept(&rec),
        [
            ("/a.ttf", KeptBecause::Policy),
            ("/z.ttc", KeptBecause::LastCopy)
        ]
    );
    assert_eq!(rec.keep[1].fonts_only_here, ["Inter-Bold"]);
    assert_eq!(
        rec.keep[1].sentence,
        "Kept: its file is the last copy of Inter-Bold."
    );
    assert_eq!(
        removed(&rec),
        ["/m.ttf"],
        "a copy that holds nothing else may go"
    );
}

fn file(path: &str, names: &[&str]) -> SetFile {
    SetFile {
        path: path.to_owned(),
        size: 1,
        hash: path.to_owned(),
        format: Some(FontFormat::Ttf),
        source: SourceId(1),
        in_library_folder: false,
        system: false,
        faces: names
            .iter()
            .enumerate()
            .map(|(i, n)| SetFace {
                face: FaceId(u32::try_from(i).unwrap_or_default()),
                name: (*n).to_owned(),
                version: None,
            })
            .collect(),
    }
}

#[test]
fn holdings_name_every_file_that_holds_each_font() {
    let files = [
        file("/a.ttc", &["Inter-Regular", "Inter-Bold"]),
        file("/b.ttf", &["Inter-Regular"]),
        file("/c.ttf", &[""]),
    ];
    let held = holdings(&files);
    let of = |path: &str| -> Vec<(String, Vec<String>)> {
        held.get(path)
            .map(|h| {
                h.iter()
                    .map(|f| (f.name.clone(), f.holders.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        of("/a.ttc"),
        [
            ("Inter-Bold".to_owned(), vec!["/a.ttc".to_owned()]),
            (
                "Inter-Regular".to_owned(),
                vec!["/a.ttc".to_owned(), "/b.ttf".to_owned()]
            ),
        ]
    );
    assert!(of("/c.ttf").is_empty(), "a face without a name is no font");
}

#[test]
fn when_every_holder_of_a_font_would_go_the_first_ranked_one_is_kept() {
    // Two identical collections (Foo and Bar) and a Foo TTF: the TTF wins
    // by format, and both collections are copies of Foo, but they are the
    // only files with Bar, so one of them stays.
    let files = [
        file("/x/Fam.ttc", &["Foo", "Bar"]),
        file("/y/Fam.ttc", &["Foo", "Bar"]),
        file("/z/Foo.ttf", &["Foo"]),
    ];
    let mut groups = vec![DuplicateGroup {
        kind: DuplicateKind::Near,
        postscript_name: Some("Foo".into()),
        copies: vec![
            group_copy("/x/Fam.ttc", 1),
            group_copy("/y/Fam.ttc", 2),
            group_copy("/z/Foo.ttf", 3),
        ],
    }];
    apply_to_groups(
        &mut groups,
        &files,
        &HashMap::new(),
        &KeeperPolicy::default(),
        &[],
        &LanguageNames::default(),
    );
    let marks: Vec<(&str, bool)> = groups[0]
        .copies
        .iter()
        .map(|c| (c.path.as_str(), c.keep))
        .collect();
    assert_eq!(
        marks,
        [
            ("/z/Foo.ttf", true),
            ("/x/Fam.ttc", true),
            ("/y/Fam.ttc", false)
        ]
    );
    let problems: Vec<String> = groups[0].problems().into_iter().map(|p| p.path).collect();
    assert_eq!(problems, ["/y/Fam.ttc"]);

    // A Bar file elsewhere in the library: no collection has to stay.
    let more = [
        file("/x/Fam.ttc", &["Foo", "Bar"]),
        file("/y/Fam.ttc", &["Foo", "Bar"]),
        file("/z/Foo.ttf", &["Foo"]),
        file("/w/Bar.ttf", &["Bar"]),
    ];
    let held = holdings(&more);
    let candidate = |path: &str| Candidate {
        format: format_from_extension(path),
        holds: held.get(path).cloned().unwrap_or_default(),
        ..copy(path)
    };
    let rec = run(
        &[
            candidate("/x/Fam.ttc"),
            candidate("/y/Fam.ttc"),
            candidate("/z/Foo.ttf"),
        ],
        &KeeperPolicy::default(),
        &[],
    );
    assert_eq!(removed(&rec), ["/x/Fam.ttc", "/y/Fam.ttc"]);
}

#[test]
fn a_group_ranks_the_format_by_extension_as_choose_keeper_does() {
    // An .otf with TrueType outlines: the index stores Ttf, the extension
    // says OpenType CFF; both listings rank it as choose_keeper does.
    let files = [
        file("/b/Castoro.otf", &["X"]),
        file("/a/Castoro.ttf", &["X"]),
    ];
    let copies = vec![
        group_copy("/a/Castoro.ttf", 1),
        group_copy("/b/Castoro.otf", 2),
    ];
    let mut expected = DuplicateGroup {
        kind: DuplicateKind::Exact,
        postscript_name: Some("X".into()),
        copies,
    };
    let mut groups = vec![expected.clone()];
    expected.choose_keeper();
    apply_to_groups(
        &mut groups,
        &files,
        &HashMap::new(),
        &KeeperPolicy::default(),
        &[],
        &LanguageNames::default(),
    );
    assert_eq!(groups[0].copies[0].path, expected.copies[0].path);
    assert_eq!(groups[0].copies[0].path, "/b/Castoro.otf");

    let mut sets = [set_of(&[copy("/a/Castoro.ttf"), copy("/b/Castoro.otf")])];
    apply_to_sets(
        &mut sets,
        &files,
        &KeeperPolicy::default(),
        &[],
        &LanguageNames::default(),
    );
    let rec = sets[0].recommendation.as_ref().expect("recommendation");
    assert_eq!(rec.keeper.as_deref(), Some("/b/Castoro.otf"));
}

fn group_copy(path: &str, face: u32) -> DuplicateCopy {
    DuplicateCopy {
        path: path.to_owned(),
        size: 1,
        hash: String::new(),
        face: Some(FaceId(face)),
        version: Some("1.0".into()),
        source: SourceId(1),
        in_library_folder: false,
        keep: false,
    }
}

#[test]
fn a_group_is_ordered_keeper_first_and_protected_copies_are_no_problem() {
    let files = [
        file("/a.ttf", &["X"]),
        file("/b.ttf", &["X"]),
        file("/c.ttf", &["X"]),
    ];
    let mut groups = vec![DuplicateGroup {
        kind: DuplicateKind::Near,
        postscript_name: Some("X".into()),
        copies: vec![
            group_copy("/a.ttf", 1),
            group_copy("/b.ttf", 2),
            group_copy("/c.ttf", 3),
        ],
    }];
    let mut sr = facts();
    sr.languages_full = langs(&["sr-Cyrl"]);
    let facts: HashMap<FaceId, CopyFacts> = [(FaceId(3), sr)].into_iter().collect();
    let my = langs(&["sr-Cyrl"]);
    let names = LanguageNames::load(&my).expect("names");
    apply_to_groups(
        &mut groups,
        &files,
        &facts,
        &policy(&[KeeperRule::Path]),
        &my,
        &names,
    );
    let marks: Vec<(&str, bool)> = groups[0]
        .copies
        .iter()
        .map(|c| (c.path.as_str(), c.keep))
        .collect();
    assert_eq!(
        marks,
        [("/a.ttf", true), ("/c.ttf", true), ("/b.ttf", false)]
    );
    let problems: Vec<String> = groups[0].problems().into_iter().map(|p| p.path).collect();
    assert_eq!(
        problems,
        ["/b.ttf"],
        "the protected copy is no Problems entry"
    );
}

#[test]
fn a_system_copy_in_a_group_is_kept_and_never_the_keeper() {
    let mut system = file("/a.ttf", &["X"]);
    system.system = true;
    let files = [system, file("/b.ttf", &["X"])];
    let mut groups = vec![DuplicateGroup {
        kind: DuplicateKind::Exact,
        postscript_name: Some("X".into()),
        copies: vec![group_copy("/a.ttf", 1), group_copy("/b.ttf", 2)],
    }];
    apply_to_groups(
        &mut groups,
        &files,
        &HashMap::new(),
        &KeeperPolicy::default(),
        &[],
        &LanguageNames::default(),
    );
    assert_eq!(groups[0].copies[0].path, "/b.ttf");
    assert!(groups[0].copies.iter().all(|c| c.keep));
    assert!(groups[0].problems().is_empty());
}

fn set_of(copies: &[Candidate]) -> DuplicateSet {
    DuplicateSet {
        id: "name:X".into(),
        level: DuplicateLevel::OtherVersion,
        postscript_name: Some("X".into()),
        formats_differ: false,
        copies: copies
            .iter()
            .map(|c| DuplicateSetCopy {
                path: c.path.clone(),
                size: 1,
                hash: c.path.clone(),
                face: None,
                version: c.version.clone(),
                format: c.format,
                source: SourceId(1),
                in_library_folder: c.in_library_folder,
                system: c.system,
                in_scope: true,
                identical_with: Vec::new(),
                other_sets: Vec::new(),
                facts: c.facts.clone(),
            })
            .collect(),
        recommendation: None,
    }
}

#[test]
fn a_preview_counts_keepers_keep_both_and_changes_against_the_saved_policy() {
    let mut newer = copy("/b.ttf");
    newer.version = Some("2.0".into());
    let sets = [
        // The version decides by the saved policy, the path by the tried one.
        set_of(&[copy("/a.ttf"), newer]),
        // Keep both under either policy.
        set_of(&[
            covering("/c.ttf", &["de"]),
            covering("/d.ttf", &["sr-Cyrl"]),
        ]),
        // The same keeper under either.
        set_of(&[copy("/e.ttf"), copy("/f.ttf")]),
    ];
    let my = langs(&["de", "sr-Cyrl"]);
    let names = LanguageNames::load(&my).expect("names");
    let got = preview(
        &sets,
        &[],
        &policy(&[KeeperRule::Path]),
        &KeeperPolicy::default(),
        &my,
        &names,
    );
    assert_eq!(
        (got.sets, got.with_keeper, got.keep_both, got.changed),
        (3, 2, 1, 1)
    );
    assert_eq!(got.my_languages, MyLanguagesStatus::Set);
    let same = preview(
        &sets,
        &[],
        &KeeperPolicy::default(),
        &KeeperPolicy::default(),
        &my,
        &names,
    );
    assert_eq!(same.changed, 0);
}

#[test]
fn apply_to_sets_fills_every_recommendation() {
    let mut sets = [set_of(&[copy("/a.ttf"), copy("/b.ttf")])];
    apply_to_sets(
        &mut sets,
        &[],
        &KeeperPolicy::default(),
        &[],
        &LanguageNames::default(),
    );
    let rec = sets[0].recommendation.as_ref().expect("recommendation");
    assert_eq!(rec.keeper.as_deref(), Some("/a.ttf"));
    assert_eq!(removed(rec), ["/b.ttf"]);
}

#[test]
fn facts_are_read_only_for_a_language_or_a_rule_that_weighs_them() {
    assert!(!needs_facts(&KeeperPolicy::default(), &[]));
    assert!(needs_facts(&KeeperPolicy::default(), &langs(&["de"])));
    assert!(needs_facts(&policy(&[KeeperRule::MostGlyphs]), &[]));
    assert!(needs_facts(&policy(&[KeeperRule::ActiveOrRequested]), &[]));
}

#[test]
fn an_active_copy_wins_the_active_rule_over_a_requested_one_only_by_the_path() {
    let mut active = copy("/z.ttf");
    if let Some(f) = active.facts.as_mut() {
        f.activation = Some(ActivationMode::Session);
    }
    let (keeper, rule) = decide(KeeperRule::ActiveOrRequested, copy("/a.ttf"), active);
    assert_eq!(
        (keeper.as_str(), rule),
        ("/z.ttf", KeeperRule::ActiveOrRequested)
    );
}
