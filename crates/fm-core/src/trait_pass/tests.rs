use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use fm_api::FaceId;
use fm_api::library::SourceKind;

use super::*;
use crate::index::INDEX_FILE;
use crate::scanner::{FileIndex, discover};
use crate::test_support::TempDir;

/// Static TrueType, Latin.
const STATIC: &str = "google-static/robotoslab/RobotoSlab-ExtraBold.ttf";
/// Variable TrueType, Latin.
const VARIABLE: &str = "google-repo/epilogue/Epilogue[wght].ttf";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fonts")
}

fn copy(fonts: &Path, rel: &str, name: &str) -> PathBuf {
    fs::create_dir_all(fonts).expect("folder");
    let to = fonts.join(name);
    fs::copy(fixtures().join(rel), &to).expect("copy fixture");
    to
}

/// An index on disk holding the scan of `fonts`.
fn scanned(dir: &TempDir, fonts: &Path) -> Index {
    let mut files = FileIndex::new();
    files.rescan(fonts, &discover(fonts).expect("discover"));
    let mut index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .expect("open")
        .index;
    let source = index
        .add_source(&fonts.to_string_lossy(), SourceKind::Watched)
        .expect("source")
        .id;
    index.persist_files(source, files.files()).expect("persist");
    index
}

fn current() -> Vec<(&'static str, u32)> {
    traits::versions().collect()
}

fn run_all(index: &Mutex<Index>) -> PassReport {
    run(index, &AtomicBool::new(false), &mut |_, _| {}).expect("pass")
}

fn face_of(index: &Index, file: &str) -> FaceId {
    index
        .trait_debug_face(file)
        .unwrap_or_else(|| panic!("no face for {file}"))
}

/// Criterion 3 and 4: the pass measures every face once, writes a row per
/// detection, and a second pass has nothing to do.
#[test]
fn the_pass_measures_every_face_once() {
    let dir = TempDir::new("trait-pass-all");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    copy(&fonts, VARIABLE, "variable.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    assert_eq!(
        index
            .lock()
            .unwrap()
            .trait_pass_remaining(&current())
            .unwrap(),
        2
    );
    let mut seen = Vec::new();
    let report = run(&index, &AtomicBool::new(false), &mut |d, t| {
        seen.push((d, t))
    })
    .unwrap();
    assert_eq!(
        report,
        PassReport {
            measured: 2,
            total: 2,
            cancelled: false
        }
    );
    assert_eq!(seen.first(), Some(&(0, 2)));
    assert_eq!(seen.last(), Some(&(2, 2)));
    let index = index.into_inner().unwrap();
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 0);
    let rows = index
        .face_glyph_traits(face_of(&index, "static.ttf"))
        .unwrap();
    assert_eq!(
        rows.len(),
        traits::trait_ids().count(),
        "every Latin glyph is there"
    );
    let o = rows.iter().find(|r| r.trait_id == "O.roundness").unwrap();
    assert!(o.scalar.is_some() && o.scalars.as_deref().is_some_and(|s| s.contains("corners")));
    let a = rows.iter().find(|r| r.trait_id == "a.storeys").unwrap();
    assert_eq!(a.value, "double", "Roboto Slab has a double-storey a");
    assert!(a.confidence >= traits::CONFIDENT);
    assert!(a.scalar.is_none() && a.scalars.is_none());
    assert!(rows.iter().all(|r| r.detector_version >= 1));
    let again = Mutex::new(index);
    assert_eq!(run_all(&again).measured, 0);
}

/// Criterion 5: a variable face's rows say they are of the default instance;
/// a static face's say static.
#[test]
fn a_variable_face_is_stored_as_its_default_instance() {
    let dir = TempDir::new("trait-pass-variable");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    copy(&fonts, VARIABLE, "variable.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    run_all(&index);
    let index = index.into_inner().unwrap();
    let var = index
        .face_glyph_traits(face_of(&index, "variable.ttf"))
        .unwrap();
    assert!(!var.is_empty());
    assert!(var.iter().all(|r| r.instance == "default"));
    let fixed = index
        .face_glyph_traits(face_of(&index, "static.ttf"))
        .unwrap();
    assert!(fixed.iter().all(|r| r.instance == "static"));
}

/// Criterion 3: a detector version bump measures that trait again on every
/// face and leaves every other trait as it was.
#[test]
fn a_version_bump_recomputes_only_that_trait() {
    let dir = TempDir::new("trait-pass-bump");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    copy(&fonts, VARIABLE, "variable.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    run_all(&index);
    // Mark every stored row, so a row written again shows.
    index
        .lock()
        .unwrap()
        .trait_debug_sql("UPDATE glyph_traits SET confidence = 0.123");
    let bumped: Vec<(&str, u32)> = current()
        .into_iter()
        .map(|(id, v)| (id, if id == "g.storeys" { v + 1 } else { v }))
        .collect();
    let report = run_at(&index, &AtomicBool::new(false), &bumped, &mut |_, _| {}).unwrap();
    assert_eq!(report.measured, 2);
    let index = index.into_inner().unwrap();
    assert_eq!(index.trait_pass_remaining(&bumped).unwrap(), 0);
    // The old versions are now out of date for that one trait only.
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 2);
    for file in ["static.ttf", "variable.ttf"] {
        let rows = index.face_glyph_traits(face_of(&index, file)).unwrap();
        for r in &rows {
            if r.trait_id == "g.storeys" {
                assert_ne!(r.confidence, 0.123, "{file}: g.storeys measured again");
                assert_eq!(r.detector_version, current()[2].1 + 1);
            } else {
                assert_eq!(r.confidence, 0.123, "{file}: {} untouched", r.trait_id);
            }
        }
    }
    let todo = index.trait_pass_todo(&current(), 0, 10).unwrap();
    assert!(todo.iter().all(|t| t.traits == ["g.storeys"]));
}

/// Criterion 4: a cancelled pass stops after the batch it is in, keeps what
/// it wrote, and the next pass (after a restart: a new index handle on the
/// same file) measures only the rest.
#[test]
fn a_cancelled_pass_resumes_where_it_stopped() {
    let dir = TempDir::new("trait-pass-cancel");
    let fonts = dir.path().join("fonts");
    let n = BATCH as usize + 6;
    for i in 0..n {
        copy(&fonts, STATIC, &format!("copy-{i:03}.ttf"));
    }
    let index = Mutex::new(scanned(&dir, &fonts));
    let cancel = AtomicBool::new(false);
    let report = run(&index, &cancel, &mut |done, _| {
        if done > 0 {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    })
    .unwrap();
    assert!(report.cancelled);
    assert_eq!(report.measured, BATCH);
    assert_eq!(report.total, n as u32);
    drop(index);
    // A restart: the index opened again from its file.
    let reopened = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .unwrap()
        .index;
    assert_eq!(reopened.trait_pass_remaining(&current()).unwrap(), 6);
    let reopened = Mutex::new(reopened);
    let rest = run_all(&reopened);
    assert_eq!((rest.measured, rest.total, rest.cancelled), (6, 6, false));
    // A pass cancelled before it starts measures nothing.
    let set = AtomicBool::new(true);
    let none = run(&reopened, &set, &mut |_, _| {}).unwrap();
    assert_eq!(none.measured, 0);
}

/// A file read again with other content (a new hash) is measured again; a
/// result for the old content is not written.
#[test]
fn a_changed_file_is_measured_again_and_a_stale_result_is_dropped() {
    let dir = TempDir::new("trait-pass-changed");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    let todo = index
        .lock()
        .unwrap()
        .trait_pass_todo(&current(), 0, 10)
        .unwrap();
    assert_eq!(todo.len(), 1);
    let measured = measure_all(&todo, &current(), &AtomicBool::new(false));
    // The file is read again (another hash) before the result is written.
    index
        .lock()
        .unwrap()
        .trait_debug_sql("UPDATE font_file SET hash = x'00ff'");
    assert_eq!(
        index.lock().unwrap().write_glyph_traits(&measured).unwrap(),
        0
    );
    let mut guard = index.lock().unwrap();
    let face = face_of(&guard, "static.ttf");
    assert!(guard.face_glyph_traits(face).unwrap().is_empty());
    assert_eq!(guard.trait_pass_remaining(&current()).unwrap(), 1);
    let todo = guard.trait_pass_todo(&current(), 0, 10).unwrap();
    assert_eq!(todo[0].file_hash, vec![0x00, 0xff]);
    // The result for the current hash is written.
    let fresh = measure_all(&todo, &current(), &AtomicBool::new(false));
    assert_eq!(guard.write_glyph_traits(&fresh).unwrap(), 1);
    assert_eq!(guard.trait_pass_remaining(&current()).unwrap(), 0);
}

/// A file that cannot be read is skipped, not stamped, and the pass ends.
#[test]
fn a_file_that_cannot_be_read_is_tried_again_by_the_next_pass() {
    let dir = TempDir::new("trait-pass-gone");
    let fonts = dir.path().join("fonts");
    let gone = copy(&fonts, STATIC, "gone.ttf");
    copy(&fonts, VARIABLE, "here.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    fs::remove_file(&gone).unwrap();
    let report = run_all(&index);
    assert_eq!((report.measured, report.total), (1, 2));
    assert_eq!(
        index
            .lock()
            .unwrap()
            .trait_pass_remaining(&current())
            .unwrap(),
        1
    );
}

/// A face whose data cannot be parsed is stamped without rows, so the pass
/// does not try it forever.
#[test]
fn a_face_that_cannot_be_parsed_is_stamped_without_traits() {
    let dir = TempDir::new("trait-pass-broken");
    let fonts = dir.path().join("fonts");
    let path = copy(&fonts, STATIC, "static.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    // Same hash in the index, other bytes on disk: unreadable as a font.
    fs::write(&path, b"not a font any more").unwrap();
    let report = run_all(&index);
    assert_eq!(report.measured, 1);
    let index = index.into_inner().unwrap();
    let face = face_of(&index, "static.ttf");
    assert!(index.face_glyph_traits(face).unwrap().is_empty());
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 0);
}

/// A face that lacks a trait's glyph has a stamp but no row for it.
#[test]
fn a_missing_glyph_is_no_row() {
    let dir = TempDir::new("trait-pass-missing");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    let mut index = scanned(&dir, &fonts);
    let todo = index.trait_pass_todo(&current(), 0, 10).unwrap();
    let mut measured = measure_all(&todo, &current(), &AtomicBool::new(false));
    // As if the face had no `1`.
    for t in &mut measured[0].traits {
        if t.0 == "one.flag" {
            t.2 = None;
        }
    }
    index.write_glyph_traits(&measured).unwrap();
    let face = face_of(&index, "static.ttf");
    let rows = index.face_glyph_traits(face).unwrap();
    assert!(rows.iter().all(|r| r.trait_id != "one.flag"));
    assert_eq!(rows.len(), traits::trait_ids().count() - 1);
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 0);
}

/// The table and its (trait, value) index exist.
#[test]
fn glyph_traits_has_an_index_on_trait_and_value() {
    let index = Index::open_in_memory().unwrap();
    assert_eq!(
        index.trait_debug_index_columns("glyph_traits_trait_value"),
        ["trait", "value"]
    );
}

/// Criterion 4: the runner's pass ends with one finish event; cancelled at
/// once, it stops before it is done and says so.
#[test]
fn the_runner_reports_its_end_and_can_be_cancelled() {
    let dir = TempDir::new("trait-pass-runner");
    let fonts = dir.path().join("fonts");
    let n = BATCH as usize + 6;
    for i in 0..n {
        copy(&fonts, STATIC, &format!("copy-{i:03}.ttf"));
    }
    let index = std::sync::Arc::new(Mutex::new(scanned(&dir, &fonts)));
    let (tx, rx) = std::sync::mpsc::channel();
    let runner = Runner::default();
    let send = tx.clone();
    runner.start(std::sync::Arc::clone(&index), JobId(7), move |e| {
        let _ = send.send(e);
    });
    runner.cancel();
    let end = loop {
        match rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("events")
        {
            e @ Event::TraitPassFinished { .. } => break e,
            Event::TraitPassProgress { job, .. } => assert_eq!(job, JobId(7)),
            other => panic!("{other:?}"),
        }
    };
    let Event::TraitPassFinished {
        job,
        measured,
        cancelled,
    } = end
    else {
        unreachable!()
    };
    assert_eq!(job, JobId(7));
    assert!(cancelled);
    assert!((measured as usize) < n);
    // Started again, it finishes the rest.
    runner.start(std::sync::Arc::clone(&index), JobId(8), move |e| {
        let _ = tx.send(e);
    });
    let end = loop {
        if let e @ Event::TraitPassFinished { job: JobId(8), .. } = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("events")
        {
            break e;
        }
    };
    assert_eq!(
        end,
        Event::TraitPassFinished {
            job: JobId(8),
            measured: n as u32 - measured,
            cancelled: false
        }
    );
    runner.stop();
    assert_eq!(
        index
            .lock()
            .unwrap()
            .trait_pass_remaining(&current())
            .unwrap(),
        0
    );
}

/// An index from before glyph traits (V22) gains the tables on open, and its
/// faces wait for the pass; nothing else of the face changes.
#[test]
fn an_index_from_before_glyph_traits_measures_them_after_the_upgrade() {
    let dir = TempDir::new("trait-pass-migration");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    let index = scanned(&dir, &fonts);
    let face = face_of(&index, "static.ttf");
    index.trait_debug_sql(
        "DROP TABLE glyph_descriptor; DROP TABLE glyph_trait_stamp; DROP TABLE glyph_traits; PRAGMA user_version = 22;",
    );
    drop(index);
    let index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .unwrap()
        .index;
    assert_eq!(
        index.schema_version().unwrap(),
        crate::index::SCHEMA_VERSION
    );
    assert_eq!(face_of(&index, "static.ttf"), face, "the face keeps its id");
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 1);
    let index = Mutex::new(index);
    assert_eq!(run_all(&index).measured, 1);
    assert!(
        !index
            .lock()
            .unwrap()
            .face_glyph_traits(face)
            .unwrap()
            .is_empty()
    );
}

/// A pass with nothing to measure (an empty or up-to-date library) reports
/// nothing: no progress and no finish event, so a scan's events are not
/// interleaved with a pass that did no work.
#[test]
fn a_pass_with_nothing_to_measure_is_silent() {
    let dir = TempDir::new("trait-pass-silent");
    let fonts = dir.path().join("fonts");
    fs::create_dir_all(&fonts).unwrap();
    let index = std::sync::Arc::new(Mutex::new(scanned(&dir, &fonts)));
    let mut calls = 0;
    let report = run(&index, &AtomicBool::new(false), &mut |_, _| calls += 1).unwrap();
    assert_eq!((report.total, report.measured, calls), (0, 0, 0));
    let (tx, rx) = std::sync::mpsc::channel();
    let runner = Runner::default();
    runner.start(std::sync::Arc::clone(&index), JobId(9), move |e| {
        let _ = tx.send(e);
    });
    runner.stop();
    assert!(rx.try_iter().next().is_none(), "no events");
}

/// E49.8 criterion 1 and 4: the pass stores the descriptor of every trait
/// glyph for every face, so a similar-glyph query of one has nothing left
/// to describe; a face's descriptor is the one its file gives.
#[test]
fn the_pass_describes_every_trait_glyph_of_every_face() {
    let dir = TempDir::new("trait-pass-descriptors");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    copy(&fonts, VARIABLE, "variable.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    {
        let index = index.lock().unwrap();
        assert_eq!(index.glyph_descriptor_todo('R' as u32).unwrap().len(), 2);
        assert_eq!(index.trait_pass_remaining(&versions()).unwrap(), 2);
    }
    run_all(&index);
    let index = index.into_inner().unwrap();
    assert_eq!(index.trait_pass_remaining(&versions()).unwrap(), 0);
    for ch in similarity::TRAIT_GLYPHS {
        assert!(
            index
                .glyph_descriptor_todo(u32::from(ch))
                .unwrap()
                .is_empty(),
            "{ch} described for every face"
        );
    }
    assert_eq!(
        index.glyph_descriptor_todo('x' as u32).unwrap().len(),
        2,
        "a glyph that is not a trait glyph is left for on demand"
    );
    let data = fs::read(fixtures().join(STATIC)).unwrap();
    let stored = index.glyph_debug_descriptor(face_of(&index, "static.ttf"), 'R' as u32);
    assert_eq!(stored, similarity::describe(&data, 0, 'R' as u32));
}

/// E49.8: a descriptor version bump describes every face again and measures
/// no trait; a detector bump leaves the descriptors alone.
#[test]
fn a_descriptor_version_bump_describes_again_and_measures_no_trait() {
    let dir = TempDir::new("trait-pass-descriptor-bump");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    copy(&fonts, VARIABLE, "variable.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    run_all(&index);
    index.lock().unwrap().trait_debug_sql(
        "UPDATE glyph_traits SET confidence = 0.123;
         UPDATE glyph_descriptor SET descriptor = x'00';",
    );
    let bumped: Vec<(&str, u32)> = versions()
        .into_iter()
        .map(|(id, v)| (id, if id == similarity::STAMP { v + 1 } else { v }))
        .collect();
    let todo = index
        .lock()
        .unwrap()
        .trait_pass_todo(&bumped, 0, 10)
        .unwrap();
    assert!(todo.iter().all(|t| t.traits == [similarity::STAMP]));
    let report = run_at(&index, &AtomicBool::new(false), &bumped, &mut |_, _| {}).unwrap();
    assert_eq!(report.measured, 2);
    let index = index.into_inner().unwrap();
    for file in ["static.ttf", "variable.ttf"] {
        let face = face_of(&index, file);
        let rows = index.face_glyph_traits(face).unwrap();
        assert!(
            rows.iter().all(|r| r.confidence == 0.123),
            "{file}: traits untouched"
        );
        assert_eq!(
            index
                .glyph_debug_descriptor(face, 'R' as u32)
                .map(|d| d.as_bytes().len()),
            Some(similarity::LEN),
            "{file}: R described again"
        );
    }
}

/// E49.8: a face whose data cannot be parsed is stamped with no descriptor
/// of each trait glyph, so it is not described again on every query.
#[test]
fn a_broken_face_has_no_descriptors_and_is_not_described_again() {
    let dir = TempDir::new("trait-pass-descriptor-broken");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    let face = face_of(&index.lock().unwrap(), "static.ttf");
    // The file now holds garbage; its hash in the index stays, as if the
    // pass read a file that is damaged.
    fs::write(fonts.join("static.ttf"), b"not a font, not at all").unwrap();
    run_all(&index);
    let index = index.into_inner().unwrap();
    assert_eq!(index.glyph_debug_descriptor(face, 'R' as u32), None);
    assert!(index.glyph_descriptor_todo('R' as u32).unwrap().is_empty());
}

/// E49.8: an index from before glyph descriptors (V23), its traits all
/// measured, gains the table on open; the next pass describes its faces and
/// measures no trait again.
#[test]
fn an_index_from_before_glyph_descriptors_describes_them_after_the_upgrade() {
    let dir = TempDir::new("trait-pass-descriptor-migration");
    let fonts = dir.path().join("fonts");
    copy(&fonts, STATIC, "static.ttf");
    let index = Mutex::new(scanned(&dir, &fonts));
    run_at(&index, &AtomicBool::new(false), &current(), &mut |_, _| {}).unwrap();
    let index = index.into_inner().unwrap();
    let face = face_of(&index, "static.ttf");
    index.trait_debug_sql(
        "UPDATE glyph_traits SET confidence = 0.123;
         DROP TABLE glyph_descriptor; PRAGMA user_version = 23;",
    );
    drop(index);
    let index = Index::open(&dir.path().join("data").join(INDEX_FILE))
        .unwrap()
        .index;
    assert_eq!(
        index.schema_version().unwrap(),
        crate::index::SCHEMA_VERSION
    );
    assert_eq!(index.trait_pass_remaining(&current()).unwrap(), 0);
    assert_eq!(index.trait_pass_remaining(&versions()).unwrap(), 1);
    let index = Mutex::new(index);
    assert_eq!(run_all(&index).measured, 1);
    let index = index.into_inner().unwrap();
    assert!(
        index
            .face_glyph_traits(face)
            .unwrap()
            .iter()
            .all(|r| r.confidence == 0.123)
    );
    assert!(index.glyph_debug_descriptor(face, 'R' as u32).is_some());
}
