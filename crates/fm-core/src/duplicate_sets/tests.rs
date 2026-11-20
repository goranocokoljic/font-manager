use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

/// One row of `fixtures/duplicates/set-cases.json`.
#[derive(Deserialize)]
struct Case {
    name: String,
    #[serde(default)]
    scope: DuplicateScope,
    files: Vec<CaseFile>,
    sets: Value,
}

#[derive(Deserialize)]
struct CaseFile {
    path: String,
    hash: String,
    format: Option<FontFormat>,
    source: u32,
    kind: String,
    faces: Vec<CaseFace>,
}

#[derive(Deserialize)]
struct CaseFace {
    face: u32,
    name: String,
    version: Option<String>,
}

/// The fields of a set the case file states.
pub(crate) fn summary(sets: &[DuplicateSet]) -> Value {
    sets.iter()
        .map(|s| {
            json!({
                "id": s.id,
                "level": s.level,
                "postscriptName": s.postscript_name,
                "formatsDiffer": s.formats_differ,
                "copies": s.copies.iter().map(|c| json!({
                    "path": c.path,
                    "face": c.face,
                    "version": c.version,
                    "system": c.system,
                    "inScope": c.in_scope,
                    "identicalWith": c.identical_with,
                    "otherSets": c.other_sets,
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!(
        "../../../../fixtures/duplicates/set-cases.json"
    ))
    .expect("set-cases.json")
}

fn files(case: &Case) -> Vec<SetFile> {
    case.files
        .iter()
        .map(|f| SetFile {
            path: f.path.clone(),
            size: 1000,
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

#[test]
fn every_case_of_the_case_file_gives_its_sets() {
    let cases = cases();
    assert!(cases.len() >= 10);
    for case in &cases {
        let got = summary(&sets(&files(case), &case.scope));
        assert_eq!(got, case.sets, "{}", case.name);
    }
}

#[test]
fn the_order_of_the_files_does_not_change_the_sets() {
    for case in &cases() {
        let mut reversed = files(case);
        reversed.reverse();
        let mut a = sets(&files(case), &case.scope);
        let mut b = sets(&reversed, &case.scope);
        // An identical set names the first face of the first file it meets.
        for s in a.iter_mut().chain(b.iter_mut()) {
            if s.level == DuplicateLevel::Identical {
                s.postscript_name = None;
                for c in &mut s.copies {
                    c.face = None;
                    c.version = None;
                }
            }
        }
        assert_eq!(summary(&a), summary(&b), "{}", case.name);
    }
}

#[test]
fn an_empty_library_and_a_single_file_have_no_sets() {
    assert!(sets(&[], &DuplicateScope::Library).is_empty());
    let one = SetFile {
        path: "/a.ttf".into(),
        size: 1,
        hash: "aa".into(),
        format: Some(FontFormat::Ttf),
        source: SourceId(1),
        in_library_folder: false,
        system: false,
        faces: vec![SetFace {
            face: FaceId(1),
            name: "A".into(),
            version: None,
        }],
    };
    assert!(sets(&[one], &DuplicateScope::Library).is_empty());
}

#[test]
fn versions_compare_by_digit_runs_and_no_version_is_its_own_value() {
    let key = |v: Option<&str>| version_key(v);
    assert_eq!(key(Some("Version 1.000")), key(Some(" version 1.0")));
    assert_ne!(key(Some("Version 2.9")), key(Some("Version 2.10")));
    assert_eq!(key(None), key(Some("   ")));
    assert_ne!(key(None), key(Some("1.0")));
}

#[test]
fn digit_runs_past_u64_are_one_value_and_a_page_holds_at_most_500_sets() {
    assert_eq!(
        version_key(Some("1.99999999999999999999")),
        version_key(Some("1.99999999999999999998"))
    );
    let files: Vec<SetFile> = (0..1002)
        .map(|i| SetFile {
            path: format!("/f/{i}.ttf"),
            size: 1,
            hash: format!("h{}", i / 2),
            format: Some(FontFormat::Ttf),
            source: SourceId(1),
            in_library_folder: false,
            system: false,
            faces: Vec::new(),
        })
        .collect();
    let page = list(
        &files,
        &ListDuplicateSetsRequest {
            page: fm_api::common::Page {
                offset: 0,
                limit: 10_000,
            },
            ..Default::default()
        },
    );
    assert_eq!((page.total, page.items.len()), (501, MAX_PAGE as usize));
}
