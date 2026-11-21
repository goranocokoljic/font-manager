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
    }
}
