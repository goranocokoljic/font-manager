//! The mock's cleanup requests (E56.7): plan through [`super::cleanup::plan`],
//! apply at once on the caller's thread, sessions in memory. "Keep this one"
//! actions are sessions of one set, as in fm-core.

use std::collections::{BTreeSet, HashMap};

use super::{ACTIVE_COPY, MOCK_NOW, MockApi, cleanup, duplicate_sets};
use crate::activation::ActivationMode;
use crate::cleanup::{
    ApplyCleanupRequest, CLEANUP_PLAN_VERSION, CleanupAction, CleanupFile, CleanupPlan,
    CleanupScope, CleanupSession, PlanCleanupRequest, SessionFile, SessionKind, UndoCleanupRequest,
};
use crate::event::Event;
use crate::library::{DuplicateScope, Resolution, TrashMoveStatus};
use crate::settings::MyLanguagesStatus;
use crate::{ApiError, ApiResult, ErrorCode, FaceId, JobId, ResolutionId, SourceId};

/// A "Keep this one" action as a session of one set: the name fm-core
/// gives it, its copies with the kept copy.
pub(super) fn resolution_session(r: &Resolution) -> CleanupSession {
    let file = std::path::Path::new(&r.kept)
        .file_name()
        .map_or_else(|| r.kept.clone(), |n| n.to_string_lossy().into_owned());
    let trashed = r
        .moves
        .iter()
        .filter(|m| m.status == TrashMoveStatus::Trashed)
        .count();
    CleanupSession {
        id: r.id.unwrap_or(ResolutionId(0)),
        name: format!("Keep {file}"),
        at: r.at.clone(),
        kind: SessionKind::KeepCopy,
        cancelled: false,
        running: false,
        undone: r.undone,
        sets: u32::from(!r.moves.is_empty()),
        trashed: u32::try_from(trashed).unwrap_or(u32::MAX),
        frees: 0,
        files: r
            .moves
            .iter()
            .map(|m| SessionFile {
                path: m.path.clone(),
                set: None,
                kept: r.kept.clone(),
                size: None,
                status: m.status,
                message: m.message.clone(),
            })
            .collect(),
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The trashed files' count and bytes of `files`.
fn trashed(files: &[SessionFile]) -> (u32, u32) {
    let t: Vec<&SessionFile> = files
        .iter()
        .filter(|f| f.status == TrashMoveStatus::Trashed)
        .collect();
    (
        count(t.len()),
        t.iter()
            .fold(0u32, |sum, f| sum.saturating_add(f.size.unwrap_or(0))),
    )
}

impl MockApi {
    pub(super) fn mock_plan_cleanup(&self, req: PlanCleanupRequest) -> ApiResult<CleanupPlan> {
        let scope = match &req.scope {
            CleanupScope::Source { source } => DuplicateScope::Source { source: *source },
            _ => DuplicateScope::Library,
        };
        let files = self.duplicate_set_files(&scope)?;
        let all = duplicate_sets::sets(&files, &DuplicateScope::Library);
        let source_of: HashMap<&str, SourceId> =
            files.iter().map(|f| (f.path.as_str(), f.source)).collect();
        let in_scope = |path: &str| match &req.scope {
            CleanupScope::Source { source } => source_of.get(path) == Some(source),
            CleanupScope::Folder { path: dir } => std::path::Path::new(path).starts_with(dir),
            _ => true,
        };
        let (active, languages) = {
            let st = self.state();
            let active: HashMap<String, ActivationMode> = st
                .faces
                .iter()
                .filter_map(|f| {
                    st.active
                        .get(&f.detail.face.id)
                        .map(|a| (f.detail.path.clone(), a.mode))
                })
                .collect();
            (active, MyLanguagesStatus::of(&st.settings.my_languages))
        };
        let (sets, files) = cleanup::plan(&all, &req, &active, &in_scope)?;
        let frees = files
            .iter()
            .filter(|f| f.action == CleanupAction::Trash)
            .fold(0u32, |sum, f| sum.saturating_add(f.size));
        Ok(CleanupPlan {
            version: CLEANUP_PLAN_VERSION,
            planned_at: MOCK_NOW.to_owned(),
            request: req,
            sets,
            files,
            conflicts: Vec::new(),
            frees,
            my_languages: languages,
        })
    }

    pub(super) fn mock_apply_cleanup(&self, req: ApplyCleanupRequest) -> ApiResult<JobId> {
        req.plan.check()?;
        if req
            .plan
            .files
            .iter()
            .any(|f| f.action == CleanupAction::Hide)
        {
            return Err(ApiError::new(
                ErrorCode::Unsupported,
                "Hiding a copy is not available yet. Plan the cleanup with the trash.",
            ));
        }
        if !req.choices.is_empty() {
            return Err(ApiError::invalid(
                "A choice names a value that is not a conflict of this plan.",
            ));
        }
        let mut st = self.state();
        st.mutations.push("apply_cleanup");
        let job = JobId(st.next());
        let session = ResolutionId(st.next());
        let leaving: Vec<&CleanupFile> = req.plan.leaving().collect();
        let total = count(leaving.len());
        let mut out = Vec::new();
        let mut removed = Vec::new();
        for (done, f) in leaving.iter().enumerate() {
            let faces: Vec<FaceId> = st
                .faces
                .iter()
                .filter(|x| x.detail.path == f.path)
                .map(|x| x.detail.face.id)
                .collect();
            let hash = st
                .faces
                .iter()
                .find(|x| x.detail.path == f.path)
                .map(|x| x.detail.hash.clone());
            let active = faces.iter().any(|x| st.active.contains_key(x));
            let (status, message) = if st.trashed.contains(&f.path) || hash.is_none() {
                (
                    TrashMoveStatus::Skipped,
                    Some("The file is no longer in the library."),
                )
            } else if hash.as_deref() != Some(f.hash.as_str()) {
                (
                    TrashMoveStatus::Skipped,
                    Some("The file changed since the plan was made, so it was not moved."),
                )
            } else if active && !f.activation.as_ref().is_some_and(|a| a.planned) {
                (TrashMoveStatus::Skipped, Some(ACTIVE_COPY))
            } else {
                for x in &faces {
                    st.active.remove(x);
                }
                removed.extend(faces);
                (TrashMoveStatus::Trashed, None)
            };
            if status == TrashMoveStatus::Trashed {
                st.trashed.insert(f.path.clone());
            }
            out.push(SessionFile {
                path: f.path.clone(),
                set: Some(f.set.clone()),
                kept: f.kept.clone().unwrap_or_default(),
                size: Some(f.size),
                status,
                message: message.map(str::to_owned),
            });
            st.emit(Event::CleanupProgress {
                job,
                session: Some(session),
                done: count(done + 1),
                total,
                path: Some(f.path.clone()),
                finished: false,
                cancelled: false,
            });
        }
        let sets: BTreeSet<&str> = out.iter().filter_map(|f| f.set.as_deref()).collect();
        let sets = count(sets.len());
        let (moved, frees) = trashed(&out);
        let name = req
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(|| format!("Cleanup of {}", &MOCK_NOW[..10]), str::to_owned);
        st.sessions.push(CleanupSession {
            id: session,
            name,
            at: MOCK_NOW.to_owned(),
            kind: SessionKind::Cleanup,
            cancelled: false,
            running: false,
            undone: false,
            sets,
            trashed: moved,
            frees,
            files: out,
        });
        if !removed.is_empty() {
            st.emit(Event::LibraryChanged {
                from_watcher: false,
                source: None,
                added: Vec::new(),
                removed,
                updated: Vec::new(),
            });
        }
        st.emit(Event::CleanupProgress {
            job,
            session: Some(session),
            done: total,
            total,
            path: None,
            finished: true,
            cancelled: false,
        });
        Ok(job)
    }

    pub(super) fn mock_list_cleanup_sessions(&self) -> Vec<CleanupSession> {
        let st = self.state();
        let mut out: Vec<CleanupSession> = st
            .resolutions
            .iter()
            .map(resolution_session)
            .chain(st.sessions.iter().cloned())
            .collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.id.0));
        out
    }

    pub(super) fn mock_undo_cleanup_session(
        &self,
        req: UndoCleanupRequest,
    ) -> ApiResult<CleanupSession> {
        let mut st = self.state();
        let mut back = Vec::new();
        let answer = if let Some(i) = st
            .resolutions
            .iter()
            .position(|r| r.id == Some(req.session))
        {
            let mut r = st.resolutions[i].clone();
            for p in &req.paths {
                if !r.moves.iter().any(|m| m.path == *p) {
                    return Err(ApiError::invalid(format!(
                        "{p} is not a file of this session."
                    )));
                }
            }
            for m in &mut r.moves {
                if (req.paths.is_empty() || req.paths.contains(&m.path))
                    && m.status == TrashMoveStatus::Trashed
                {
                    st.trashed.remove(&m.path);
                    back.push(m.path.clone());
                    m.status = TrashMoveStatus::Restored;
                }
            }
            r.undone = !r.moves.iter().any(|m| m.status == TrashMoveStatus::Trashed);
            st.resolutions[i] = r.clone();
            resolution_session(&r)
        } else {
            let i = st
                .sessions
                .iter()
                .position(|s| s.id == req.session)
                .ok_or_else(|| ApiError::not_found(format!("Session {}", req.session)))?;
            let mut session = st.sessions[i].clone();
            for p in &req.paths {
                if !session.files.iter().any(|f| f.path == *p) {
                    return Err(ApiError::invalid(format!(
                        "{p} is not a file of this session."
                    )));
                }
            }
            for f in &mut session.files {
                if (req.paths.is_empty() || req.paths.contains(&f.path))
                    && f.status == TrashMoveStatus::Trashed
                {
                    st.trashed.remove(&f.path);
                    back.push(f.path.clone());
                    f.status = TrashMoveStatus::Restored;
                }
            }
            let (moved, frees) = trashed(&session.files);
            session.trashed = moved;
            session.frees = frees;
            session.undone = moved == 0;
            st.sessions[i] = session.clone();
            session
        };
        let added: Vec<FaceId> = st
            .faces
            .iter()
            .filter(|f| back.contains(&f.detail.path))
            .map(|f| f.detail.face.id)
            .collect();
        if !added.is_empty() {
            st.emit(Event::LibraryChanged {
                from_watcher: false,
                source: None,
                added,
                removed: Vec::new(),
                updated: Vec::new(),
            });
        }
        Ok(answer)
    }
}
