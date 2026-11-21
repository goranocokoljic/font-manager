//! A cleanup as one session through the API (E56.7): plan, apply on a job
//! with progress, a file changed since the plan, cancel in the middle, undo
//! whole and file by file, undo after the trash was emptied, "Keep this
//! one" as a session of one set, the activation move, one library change
//! per batch, and the budget on the reference library.

use std::sync::mpsc::{Receiver, Sender, channel};

use fm_api::activation::{ActivateRequest, Target};
use fm_api::cleanup::{
    ApplyCleanupRequest, CleanupAction, CleanupPlan, CleanupScope, CleanupSession, RemoveAs,
    SessionKind, StayReason, UndoCleanupRequest,
};
use fm_platform::{Trash, TrashError, Trashed};

use super::*;

/// A trash that holds its first move until the test lets it go, so a test
/// can act while an apply is between two files.
#[derive(Debug)]
struct GateTrash {
    inner: MockTrash,
    gate: Mutex<Option<(Sender<()>, Receiver<()>)>>,
}

impl Trash for GateTrash {
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError> {
        let gate = lock(&self.gate).take();
        let moved = self.inner.trash(path);
        if let Some((called, release)) = gate {
            let _ = called.send(());
            let _ = release.recv_timeout(WAIT);
        }
        moved
    }
}

/// The fixture files that get a copy in `zcopy/`, one set each.
const COPIED: &[&str] = &[
    "bebasneue/BebasNeue-Regular.ttf",
    "barlow/Barlow-BoldItalic.ttf",
    "archivo/Archivo-Bold.woff2",
];

/// A library with `copies` copies of each [`COPIED`] file in `zcopy/<n>/`,
/// scanned, its trash a folder.
fn library(label: &str, copies: usize, trash: Arc<dyn Trash>) -> (Lib, Vec<PathBuf>) {
    let lib = Lib::new(label);
    let mut made = Vec::new();
    for n in 0..copies {
        let dir = lib.root.join("zcopy").join(n.to_string());
        std::fs::create_dir_all(&dir).expect("folder");
        for file in COPIED {
            let from = lib.root.join(file);
            let to = dir.join(from.file_name().expect("name"));
            std::fs::copy(&from, &to).expect("copy");
            made.push(to);
        }
    }
    let lib = Lib {
        api: lib.api.with_trash(trash),
        ..lib
    };
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    (lib, made)
}

fn zcopy_plan(lib: &Lib) -> CleanupPlan {
    lib.api
        .plan_cleanup(fm_api::cleanup::PlanCleanupRequest {
            scope: CleanupScope::Folder {
                path: lib.root.join("zcopy").display().to_string(),
            },
            ..Default::default()
        })
        .expect("plan")
}

/// Applies `req` and returns the job's events up to its last one.
fn apply(lib: &Lib, req: ApplyCleanupRequest) -> (fm_api::JobId, Vec<Event>) {
    let rx = lib.api.subscribe();
    let job = lib.api.apply_cleanup(req).expect("apply");
    (job, until_finished(&rx, job))
}

fn until_finished(rx: &Receiver<Event>, job: fm_api::JobId) -> Vec<Event> {
    let mut events = Vec::new();
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = rx
            .recv_timeout(left)
            .unwrap_or_else(|e| panic!("cleanup {job} did not end ({e}): {events:?}"));
        let last = match &event {
            Event::CleanupProgress {
                job: j, finished, ..
            } => *j == job && *finished,
            Event::JobFailed { job: j, error } if *j == job => panic!("cleanup failed: {error:?}"),
            _ => false,
        };
        events.push(event);
        if last {
            return events;
        }
    }
}

fn session(lib: &Lib, events: &[Event]) -> CleanupSession {
    let id = events
        .iter()
        .find_map(|e| match e {
            Event::CleanupProgress { session, .. } => *session,
            _ => None,
        })
        .expect("a session");
    lib.api
        .list_cleanup_sessions()
        .expect("sessions")
        .into_iter()
        .find(|s| s.id == id)
        .expect("listed")
}

fn status(s: &CleanupSession, path: &Path) -> TrashMoveStatus {
    s.files
        .iter()
        .find(|f| Path::new(&f.path) == path)
        .unwrap_or_else(|| panic!("{} is not in {s:#?}", path.display()))
        .status
}

#[test]
fn a_plan_applies_as_one_session_and_undo_brings_the_files_back() {
    let dir = TempDir::new("api-cleanup-apply-trash");
    let (lib, made) = library(
        "api-cleanup-apply",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );

    let plan = zcopy_plan(&lib);
    assert_eq!(plan.sets, 3, "{plan:#?}");
    for copy in &made {
        let row = plan
            .files
            .iter()
            .find(|f| Path::new(&f.path) == copy)
            .unwrap_or_else(|| panic!("{} in {plan:#?}", copy.display()));
        assert_eq!(row.action, CleanupAction::Trash, "{row:#?}");
        assert!(row.kept.as_deref().is_some_and(|k| !k.contains("zcopy")));
    }
    assert!(
        plan.files
            .iter()
            .filter(|f| !f.path.contains("zcopy"))
            .all(|f| f.reason == Some(StayReason::Keeper))
    );
    let sizes: u32 = made
        .iter()
        .map(|p| u32::try_from(std::fs::metadata(p).expect("size").len()).expect("small"))
        .sum();
    assert_eq!(plan.frees, sizes);
    assert!(made.iter().all(|p| p.is_file()), "a plan touches nothing");

    // Saved as JSON and read back, it is the same plan.
    let saved = serde_json::to_string(&plan).expect("json");
    let plan: CleanupPlan = serde_json::from_str(&saved).expect("read back");

    // A file that changed since the plan is skipped and reported.
    let changed = &made[2];
    let mut bytes = std::fs::read(changed).expect("read");
    bytes.push(0);
    std::fs::write(changed, bytes).expect("change");

    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: Some("Spring cleanup".into()),
            choices: Vec::new(),
        },
    );
    let progress: Vec<(u32, u32)> = events
        .iter()
        .filter_map(|e| match e {
            Event::CleanupProgress { done, total, .. } => Some((*done, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(progress.first(), Some(&(0, 3)));
    assert_eq!(progress.last(), Some(&(3, 3)));
    let s = session(&lib, &events);
    assert_eq!(s.name, "Spring cleanup");
    assert_eq!(s.kind, SessionKind::Cleanup);
    assert!(!s.cancelled && !s.running && !s.undone);
    assert_eq!(s.trashed, 2);
    assert_eq!(status(&s, &made[0]), TrashMoveStatus::Trashed);
    assert_eq!(status(&s, &made[1]), TrashMoveStatus::Trashed);
    assert_eq!(status(&s, changed), TrashMoveStatus::Skipped);
    let skipped = s
        .files
        .iter()
        .find(|f| Path::new(&f.path) == changed)
        .and_then(|f| f.message.clone())
        .unwrap_or_default();
    assert!(skipped.contains("changed since the plan"), "{skipped}");
    assert!(!made[0].exists() && !made[1].exists() && changed.is_file());
    assert!(s.at.len() >= 10, "dated: {}", s.at);

    // One file back, then the rest.
    let one = lib
        .api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: vec![made[0].display().to_string()],
        })
        .expect("undo one");
    assert!(made[0].is_file() && !made[1].exists());
    assert!(!one.undone);
    assert_eq!(status(&one, &made[0]), TrashMoveStatus::Restored);
    let all = lib
        .api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: Vec::new(),
        })
        .expect("undo");
    assert!(all.undone, "{all:#?}");
    assert!(made.iter().all(|p| p.is_file()));
    let listed = lib.api.list_cleanup_sessions().expect("sessions");
    assert!(listed.iter().any(|x| x.id == s.id && x.undone));
}

#[test]
fn cancel_in_the_middle_leaves_a_session_of_what_was_done() {
    let dir = TempDir::new("api-cleanup-cancel-trash");
    let (called_tx, called) = channel();
    let (release, release_rx) = channel();
    let gate = Arc::new(GateTrash {
        inner: MockTrash::new(dir.path().join("trash")),
        gate: Mutex::new(Some((called_tx, release_rx))),
    });
    let (lib, made) = library("api-cleanup-cancel", 1, gate);
    let plan = zcopy_plan(&lib);
    let rx = lib.api.subscribe();
    let job = lib
        .api
        .apply_cleanup(ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        })
        .expect("apply");
    called.recv_timeout(WAIT).expect("the first move started");
    lib.api.cancel_cleanup(job).expect("cancel");
    release.send(()).expect("release");
    let events = until_finished(&rx, job);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::CleanupProgress {
            finished: true,
            cancelled: true,
            ..
        }
    )));
    let s = session(&lib, &events);
    assert!(s.cancelled && !s.running, "{s:#?}");
    assert_eq!(s.trashed, 1);
    assert_eq!(s.files.len(), 1, "only what was done: {s:#?}");
    assert!(s.name.starts_with("Cleanup of "), "{}", s.name);
    assert_eq!(made.iter().filter(|p| p.is_file()).count(), 2);
    assert_eq!(
        lib.api.cancel_cleanup(job).unwrap_err().code,
        ErrorCode::NotFound,
        "a finished job is not running"
    );
    // What was done can be undone.
    let undone = lib
        .api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: Vec::new(),
        })
        .expect("undo");
    assert!(undone.undone);
    assert!(made.iter().all(|p| p.is_file()));
}

#[test]
fn undo_after_the_trash_was_emptied_reports_each_file_that_cannot_come_back() {
    let dir = TempDir::new("api-cleanup-emptied-trash");
    let trash = Arc::new(MockTrash::new(dir.path().join("trash")));
    let (lib, made) = library(
        "api-cleanup-emptied",
        1,
        Arc::clone(&trash) as Arc<dyn Trash>,
    );
    let plan = zcopy_plan(&lib);
    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        },
    );
    let s = session(&lib, &events);
    assert_eq!(s.trashed, 3);
    // The user empties the trash of one file.
    let in_trash = std::fs::read_dir(trash.dir())
        .expect("trash")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with("BebasNeue-Regular.ttf"))
        .expect("in the trash");
    std::fs::remove_file(in_trash).expect("empty");
    // Another file is at the old path of a second one.
    std::fs::write(&made[1], b"another file").expect("occupy");

    let undone = lib
        .api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: Vec::new(),
        })
        .expect("undo");
    assert!(!undone.undone, "{undone:#?}");
    let row = |p: &Path| {
        undone
            .files
            .iter()
            .find(|f| Path::new(&f.path) == p)
            .expect("row")
            .clone()
    };
    assert_eq!(row(&made[0]).status, TrashMoveStatus::Failed);
    assert!(row(&made[0]).message.is_some());
    assert_eq!(row(&made[1]).status, TrashMoveStatus::Failed);
    assert_eq!(
        std::fs::read(&made[1]).expect("read"),
        b"another file",
        "never over a file at the old path"
    );
    assert_eq!(row(&made[2]).status, TrashMoveStatus::Restored);
    assert!(made[2].is_file());
}

#[test]
fn keep_this_one_is_a_session_of_one_set_in_the_same_history() {
    let dir = TempDir::new("api-cleanup-keep-copy-trash");
    let (lib, made) = library(
        "api-cleanup-keep-copy",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let original: PathBuf = COPIED[0]
        .split('/')
        .fold(lib.root.clone(), |p, part| p.join(part));
    lib.api
        .keep_copy(KeepCopyRequest {
            keep: original.display().to_string(),
            trash: vec![made[0].display().to_string()],
            dry_run: false,
        })
        .expect("keep");
    let sessions = lib.api.list_cleanup_sessions().expect("sessions");
    let s = sessions.first().expect("one session");
    assert_eq!(s.kind, SessionKind::KeepCopy);
    assert_eq!(s.name, "Keep BebasNeue-Regular.ttf");
    assert_eq!(s.sets, 1);
    assert_eq!(s.files.len(), 1);
    assert_eq!(s.files[0].kept, original.display().to_string());
    assert_eq!(lib.api.list_resolutions().expect("history").len(), 1);
    let undone = lib
        .api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: Vec::new(),
        })
        .expect("undo");
    assert!(undone.undone);
    assert!(made[0].is_file());
    assert!(lib.api.list_resolutions().expect("history")[0].undone);
}

#[test]
fn an_active_identical_copy_can_hand_its_activation_to_the_kept_copy() {
    let dir = TempDir::new("api-cleanup-activation-trash");
    let (lib, made) = library(
        "api-cleanup-activation",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let plan = zcopy_plan(&lib);
    let face_of = |p: &Path| {
        plan.files
            .iter()
            .find(|f| Path::new(&f.path) == p)
            .expect("row")
            .faces[0]
    };
    let copy = face_of(&made[0]);
    let kept = face_of(&lib.root.join(COPIED[0]));
    lib.api
        .activate(ActivateRequest {
            target: Target::Faces { faces: vec![copy] },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .expect("activate");

    let offered = zcopy_plan(&lib);
    let row = offered
        .files
        .iter()
        .find(|f| Path::new(&f.path) == made[0])
        .expect("row");
    assert_eq!(row.reason, Some(StayReason::Active));
    assert!(row.activation.as_ref().is_some_and(|a| !a.planned));

    let moved = lib
        .api
        .plan_cleanup(fm_api::cleanup::PlanCleanupRequest {
            scope: CleanupScope::Folder {
                path: lib.root.join("zcopy").display().to_string(),
            },
            move_activations: true,
            ..Default::default()
        })
        .expect("plan");
    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan: moved,
            name: None,
            choices: Vec::new(),
        },
    );
    let s = session(&lib, &events);
    assert_eq!(status(&s, &made[0]), TrashMoveStatus::Trashed, "{s:#?}");
    let active: Vec<FaceId> = lib
        .api
        .list_active()
        .expect("active")
        .into_iter()
        .map(|a| a.face)
        .collect();
    assert_eq!(active, vec![kept], "the kept copy took over the activation");
}

#[test]
fn the_library_change_is_one_event_per_batch_not_per_file() {
    let dir = TempDir::new("api-cleanup-batch-trash");
    let (lib, made) = library(
        "api-cleanup-batch",
        60,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let plan = zcopy_plan(&lib);
    assert_eq!(plan.leaving().count(), made.len());
    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        },
    );
    let changes: Vec<usize> = events
        .iter()
        .filter_map(|e| match e {
            Event::LibraryChanged { removed, .. } => Some(removed.len()),
            _ => None,
        })
        .collect();
    assert_eq!(changes.iter().sum::<usize>(), made.len(), "{changes:?}");
    assert_eq!(
        changes.len(),
        made.len().div_ceil(crate::resolve::cleanup::BATCH),
        "{changes:?}"
    );
}

#[test]
fn a_hidden_copy_is_planned_but_not_applied_yet() {
    let dir = TempDir::new("api-cleanup-hide-trash");
    let (lib, _) = library(
        "api-cleanup-hide",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let plan = lib
        .api
        .plan_cleanup(fm_api::cleanup::PlanCleanupRequest {
            scope: CleanupScope::Folder {
                path: lib.root.join("zcopy").display().to_string(),
            },
            remove_as: RemoveAs::Hide,
            ..Default::default()
        })
        .expect("plan");
    assert!(plan.leaving().all(|f| f.action == CleanupAction::Hide));
    assert_eq!(plan.frees, 0, "hiding frees no space");
    let err = lib
        .api
        .apply_cleanup(ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);
}

/// The budget of ADR 0027: a plan of a thousand files of the reference
/// library applies (with a folder trash) within it.
#[test]
fn a_thousand_file_plan_applies_within_the_budget() {
    let Some(reference) = std::env::var_os("FM_REFERENCE_LIBRARY") else {
        eprintln!("skipped: FM_REFERENCE_LIBRARY is not set");
        return;
    };
    let mut fonts: Vec<PathBuf> = walk(Path::new(&reference));
    fonts.sort();
    fonts.truncate(1000);
    assert_eq!(
        fonts.len(),
        1000,
        "the reference library holds a thousand fonts"
    );
    let lib = Lib::new("api-cleanup-budget");
    let dir = TempDir::new("api-cleanup-budget-trash");
    let lib = Lib {
        api: lib
            .api
            .with_trash(Arc::new(MockTrash::new(dir.path().join("trash")))),
        ..lib
    };
    for (n, from) in fonts.iter().enumerate() {
        let name = format!(
            "{n:04}-{}",
            from.file_name().expect("name").to_string_lossy()
        );
        for side in ["a", "zcopy"] {
            let to = lib.root.join(side).join(&name);
            std::fs::create_dir_all(to.parent().expect("folder")).expect("folder");
            std::fs::copy(from, &to).expect("copy");
        }
    }
    let source = lib
        .api
        .add_source(AddSourceRequest {
            path: lib.root.display().to_string(),
            mode: ImportMode::IndexInPlace,
        })
        .expect("source");
    lib.scan(Some(source.id), true);
    let plan = zcopy_plan(&lib);
    let leaving = plan.leaving().count();
    assert!(leaving >= 950, "most copies leave: {leaving}");
    let start = Instant::now();
    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: None,
            choices: Vec::new(),
        },
    );
    let took = start.elapsed();
    let changes = events
        .iter()
        .filter(|e| matches!(e, Event::LibraryChanged { .. }))
        .count();
    eprintln!(
        "cleanup_apply: {leaving} files in {} ms, {changes} library changes",
        took.as_millis()
    );
    assert!(changes <= leaving.div_ceil(crate::resolve::cleanup::BATCH) + 1);
    assert!(
        took < crate::resolve::cleanup::APPLY_BUDGET,
        "{leaving} files took {took:?}"
    );
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if crate::scanner::is_font_file(&p) {
            out.push(p);
        }
    }
    out
}

#[test]
fn the_plan_carries_the_metadata_conflicts_and_undo_takes_the_merge_back() {
    use fm_api::OrganizationService;
    use fm_api::cleanup::{MetadataChoice, MetadataField, MetadataValue};
    use fm_api::organization::UserMetaUpdate;
    let dir = TempDir::new("api-cleanup-merge-trash");
    let (lib, made) = library(
        "api-cleanup-merge",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let plan = zcopy_plan(&lib);
    let row = plan
        .files
        .iter()
        .find(|f| Path::new(&f.path) == made[0])
        .expect("row");
    let copy = row.faces[0];
    let kept = plan
        .files
        .iter()
        .find(|f| Some(&f.path) == row.kept.as_ref())
        .expect("kept row")
        .faces[0];
    let meta = |faces: Vec<FaceId>, favorite: Option<bool>, rating: u8| UserMetaUpdate {
        faces,
        favorite,
        rating: Some(rating),
        notes: None,
    };
    lib.api
        .update_user_meta(meta(vec![kept], None, 2))
        .expect("kept rating");
    lib.api
        .update_user_meta(meta(vec![copy], Some(true), 5))
        .expect("copy rating");

    let plan = zcopy_plan(&lib);
    let conflict = plan
        .conflicts
        .iter()
        .find(|c| c.kept_face == kept)
        .unwrap_or_else(|| panic!("a conflict: {:#?}", plan.conflicts));
    assert_eq!(conflict.field, MetadataField::Rating);
    assert_eq!(conflict.default, MetadataValue::Rating { stars: 2 });
    assert_eq!(conflict.other, MetadataValue::Rating { stars: 5 });
    assert_eq!(conflict.other_face, copy);

    let (_, events) = apply(
        &lib,
        ApplyCleanupRequest {
            plan,
            name: None,
            choices: vec![MetadataChoice {
                kept_face: kept,
                field: MetadataField::Rating,
                take: copy,
            }],
        },
    );
    let face = lib.api.get_face(kept).expect("face");
    assert_eq!(face.rating, Some(5), "the user took the other value");
    assert!(face.favorite, "favorite is on when any copy has it");

    let s = session(&lib, &events);
    lib.api
        .undo_cleanup_session(UndoCleanupRequest {
            session: s.id,
            paths: vec![made[0].display().to_string()],
        })
        .expect("undo");
    let face = lib.api.get_face(kept).expect("face");
    assert_eq!(face.rating, Some(2), "the merge is taken back with its set");
    assert!(!face.favorite);
}

#[test]
fn a_choice_that_names_no_conflict_of_the_plan_is_refused() {
    use fm_api::cleanup::{MetadataChoice, MetadataField};
    let dir = TempDir::new("api-cleanup-choice-trash");
    let (lib, _) = library(
        "api-cleanup-choice",
        1,
        Arc::new(MockTrash::new(dir.path().join("trash"))),
    );
    let plan = zcopy_plan(&lib);
    let err = lib
        .api
        .apply_cleanup(ApplyCleanupRequest {
            plan,
            name: None,
            choices: vec![MetadataChoice {
                kept_face: FaceId(1),
                field: MetadataField::Notes,
                take: FaceId(2),
            }],
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert!(
        lib.api
            .list_cleanup_sessions()
            .expect("sessions")
            .is_empty()
    );
}
