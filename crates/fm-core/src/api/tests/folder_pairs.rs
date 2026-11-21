//! Folder pairs through the API (E56.6): a copied tree is one contained
//! pair at its top, the files that are not fonts are counted from the
//! folders, and the second request names each side's files.

use fm_api::library::{ListFolderPairFilesRequest, ListFolderPairsRequest, PairSide};

use super::*;

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("folder");
    for entry in std::fs::read_dir(from).expect("read") {
        let path = entry.expect("entry").path();
        std::fs::copy(&path, to.join(path.file_name().expect("name"))).expect("copy");
    }
}

#[test]
fn a_copied_tree_is_one_pair_with_its_other_files_counted() {
    let lib = Lib::new("api-folder-pairs");
    let (one, two) = (lib.root.join("one"), lib.root.join("two"));
    for name in ["bebasneue", "asapsharp", "barlow", "agdasima", "archivo"] {
        copy_tree(&lib.root.join(name), &one.join(name));
        copy_tree(&lib.root.join(name), &two.join(name));
    }
    std::fs::write(two.join("readme.txt"), b"not a font").expect("write");
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);

    let pairs = lib
        .api
        .list_folder_pairs(ListFolderPairsRequest::default())
        .expect("pairs");
    let path = |p: &Path| p.display().to_string();
    let top = pairs
        .items
        .iter()
        .find(|p| p.a.path == path(&one) && p.b.path == path(&two))
        .unwrap_or_else(|| panic!("the copied tree: {pairs:#?}"));
    assert!(top.a.contained && top.b.contained);
    assert!(top.a.font_files >= 5);
    assert_eq!(top.a.other_files, 0);
    assert_eq!(top.b.other_files, 1, "the readme is counted");
    assert!(two.join("readme.txt").exists(), "and never touched");
    assert_eq!(top.frees, top.a.size);
    assert!(
        !pairs
            .items
            .iter()
            .any(|p| p.a.path == path(&one.join("barlow"))
                && p.b.path == path(&two.join("barlow"))),
        "not once per subfolder"
    );
    let order: Vec<u32> = pairs.items.iter().map(|p| p.frees).collect();
    assert!(order.windows(2).all(|w| w[0] >= w[1]), "{order:?}");

    let files = lib
        .api
        .list_folder_pair_files(ListFolderPairFilesRequest {
            a: path(&one),
            b: path(&two),
            side: Some(PairSide::B),
            only_here: false,
            page: Page::default(),
        })
        .expect("files");
    assert_eq!(files.total, top.b.font_files);
    assert!(files.items.iter().all(|f| {
        f.side == PairSide::B
            && f.level == Some(fm_api::library::DuplicateLevel::Identical)
            && f.copy
                .as_deref()
                .is_some_and(|c| c.starts_with(&path(&one)))
    }));

    let missing = lib.api.list_folder_pairs(ListFolderPairsRequest {
        scope: DuplicateScope::Source {
            source: SourceId(999),
        },
        ..Default::default()
    });
    assert_eq!(missing.unwrap_err().code, fm_api::ErrorCode::NotFound);
}
