use serde::Deserialize;
use serde_json::Value;

use super::*;
use crate::common::FontFormat;
use crate::mock::duplicate_sets::SetFace;
use crate::{FaceId, SourceId};

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

/// The same table `fm_core::folder_pairs` and the TypeScript mock run.
#[test]
fn the_pair_rule_gives_the_core_answer_for_every_shared_case() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../../fixtures/duplicates/pair-cases.json"
    ))
    .expect("pair cases");
    assert!(cases.len() >= 9, "the table is there");
    for case in cases {
        let files: Vec<SetFile> = case
            .files
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
            .collect();
        let roots: Vec<Root> = case
            .roots
            .iter()
            .map(|r| Root {
                path: PathBuf::from(&r.path),
                kind: r.kind,
            })
            .collect();
        let got = serde_json::to_value(list(&files, &roots, &case.request)).unwrap();
        assert_eq!(got, case.pairs, "case: {}", case.name);
        // `find` names a listed pair either way round (E56.12's draft), at
        // the default threshold.
        if case.request.min_shared.is_none() && case.request.scope == DuplicateScope::Library {
            let listed = list(&files, &roots, &case.request).items;
            for pair in &listed {
                assert_eq!(
                    find(&files, &roots, &pair.a.path, &pair.b.path).as_ref(),
                    Some(pair)
                );
                assert_eq!(
                    find(&files, &roots, &pair.b.path, &pair.a.path).as_ref(),
                    Some(pair)
                );
            }
            assert_eq!(
                find(&files, &roots, "/nowhere", "/else"),
                None,
                "case: {}",
                case.name
            );
        }
    }
}

#[test]
fn the_files_of_a_pair_filter_by_level() {
    let file = |level| FolderPairFile {
        path: "/a/x.ttf".to_owned(),
        side: PairSide::A,
        size: 1,
        level,
        copy: None,
    };
    let req = |levels: Option<Vec<DuplicateLevel>>, only_here| ListFolderPairFilesRequest {
        a: "/a".to_owned(),
        b: "/b".to_owned(),
        side: None,
        only_here,
        levels,
        page: crate::common::Page::default(),
    };
    let identical = Some(vec![DuplicateLevel::Identical]);
    let other = Some(vec![
        DuplicateLevel::SameVersion,
        DuplicateLevel::OtherVersion,
    ]);
    assert!(req(identical.clone(), false).keeps(&file(Some(DuplicateLevel::Identical))));
    assert!(!req(identical.clone(), false).keeps(&file(Some(DuplicateLevel::OtherVersion))));
    assert!(
        !req(identical, false).keeps(&file(None)),
        "only here has no level"
    );
    assert!(req(other.clone(), false).keeps(&file(Some(DuplicateLevel::SameVersion))));
    assert!(!req(other, false).keeps(&file(Some(DuplicateLevel::Identical))));
    assert!(req(None, false).keeps(&file(Some(DuplicateLevel::Identical))));
    assert!(req(None, true).keeps(&file(None)));
    assert!(!req(None, true).keeps(&file(Some(DuplicateLevel::Identical))));
}
