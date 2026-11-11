use std::fs;
use std::path::{Path, PathBuf};

use fm_api::library::SourceKind;

use super::*;
use crate::index::INDEX_FILE;
use crate::scanner::{FileIndex, discover};
use crate::test_support::TempDir;

const MULISH: &str = "google-repo/mulish/Mulish[wght].ttf";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn scanned(dir: &TempDir, names: &[&str]) -> Index {
    let root = dir.path().join("fonts");
    fs::create_dir_all(&root).unwrap();
    for name in names {
        fs::copy(fixtures().join(MULISH), root.join(name)).unwrap();
    }
    let mut files = FileIndex::new();
    files.rescan(&root, &discover(&root).expect("discover"));
    let mut index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .expect("open")
        .index;
    let source = index
        .add_source(&root.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files.files()).expect("persist");
    index
}

fn r_of(t: &DescriptorTodo) -> DescribedFace {
    let data = fs::read(&t.path).unwrap();
    DescribedFace {
        face: t.face,
        file_hash: t.file_hash.clone(),
        glyphs: vec![('R' as u32, similarity::describe(&data, 0, 'R' as u32))],
    }
}

#[test]
fn every_read_face_needs_a_descriptor_until_one_is_written() {
    let dir = TempDir::new("glyph-descriptor-todo");
    let mut index = scanned(&dir, &["a.ttf", "b.ttf"]);
    let todo = index.glyph_descriptor_todo('R' as u32).unwrap();
    assert_eq!(todo.len(), 2);
    assert!(
        todo.windows(2).all(|w| w[0].face.0 < w[1].face.0),
        "by face id"
    );
    let first = r_of(&todo[0]);
    assert_eq!(
        index
            .write_glyph_descriptors(std::slice::from_ref(&first))
            .unwrap(),
        1
    );
    let left = index.glyph_descriptor_todo('R' as u32).unwrap();
    assert_eq!(left, todo[1..].to_vec());
    assert_eq!(
        index.current_glyph_descriptors('R' as u32).unwrap(),
        vec![(first.face.0, first.glyphs[0].1.clone().unwrap())]
    );
    // Other characters are not touched.
    assert_eq!(index.glyph_descriptor_todo('O' as u32).unwrap().len(), 2);
}

#[test]
fn a_face_that_does_not_draw_the_character_is_current_without_a_descriptor() {
    let dir = TempDir::new("glyph-descriptor-none");
    let mut index = scanned(&dir, &["a.ttf"]);
    let t = &index.glyph_descriptor_todo(0x05D0).unwrap()[0];
    let none = DescribedFace {
        face: t.face,
        file_hash: t.file_hash.clone(),
        glyphs: vec![(0x05D0, None)],
    };
    index.write_glyph_descriptors(&[none]).unwrap();
    assert!(index.glyph_descriptor_todo(0x05D0).unwrap().is_empty());
    assert!(index.current_glyph_descriptors(0x05D0).unwrap().is_empty());
}

#[test]
fn a_write_for_a_changed_file_or_a_gone_face_writes_nothing() {
    let dir = TempDir::new("glyph-descriptor-stale-write");
    let mut index = scanned(&dir, &["a.ttf"]);
    let t = index.glyph_descriptor_todo('R' as u32).unwrap().remove(0);
    let mut changed = r_of(&t);
    changed.file_hash = vec![1, 2, 3];
    let mut gone = r_of(&t);
    gone.face = FaceId(999_999);
    assert_eq!(index.write_glyph_descriptors(&[changed, gone]).unwrap(), 0);
    assert_eq!(index.glyph_descriptor_todo('R' as u32).unwrap().len(), 1);
    assert_eq!(index.write_glyph_descriptors(&[]).unwrap(), 0);
}

#[test]
fn a_deleted_face_takes_its_descriptors_with_it() {
    let dir = TempDir::new("glyph-descriptor-cascade");
    let mut index = scanned(&dir, &["a.ttf"]);
    let t = index.glyph_descriptor_todo('R' as u32).unwrap().remove(0);
    index.write_glyph_descriptors(&[r_of(&t)]).unwrap();
    index.trait_debug_sql(&format!("DELETE FROM face WHERE id = {}", t.face.0));
    assert!(
        index
            .current_glyph_descriptors('R' as u32)
            .unwrap()
            .is_empty()
    );
    assert_eq!(index.glyph_debug_descriptor(t.face, 'R' as u32), None);
}

#[test]
fn a_stored_row_of_the_wrong_length_is_skipped() {
    let dir = TempDir::new("glyph-descriptor-length");
    let mut index = scanned(&dir, &["a.ttf"]);
    let t = index.glyph_descriptor_todo('R' as u32).unwrap().remove(0);
    index.write_glyph_descriptors(&[r_of(&t)]).unwrap();
    index.trait_debug_sql("UPDATE glyph_descriptor SET descriptor = x'0102'");
    assert!(
        index
            .current_glyph_descriptors('R' as u32)
            .unwrap()
            .is_empty()
    );
}
