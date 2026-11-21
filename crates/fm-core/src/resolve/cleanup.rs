//! Applying a cleanup plan as one session, and undoing it (E56.7, ADR 0027).
//!
//! - **One session.** The session's head row is written first; then the
//!   plan is applied set by set, and each set's files, gone flags and
//!   metadata merge are one transaction as soon as the set is done (as
//!   "Keep this one" writes them), so a quit or a crash loses no undo of
//!   an earlier set. A set whose log cannot be written is put back from the trash before
//!   the error is reported, so no file is ever in the trash without undo.
//! - **Checks again.** Every file is checked when its turn comes: still in
//!   the library and on disk, not a system copy (rule 4), the plan's hash
//!   (a file that changed is skipped and reported), not active unless the
//!   plan moves its activation (rule 3), and its set's kept copy still
//!   there. The activation is read again just before the move, and the
//!   move is done under the same index lock.
//! - **Cancel** stops between two files; the set being done is written, the
//!   session is `cancelled` and holds what was done.
//! - **Events.** Progress is [`Event::CleanupProgress`]; the faces that left
//!   are sent as removed in one [`Event::LibraryChanged`] per [`BATCH`]
//!   files, not one per file; the sidecars are written once per batch.
//! - **Undo** follows ADR 0009: a file goes back only to a free path; one
//!   that cannot (the path is taken, the trash was emptied) is `Failed`
//!   with why, and calling again retries it. A set's merge is taken back
//!   once every moved copy of it is back (E56.5).

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use fm_api::activation::ActivationMode;
use fm_api::cleanup::{
    ApplyCleanupRequest, CleanupAction, CleanupFile, CleanupSession, HiddenCopy, MetadataChoice,
    MetadataField, ShowHiddenRequest, UndoCleanupRequest,
};
use fm_api::event::Event;
use fm_api::library::{TrashMove, TrashMoveStatus};
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId, JobId, ResolutionId};
use fm_platform::{TrashErrorKind, Trashed};

use super::{ACTIVE_COPY, DuplicateResolver, NOT_THERE, SYSTEM_COPY};
use crate::error::FmError;
use crate::index::{CopyFile, MergeChoice, MergeField, MergePlan, SessionMove, StoredMove};

/// The files after which the faces that left are sent in one
/// [`Event::LibraryChanged`].
pub const BATCH: usize = 100;

/// How long applying a plan of a thousand files may take with a folder
/// trash (ADR 0027): the core's own work, the checks, the hashes, the log
/// and the events, without the OS trash's time per file.
pub const APPLY_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// Why a file that changed since the plan stays.
const CHANGED: &str = "The file changed since the plan was made, so it was not moved.";
/// Why a file whose kept copy is gone stays.
const KEPT_GONE: &str = "The copy to keep is not there any more, so this copy was not moved.";
/// Why a file that left the library stays.
const NOT_IN_LIBRARY: &str = "The file is no longer in the library.";
/// Why a copy that could not be hidden stays.
const HIDE_FAILED: &str = "The copy could not be hidden, so it stays in the library.";

/// Moves the activation of a leaving copy to the kept copy: deactivate the
/// `leaving` faces, activate the `kept` faces in `mode`. Both are the
/// app's own registrations. An error says why, in one sentence.
pub type MoveActivation<'a> =
    dyn Fn(&[FaceId], &[FaceId], ActivationMode) -> Result<(), String> + Send + Sync + 'a;

/// The cleanups being applied, by job.
#[derive(Debug, Clone, Default)]
pub struct Running(Arc<Mutex<HashMap<JobId, RunningJob>>>);

#[derive(Debug, Clone)]
struct RunningJob {
    cancel: Arc<AtomicBool>,
    session: Option<ResolutionId>,
}

impl Running {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<JobId, RunningJob>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn sessions(&self) -> BTreeSet<ResolutionId> {
        self.lock().values().filter_map(|j| j.session).collect()
    }
}

/// What an apply did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The session.
    pub session: ResolutionId,
    /// It was cancelled.
    pub cancelled: bool,
}

fn to_merge_field(f: MetadataField) -> MergeField {
    match f {
        MetadataField::Rating => MergeField::Rating,
        MetadataField::License => MergeField::License,
        MetadataField::Notes => MergeField::Notes,
    }
}

impl DuplicateResolver {
    /// Registers `job` as a cleanup that can be cancelled; the apply then
    /// runs [`Self::apply_cleanup`] under it.
    pub fn register_cleanup(&self, job: JobId) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.lock().insert(
            job,
            RunningJob {
                cancel: Arc::clone(&cancel),
                session: None,
            },
        );
        cancel
    }

    /// [`fm_api::LibraryService::cancel_cleanup`]: stops `job` between two
    /// files. `NotFound` when it is not running.
    pub fn cancel_cleanup(&self, job: JobId) -> ApiResult<()> {
        let running = self.running.lock();
        let j = running
            .get(&job)
            .ok_or_else(|| ApiError::not_found(format!("Cleanup {job}")))?;
        j.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Checks a request before its job starts: the plan's format, and every
    /// choice names a conflict of the plan.
    pub fn check_cleanup(&self, req: &ApplyCleanupRequest) -> ApiResult<()> {
        req.plan.check()?;
        for c in &req.choices {
            if !req
                .plan
                .conflicts
                .iter()
                .any(|k| k.kept_face == c.kept_face && k.field == c.field && k.other_face == c.take)
            {
                return Err(ApiError::invalid(
                    "A choice names a value that is not a conflict of this plan.",
                ));
            }
        }
        Ok(())
    }

    /// Applies `req` as one session under `job` (registered with
    /// [`Self::register_cleanup`]): see the module. `move_activation` moves
    /// the activation of an active copy the plan says to.
    pub fn apply_cleanup(
        &self,
        job: JobId,
        req: &ApplyCleanupRequest,
        move_activation: &MoveActivation<'_>,
    ) -> ApiResult<Applied> {
        let result = self.apply_inner(job, req, move_activation);
        self.running.lock().remove(&job);
        result
    }

    /// Ends `job`: it leaves the running cleanups first, so a caller that
    /// sees the last event can undo the session or finds the job gone, then
    /// the last [`Event::CleanupProgress`] is sent.
    fn finish(&self, job: JobId, session: ResolutionId, done: u32, total: u32, cancelled: bool) {
        self.running.lock().remove(&job);
        self.progress(job, Some(session), done, total, None, Some(cancelled));
    }

    fn progress(
        &self,
        job: JobId,
        session: Option<ResolutionId>,
        done: u32,
        total: u32,
        path: Option<String>,
        end: Option<bool>,
    ) {
        (self.emit)(Event::CleanupProgress {
            job,
            session,
            done,
            total,
            path,
            finished: end.is_some(),
            cancelled: end.unwrap_or(false),
        });
    }

    fn apply_inner(
        &self,
        job: JobId,
        req: &ApplyCleanupRequest,
        move_activation: &MoveActivation<'_>,
    ) -> ApiResult<Applied> {
        self.check_cleanup(req)?;
        let cancel = self
            .running
            .lock()
            .get(&job)
            .map(|j| Arc::clone(&j.cancel))
            .unwrap_or_default();
        let plan = &req.plan;
        let leaving: Vec<&CleanupFile> = plan.leaving().collect();
        let total = u32::try_from(leaving.len()).unwrap_or(u32::MAX);
        let name = req
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned);

        let mut wanted: Vec<&str> = leaving.iter().map(|f| f.path.as_str()).collect();
        for f in &leaving {
            wanted.extend(f.kept.as_deref());
            wanted.extend(f.also_kept.iter().map(String::as_str));
        }
        let (session, at, files) = {
            let mut index = self.lock();
            let files = index.copy_files(&wanted, &self.system)?;
            let at = index.now()?;
            let name = name.unwrap_or_else(|| format!("Cleanup of {}", &at[..10.min(at.len())]));
            let (session, at) = index.start_session(&name)?;
            (session, at, files)
        };
        let _ = at;
        if let Some(j) = self.running.lock().get_mut(&job) {
            j.session = Some(session);
        }
        self.progress(job, Some(session), 0, total, None, None);

        // The sets in plan order, each with its leaving files.
        let mut order: Vec<&str> = Vec::new();
        let mut by_set: HashMap<&str, Vec<&CleanupFile>> = HashMap::new();
        for f in &leaving {
            if !by_set.contains_key(f.set.as_str()) {
                order.push(&f.set);
            }
            by_set.entry(&f.set).or_default().push(f);
        }

        let mut position: u32 = 0;
        let mut done: u32 = 0;
        // Each set is logged as soon as it is done, so a quit or a crash
        // never leaves a moved file without undo; the faces that left are
        // sent as one library change every [`BATCH`] files.
        let mut removed: Vec<FaceId> = Vec::new();
        let mut merged = false;
        let mut pending_files = 0usize;
        let mut cancelled = false;
        for set in order {
            if cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let group = &by_set[set];
            let mut moves: Vec<SessionMove> = Vec::new();
            for (k, f) in group.iter().enumerate() {
                if k > 0 && cancel.load(Ordering::SeqCst) {
                    cancelled = true;
                    break;
                }
                let m = self.apply_file(f, &files, move_activation, position, session);
                position += 1;
                done += 1;
                self.progress(job, Some(session), done, total, Some(f.path.clone()), None);
                moves.push(m);
            }
            let trashed: Vec<&SessionMove> = moves
                .iter()
                .filter(|m| m.stored.row.status == TrashMoveStatus::Trashed)
                .collect();
            let merge = if trashed.is_empty() {
                Ok(MergePlan::default())
            } else {
                self.set_merge(group, &trashed, &files, &req.choices)
            };
            let merge = match merge {
                Ok(merge) => merge,
                Err(e) => {
                    let set = [(set, moves, MergePlan::default())];
                    self.end_batch(&mut removed, &mut merged);
                    return Err(self.abort(job, session, &set, done, total, e));
                }
            };
            pending_files += moves.len();
            // A hidden copy marked its faces for their sidecar.
            merged |= moves
                .iter()
                .any(|m| m.hidden && m.stored.row.status == TrashMoveStatus::Trashed);
            let gone: Vec<FaceId> = moves
                .iter()
                .filter(|m| m.stored.row.status == TrashMoveStatus::Trashed)
                .filter_map(|m| files.get(&m.stored.row.path))
                .flat_map(|f| f.faces.iter().copied())
                .collect();
            let set = [(set, moves, merge)];
            // Bound first: the index lock is let go before `abort` takes it.
            let written = self.lock().write_session_sets(session, &set);
            match written {
                Ok(m) => merged |= m,
                Err(e) => {
                    self.end_batch(&mut removed, &mut merged);
                    return Err(self.abort(job, session, &set, done, total, e.into()));
                }
            }
            removed.extend(gone);
            if pending_files >= BATCH {
                self.end_batch(&mut removed, &mut merged);
                pending_files = 0;
            }
            if cancelled {
                break;
            }
        }
        self.end_batch(&mut removed, &mut merged);
        self.lock().finish_session(session, cancelled)?;
        self.finish(job, session, done, total, cancelled);
        Ok(Applied { session, cancelled })
    }

    /// Ends a batch of logged sets: writes the sidecars once when a merge
    /// changed metadata, then sends the faces in `removed` (their files went
    /// to the trash) in one [`Event::LibraryChanged`], and empties it.
    fn end_batch(&self, removed: &mut Vec<FaceId>, merged: &mut bool) {
        if std::mem::take(merged) {
            self.lock().mirror_session_sidecars();
        }
        if removed.is_empty() {
            return;
        }
        (self.emit)(Event::LibraryChanged {
            from_watcher: false,
            source: None,
            added: Vec::new(),
            removed: std::mem::take(removed),
            updated: Vec::new(),
        });
    }

    /// After the log of `pending` could not be written: puts every moved
    /// file of it back (without the log there is no undo), ends the session
    /// as cancelled, and returns `error` to report.
    fn abort(
        &self,
        job: JobId,
        session: ResolutionId,
        pending: &[(&str, Vec<SessionMove>, MergePlan)],
        done: u32,
        total: u32,
        error: ApiError,
    ) -> ApiError {
        let stored: Vec<StoredMove> = pending
            .iter()
            .flat_map(|(_, moves, _)| moves.iter().map(|m| m.stored.clone()))
            .collect();
        self.put_back(&stored);
        // A hidden copy is shown again: without the log it has no undo.
        let hidden: Vec<&SessionMove> = pending
            .iter()
            .flat_map(|(_, moves, _)| moves.iter())
            .filter(|m| m.hidden && m.stored.row.status == TrashMoveStatus::Trashed)
            .collect();
        if !hidden.is_empty() {
            let mut index = self.lock();
            for m in hidden {
                if let Err(e) = index.unhide_copy(&m.stored.path) {
                    tracing::error!(path = %m.stored.path.display(), error = %e, "a hidden copy could not be logged or shown again");
                }
            }
            index.mirror_session_sidecars();
        }
        if let Err(e) = self.lock().finish_session(session, true) {
            tracing::warn!(error = %e, "ending a failed cleanup session");
        }
        self.finish(job, session, done, total, true);
        error
    }

    /// Checks and moves one file of the plan.
    fn apply_file(
        &self,
        f: &CleanupFile,
        files: &HashMap<String, CopyFile>,
        move_activation: &MoveActivation<'_>,
        position: u32,
        session: ResolutionId,
    ) -> SessionMove {
        let hide = f.action == CleanupAction::Hide;
        let mut m = SessionMove {
            stored: StoredMove {
                position,
                path: std::path::PathBuf::from(&f.path),
                row: TrashMove {
                    path: f.path.clone(),
                    status: TrashMoveStatus::Skipped,
                    message: None,
                },
                trashed: None,
            },
            set: Some(f.set.clone()),
            kept: f.kept.clone(),
            size: Some(f.size),
            hidden: hide,
        };
        let skip = |m: &mut SessionMove, why: &str| m.stored.row.message = Some(why.to_owned());
        let Some(file) = files.get(&f.path) else {
            skip(&mut m, NOT_IN_LIBRARY);
            return m;
        };
        m.stored.path = file.path.clone();
        let kept_there = f.kept.as_ref().is_some_and(|k| {
            files
                .get(k)
                .is_some_and(|c| fs::symlink_metadata(&c.path).is_ok())
        });
        if file.system {
            skip(&mut m, SYSTEM_COPY);
        } else if !kept_there {
            skip(&mut m, KEPT_GONE);
        } else if fs::symlink_metadata(&file.path).is_err() {
            skip(&mut m, NOT_THERE);
        } else if !same_hash(&file.path, &f.hash) {
            skip(&mut m, CHANGED);
        } else {
            // Activation can change while a long apply runs: the state is
            // read again now, and the file goes to the trash under the same
            // lock, so the app never trashes a copy it has registered.
            let mut index = self.lock();
            if self.active_now(&index, file) {
                let planned = f.activation.as_ref().filter(|a| a.planned);
                let kept = f.kept.as_ref().and_then(|k| files.get(k));
                let (Some(a), Some(kept)) = (planned, kept) else {
                    skip(&mut m, ACTIVE_COPY);
                    return m;
                };
                // Activation takes the lock itself.
                drop(index);
                if let Err(why) = move_activation(&file.faces, &kept.faces, a.mode) {
                    skip(&mut m, &why);
                    return m;
                }
                index = self.lock();
                if self.active_now(&index, file) {
                    skip(&mut m, ACTIVE_COPY);
                    return m;
                }
            }
            if hide {
                // The file stays where it is (rule 2); it leaves the library
                // under the same lock the activation was read under.
                match index.hide_copy(&file.path, &f.hash, session, &f.set, f.kept.as_deref()) {
                    Ok(true) => m.stored.row.status = TrashMoveStatus::Trashed,
                    Ok(false) => skip(&mut m, CHANGED),
                    Err(e) => {
                        tracing::warn!(path = %file.path.display(), error = %e, "hiding a copy failed");
                        skip(&mut m, HIDE_FAILED);
                    }
                }
                return m;
            }
            let trashed = self.trash.trash(&file.path);
            drop(index);
            match trashed {
                Ok(item) => {
                    m.stored.row.status = TrashMoveStatus::Trashed;
                    m.stored.trashed = Some((item.location, item.info));
                }
                Err(e) => {
                    tracing::warn!(path = %file.path.display(), error = %e, "moving a copy to the trash failed");
                    m.stored.row.status = if e.kind == TrashErrorKind::NoTrash {
                        TrashMoveStatus::Skipped
                    } else {
                        TrashMoveStatus::Failed
                    };
                    m.stored.row.message = Some(e.message().to_owned());
                }
            }
        }
        m
    }

    /// Whether the app has one of `file`'s faces activated now. A failed
    /// read counts as active: the file then stays.
    fn active_now(&self, index: &crate::index::Index, file: &CopyFile) -> bool {
        index.any_face_active(&file.faces).unwrap_or_else(|e| {
            tracing::warn!(path = %file.path.display(), error = %e, "reading the activation of a copy failed");
            true
        })
    }

    /// The merge of one set's moved copies onto the copies kept for them,
    /// with the choices that still name one of its conflicts.
    fn set_merge(
        &self,
        group: &[&CleanupFile],
        trashed: &[&SessionMove],
        files: &HashMap<String, CopyFile>,
        choices: &[MetadataChoice],
    ) -> ApiResult<MergePlan> {
        let kept_paths: BTreeSet<&str> = group
            .iter()
            .flat_map(|f| f.kept.iter().chain(f.also_kept.iter()))
            .map(String::as_str)
            .collect();
        let kept: Vec<FaceId> = kept_paths
            .iter()
            .filter_map(|p| files.get(*p))
            .flat_map(|f| f.faces.iter().copied())
            .collect();
        let leaving: Vec<FaceId> = trashed
            .iter()
            .filter_map(|m| files.get(&m.stored.row.path))
            .flat_map(|f| f.faces.iter().copied())
            .collect();
        let index = self.lock();
        let mut plan = index.merge_plan(&kept, &leaving)?;
        let still: Vec<MergeChoice> = choices
            .iter()
            .map(|c| MergeChoice {
                kept_face: c.kept_face,
                field: to_merge_field(c.field),
                take: c.take,
            })
            .filter(|c| {
                plan.conflicts.iter().any(|k| {
                    k.kept_face == c.kept_face && k.field == c.field && k.other_face == c.take
                })
            })
            .collect();
        plan.choose(&still)?;
        Ok(plan)
    }

    /// [`fm_api::LibraryService::list_cleanup_sessions`].
    pub fn list_sessions(&self) -> ApiResult<Vec<CleanupSession>> {
        let running = self.running.sessions();
        let index = self.lock();
        let mut out = Vec::new();
        for head in index.session_heads()? {
            let moves = index.session_moves(head.id)?;
            out.push(crate::index::session(
                &head,
                &moves,
                running.contains(&head.id),
            ));
        }
        Ok(out)
    }

    /// One session.
    pub fn session(&self, id: ResolutionId) -> ApiResult<CleanupSession> {
        let running = self.running.sessions();
        let index = self.lock();
        let head = index
            .session_head(id)?
            .ok_or_else(|| FmError::not_found(format!("Session {id}")))?;
        let moves = index.session_moves(id)?;
        Ok(crate::index::session(&head, &moves, running.contains(&id)))
    }

    /// [`fm_api::LibraryService::undo_cleanup_session`].
    pub fn undo_session(&self, req: &UndoCleanupRequest) -> ApiResult<CleanupSession> {
        if self.running.sessions().contains(&req.session) {
            return Err(ApiError::new(
                ErrorCode::Busy,
                "This cleanup is still being applied. Cancel it or wait, then undo.",
            ));
        }
        let mut index = self.lock();
        let head = index
            .session_head(req.session)?
            .ok_or_else(|| FmError::not_found(format!("Session {}", req.session)))?;
        let mut moves = index.session_moves(req.session)?;
        for p in &req.paths {
            if !moves.iter().any(|m| m.stored.row.path == *p) {
                return Err(ApiError::invalid(format!(
                    "{p} is not a file of this session."
                )));
            }
        }
        let mut back: Vec<String> = Vec::new();
        for m in &mut moves {
            let s = &mut m.stored;
            if !req.paths.is_empty() && !req.paths.contains(&s.row.path) {
                continue;
            }
            // Moved files, and files an earlier undo could not put back.
            let waiting = matches!(
                s.row.status,
                TrashMoveStatus::Trashed | TrashMoveStatus::Failed
            );
            // A hidden copy never left its folder: it is shown again
            // (`update_session` drops its row and clears its gone flag).
            if m.hidden {
                if waiting {
                    s.row.status = TrashMoveStatus::Restored;
                    s.row.message = None;
                    back.push(s.row.path.clone());
                }
                continue;
            }
            let Some((location, info)) = s.trashed.clone().filter(|_| waiting) else {
                continue;
            };
            let item = Trashed {
                original: s.path.clone(),
                location,
                info,
            };
            match self.trash.restore(&item) {
                Ok(()) => {
                    s.row.status = TrashMoveStatus::Restored;
                    s.row.message = None;
                    back.push(s.row.path.clone());
                }
                Err(e) => {
                    tracing::warn!(path = %s.path.display(), error = %e, "putting a copy back failed");
                    s.row.status = TrashMoveStatus::Failed;
                    s.row.message = Some(e.message().to_owned());
                }
            }
        }
        // A file left the library when it went to the trash or was hidden.
        let moved = |m: &SessionMove| {
            m.stored.trashed.is_some()
                || (m.hidden && m.stored.row.status != TrashMoveStatus::Skipped)
        };
        let is_back =
            |m: &SessionMove| !moved(m) || m.stored.row.status == TrashMoveStatus::Restored;
        let sets: BTreeSet<&str> = moves.iter().filter_map(|m| m.set.as_deref()).collect();
        let sets_back: Vec<String> = sets
            .into_iter()
            .filter(|set| {
                let of_set: Vec<&SessionMove> = moves
                    .iter()
                    .filter(|m| m.set.as_deref() == Some(*set))
                    .collect();
                of_set.iter().all(|m| is_back(m)) && of_set.iter().any(|m| moved(m))
            })
            .map(str::to_owned)
            .collect();
        let undone = moves.iter().all(is_back);
        index.update_session(req.session, &moves, &sets_back, undone)?;
        let refs: Vec<&str> = back.iter().map(String::as_str).collect();
        let added: Vec<FaceId> = match index.copy_files(&refs, &self.system) {
            Ok(files) => files
                .values()
                .flat_map(|f| f.faces.iter().copied())
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "reading the restored faces failed");
                Vec::new()
            }
        };
        let head = crate::index::SessionHead { undone, ..head };
        let session = crate::index::session(&head, &moves, false);
        drop(index);
        if !added.is_empty() {
            (self.emit)(Event::LibraryChanged {
                from_watcher: false,
                source: None,
                added,
                removed: Vec::new(),
                updated: Vec::new(),
            });
        }
        Ok(session)
    }
}

impl DuplicateResolver {
    /// [`fm_api::LibraryService::list_hidden_copies`].
    pub fn hidden_copies(&self) -> ApiResult<Vec<HiddenCopy>> {
        Ok(self.lock().hidden_copies()?)
    }

    /// [`fm_api::LibraryService::show_hidden_copies`]: a copy a session hid
    /// is undone in that session, so its merge is taken back as an undo
    /// does; one read from a sidecar is shown at once. Answers the copies
    /// shown.
    pub fn show_hidden(&self, req: &ShowHiddenRequest) -> ApiResult<Vec<HiddenCopy>> {
        let all = self.hidden_copies()?;
        let mut shown = Vec::new();
        for p in &req.paths {
            let copy = all
                .iter()
                .find(|h| h.path == *p)
                .ok_or_else(|| ApiError::invalid(format!("{p} is not a hidden copy.")))?;
            shown.push(copy.clone());
        }
        let mut sessionless: Vec<std::path::PathBuf> = Vec::new();
        for copy in &shown {
            // A crash between the hide and its session log leaves a session
            // that does not name the copy: it is shown here, not undone.
            if copy
                .session
                .is_some_and(|s| self.running.sessions().contains(&s))
            {
                return Err(ApiError::new(
                    ErrorCode::Busy,
                    "This cleanup is still being applied. Cancel it or wait, then show the copy.",
                ));
            }
            let logged = match copy.session {
                Some(session) => self
                    .lock()
                    .session_moves(session)?
                    .iter()
                    .any(|m| m.hidden && m.stored.row.path == copy.path),
                None => false,
            };
            if let (Some(session), true) = (copy.session, logged) {
                self.undo_session(&UndoCleanupRequest {
                    session,
                    paths: vec![copy.path.clone()],
                })?;
            }
            // Not in a session, or the session no longer names it: shown
            // here.
            if let Some(h) = self.lock().hidden_at(&copy.path)? {
                sessionless.push(h.path);
            }
        }
        if !sessionless.is_empty() {
            let mut index = self.lock();
            for path in &sessionless {
                index.show_hidden(path)?;
            }
            let refs: Vec<String> = sessionless
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let refs: Vec<&str> = refs.iter().map(String::as_str).collect();
            let added: Vec<FaceId> = index
                .copy_files(&refs, &self.system)?
                .values()
                .flat_map(|f| f.faces.iter().copied())
                .collect();
            drop(index);
            if !added.is_empty() {
                (self.emit)(Event::LibraryChanged {
                    from_watcher: false,
                    source: None,
                    added,
                    removed: Vec::new(),
                    updated: Vec::new(),
                });
            }
        }
        Ok(shown)
    }
}

/// Whether the file at `path` still has the blake3 hash `hex`.
fn same_hash(path: &std::path::Path, hex: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| blake3::hash(&bytes).to_hex().as_str() == hex)
}
