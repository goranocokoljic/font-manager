//! The activation service (E6.6): activates and deactivates faces by face,
//! family, collection or set through a [`FontActivator`], and keeps the
//! activation table in step with what the OS holds (ADR 0008).
//!
//! - **Rule 3.** A row is written only after the OS registration worked and
//!   removed only after it was undone; deactivation acts on rows only, so a
//!   face the app did not register comes back `Skipped`.
//! - **Files, not faces.** The OS registers whole files, so the faces of a
//!   collection file share one registration. The file is deregistered when
//!   the last of its active faces is deactivated.
//! - **Sessions.** [`ActivationManager::shutdown`] deregisters the session
//!   faces at quit and remembers them; [`ActivationManager::start`] cleans up
//!   what a crash left and, with "reactivate on launch", activates the last
//!   session's faces again.
//! - **Events.** Every change is sent as one [`Event::ActivationChanged`].

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fm_api::activation::{
    ActivateRequest, ActivationMode, ActivationOutcome, ActivationService, ActiveFace, Conflict,
    ConflictPolicy, ConflictWith, DeactivateRequest, OutcomeStatus, Target,
};
use fm_api::common::FontFormat;
use fm_api::event::Event;
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId};
use fm_platform::{
    ActivationError, ActivationErrorKind, FaceFile, FontActivator, Outcome, OutcomeStatus as Os,
    Registration,
};

use crate::error::{FmError, FmResult};
use crate::index::{ActiveRow, FaceRow, Index, platform_mode};

/// Where activation changes go: the app's event stream.
pub type Emit = Arc<dyn Fn(Event) + Send + Sync>;

/// The reason a face the app did not register is not deactivated.
const NOT_OURS: &str = "The app did not activate this font.";

/// Activates faces through the OS and records them in the index.
pub struct ActivationManager {
    index: Arc<Mutex<Index>>,
    activator: Arc<dyn FontActivator>,
    emit: Emit,
}

impl std::fmt::Debug for ActivationManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivationManager").finish_non_exhaustive()
    }
}

impl ActivationManager {
    /// A service over `index` that registers through `activator` and sends
    /// every change to `emit`.
    pub fn new(
        index: Arc<Mutex<Index>>,
        activator: Arc<dyn FontActivator>,
        emit: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        Self {
            index,
            activator,
            emit: Arc::new(emit),
        }
    }

    /// What the app does at start, before anything else touches activation
    /// (ADR 0008): removes the session registrations an earlier run left
    /// (a crash skips [`ActivationManager::shutdown`]) and remembers their
    /// faces as the last session; then, when `reactivate` is set ("reactivate
    /// on launch"), activates the last session's faces again for this session.
    /// Returns the outcomes of that reactivation (empty when off).
    pub fn start(
        &self,
        reactivate: bool,
        on_conflict: ConflictPolicy,
    ) -> FmResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        match self.activator.clear_session() {
            Ok(outcomes) => log_failures("clearing the last session", &outcomes),
            // Nothing could be cleared; the rows below still go, so the table
            // does not claim fonts this run did not register.
            Err(e) => tracing::warn!(error = %e, detail = %e.detail, "session cleanup failed"),
        }
        let left: Vec<FaceId> = index
            .active_rows()?
            .into_iter()
            .filter(|r| r.mode == ActivationMode::Session)
            .map(|r| r.face)
            .collect();
        if !left.is_empty() {
            tracing::info!(
                faces = left.len(),
                "cleaned up session fonts of a crashed run"
            );
            index.set_last_session(&left)?;
            index.remove_activations(&left)?;
        }
        if !reactivate {
            return Ok(Vec::new());
        }
        let faces = index.last_session()?;
        if faces.is_empty() {
            return Ok(Vec::new());
        }
        let req = ActivateRequest {
            target: Target::Faces { faces },
            mode: ActivationMode::Session,
            on_conflict,
        };
        self.activate_locked(&mut index, &req)
    }

    /// What the app does at quit: remembers the session-active faces for
    /// "reactivate on launch" and deregisters them. Permanent ones stay.
    pub fn shutdown(&self) -> FmResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        let session: Vec<FaceId> = index
            .active_rows()?
            .into_iter()
            .filter(|r| r.mode == ActivationMode::Session)
            .map(|r| r.face)
            .collect();
        index.set_last_session(&session)?;
        self.deactivate_locked(&mut index, session)
    }

    fn lock(&self) -> MutexGuard<'_, Index> {
        // A panic elsewhere must not leave activation unusable; the table is
        // only changed in transactions.
        self.index.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn activate_locked(
        &self,
        index: &mut Index,
        req: &ActivateRequest,
    ) -> FmResult<Vec<ActivationOutcome>> {
        let ids = index.target_faces(&req.target)?;
        let rows = index.activation_faces(&ids)?;
        let active = by_face(index.active_rows()?);
        let mut results: Vec<Option<OutcomeStatus>> = vec![None; rows.len()];

        // Faces to register, and faces active in the other mode to switch.
        let mut wanted = Vec::new();
        let mut switching: HashMap<FaceId, &Registration> = HashMap::new();
        for (i, row) in rows.iter().enumerate() {
            if let Some(reason) = unusable(row) {
                results[i] = Some(skipped(reason));
                continue;
            }
            match active.get(&row.id) {
                Some(a) if a.mode == req.mode => results[i] = Some(OutcomeStatus::Unchanged),
                Some(a) => {
                    if let Some(reg) = &a.registration {
                        switching.insert(row.id, reg);
                    }
                    wanted.push(i);
                }
                None => wanted.push(i),
            }
        }
        // A file shared with an active face outside the switch keeps its mode.
        let blocked: HashSet<FaceId> = switching
            .iter()
            .filter(|(_, reg)| {
                active.values().any(|a| {
                    a.registration
                        .as_ref()
                        .is_some_and(|r| r.handle == reg.handle)
                        && !switching.contains_key(&a.face)
                })
            })
            .map(|(face, _)| *face)
            .collect();
        wanted.retain(|&i| {
            let face = rows[i].id;
            if blocked.contains(&face) {
                results[i] = Some(skipped(
                    "Other fonts of this file are active in the other mode; deactivate them first."
                        .to_owned(),
                ));
            }
            !blocked.contains(&face)
        });

        if req.on_conflict == ConflictPolicy::KeepExisting && !wanted.is_empty() {
            let files: Vec<FaceFile> = wanted.iter().map(|&i| face_file(&rows[i])).collect();
            let found = self.activator.conflicts(&files).map_err(platform_error)?;
            let clashes = clash_map(&found, &active);
            wanted.retain(|&i| {
                let row = &rows[i];
                let with = clashes.get(&(row.path.clone(), row.postscript_name.clone()));
                if let Some(with) = with {
                    results[i] = Some(OutcomeStatus::Conflict { with: with.clone() });
                }
                with.is_none()
            });
        }

        // Release the registrations of the faces that change mode.
        let mut released: Vec<FaceId> = Vec::new();
        let releases: Vec<(Vec<FaceId>, Registration)> = group_by_handle(
            wanted
                .iter()
                .filter_map(|&i| switching.get(&rows[i].id).map(|r| (rows[i].id, *r))),
        );
        if !releases.is_empty() {
            let regs: Vec<Registration> = releases.iter().map(|(_, r)| r.clone()).collect();
            let outcomes = self.activator.deactivate(&regs).map_err(platform_error)?;
            for ((faces, _), outcome) in releases.iter().zip(&outcomes) {
                if let Os::Failed(err) = &outcome.status {
                    let failed: HashSet<&FaceId> = faces.iter().collect();
                    wanted.retain(|&i| {
                        let hit = failed.contains(&rows[i].id);
                        if hit {
                            results[i] = Some(failed_status(err));
                        }
                        !hit
                    });
                } else {
                    released.extend(faces);
                }
            }
        }

        let files: Vec<FaceFile> = wanted.iter().map(|&i| face_file(&rows[i])).collect();
        let outcomes = if files.is_empty() {
            Vec::new()
        } else {
            match self.activator.activate(&files, platform_mode(req.mode)) {
                Ok(outcomes) => outcomes,
                Err(e) => {
                    // The released faces are no longer registered.
                    index.remove_activations(&released)?;
                    self.send(Vec::new(), released);
                    return Err(platform_error(e));
                }
            }
        };
        let mut record = Vec::new();
        let mut fresh = Vec::new();
        for (&i, outcome) in wanted.iter().zip(&outcomes) {
            let face = rows[i].id;
            results[i] = Some(match &outcome.status {
                Os::Activated(reg) => {
                    fresh.push(reg.clone());
                    record.push((face, req.mode, reg.clone()));
                    OutcomeStatus::Done
                }
                Os::AlreadyActive(reg) if reg.mode == platform_mode(req.mode) => {
                    record.push((face, req.mode, reg.clone()));
                    OutcomeStatus::Done
                }
                Os::AlreadyActive(_) => skipped(
                    "Another font of this file is active in the other mode; deactivate it first."
                        .to_owned(),
                ),
                Os::Failed(err) => failed_status(err),
                other => failed_status(&ActivationError::new(
                    ActivationErrorKind::Os,
                    format!("unexpected activation outcome {other:?}"),
                )),
            });
        }
        if let Err(e) = index.record_activations(&record) {
            // Never leave a registration the table does not list (rule 3).
            if let Err(undo) = self.activator.deactivate(&fresh) {
                tracing::error!(error = %undo, "could not undo activations after a failed write");
            }
            index.remove_activations(&released)?;
            self.send(Vec::new(), released);
            return Err(e);
        }
        // A released face that is not active again is now inactive.
        let recorded: HashSet<FaceId> = record.iter().map(|(f, _, _)| *f).collect();
        let lost: Vec<FaceId> = released
            .into_iter()
            .filter(|f| !recorded.contains(f))
            .collect();
        index.remove_activations(&lost)?;
        self.send(record.iter().map(|(f, _, _)| *f).collect(), lost);
        Ok(finish(&rows, results))
    }

    fn deactivate_locked(
        &self,
        index: &mut Index,
        ids: Vec<FaceId>,
    ) -> FmResult<Vec<ActivationOutcome>> {
        let active = by_face(index.active_rows()?);
        let requested: HashSet<FaceId> = ids.iter().copied().collect();
        let mut results: HashMap<FaceId, OutcomeStatus> = HashMap::new();
        let mut done: Vec<FaceId> = Vec::new();
        let mut owned = Vec::new();
        for &face in &ids {
            match active.get(&face) {
                None => {
                    results.insert(face, skipped(NOT_OURS.to_owned()));
                }
                // Nothing recorded to deregister: only the row goes.
                Some(ActiveRow {
                    registration: None, ..
                }) => {
                    results.insert(face, OutcomeStatus::Done);
                    done.push(face);
                }
                Some(ActiveRow {
                    registration: Some(reg),
                    ..
                }) => {
                    let shared = active.values().any(|o| {
                        !requested.contains(&o.face)
                            && o.registration
                                .as_ref()
                                .is_some_and(|r| r.handle == reg.handle)
                    });
                    if shared {
                        // Another active face still needs the file.
                        results.insert(face, OutcomeStatus::Done);
                        done.push(face);
                    } else {
                        owned.push((face, reg));
                    }
                }
            }
        }
        let groups = group_by_handle(owned.into_iter());
        if !groups.is_empty() {
            let regs: Vec<Registration> = groups.iter().map(|(_, r)| r.clone()).collect();
            let outcomes = self.activator.deactivate(&regs).map_err(platform_error)?;
            for ((faces, _), outcome) in groups.iter().zip(&outcomes) {
                let status = match &outcome.status {
                    Os::Deactivated | Os::NotActive => OutcomeStatus::Done,
                    // The table listed it, the activator does not know it: the
                    // app does not touch it and stops claiming it.
                    Os::NotOurs => skipped(
                        "The system no longer lists this font as activated by the app.".to_owned(),
                    ),
                    Os::Failed(err) => failed_status(err),
                    other => failed_status(&ActivationError::new(
                        ActivationErrorKind::Os,
                        format!("unexpected deactivation outcome {other:?}"),
                    )),
                };
                for &face in faces {
                    if !matches!(status, OutcomeStatus::Failed { .. }) {
                        done.push(face);
                    }
                    results.insert(face, status.clone());
                }
            }
        }
        index.remove_activations(&done)?;
        let gone: Vec<FaceId> = ids
            .iter()
            .copied()
            .filter(|f| results.get(f) == Some(&OutcomeStatus::Done))
            .collect();
        self.send(Vec::new(), gone);
        Ok(ids
            .into_iter()
            .map(|face| ActivationOutcome {
                face,
                outcome: results
                    .remove(&face)
                    .unwrap_or_else(|| skipped(NOT_OURS.to_owned())),
            })
            .collect())
    }

    fn send(&self, activated: Vec<FaceId>, deactivated: Vec<FaceId>) {
        if !activated.is_empty() || !deactivated.is_empty() {
            (self.emit)(Event::ActivationChanged {
                activated,
                deactivated,
            });
        }
    }
}

impl ActivationService for ActivationManager {
    fn activate(&self, req: ActivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        Ok(self.activate_locked(&mut index, &req)?)
    }

    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        let ids = match &req.target {
            Some(target) => index.target_faces(target)?,
            None => index.active_rows()?.into_iter().map(|r| r.face).collect(),
        };
        Ok(self.deactivate_locked(&mut index, ids)?)
    }

    fn list_active(&self) -> ApiResult<Vec<ActiveFace>> {
        Ok(self
            .lock()
            .active_rows()?
            .into_iter()
            .map(|r| ActiveFace {
                face: r.face,
                mode: r.mode,
                since: r.since,
                document: r.document,
            })
            .collect())
    }

    fn conflicts(&self, target: Target) -> ApiResult<Vec<Conflict>> {
        let index = self.lock();
        let ids = index.target_faces(&target)?;
        let rows: Vec<FaceRow> = index
            .activation_faces(&ids)?
            .into_iter()
            .filter(|r| unusable(r).is_none())
            .collect();
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let active = by_face(index.active_rows()?);
        let files: Vec<FaceFile> = rows.iter().map(face_file).collect();
        let found = self.activator.conflicts(&files).map_err(platform_error)?;
        let clashes = clash_map(&found, &active);
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let key = (row.path, row.postscript_name);
                clashes.get(&key).map(|with| Conflict {
                    face: row.id,
                    postscript_name: key.1.clone(),
                    with: with.clone(),
                })
            })
            .collect())
    }
}

/// Why a face cannot be activated, or `None` when it can.
fn unusable(row: &FaceRow) -> Option<String> {
    if row.gone {
        return Some("The font file is missing.".to_owned());
    }
    if row.paused {
        return Some("The folder of this font is paused.".to_owned());
    }
    if let Some(reason) = &row.unhealthy {
        return Some(reason.clone());
    }
    match row.format {
        Some(FontFormat::Ttf | FontFormat::Otf | FontFormat::Ttc | FontFormat::Otc) => None,
        Some(FontFormat::Woff | FontFormat::Woff2) => {
            Some("Web fonts (WOFF) cannot be activated yet.".to_owned())
        }
        _ => Some("This font format cannot be activated.".to_owned()),
    }
}

fn face_file(row: &FaceRow) -> FaceFile {
    FaceFile {
        path: row.path.clone(),
        postscript_name: row.postscript_name.clone(),
    }
}

fn by_face(rows: Vec<ActiveRow>) -> HashMap<FaceId, ActiveRow> {
    rows.into_iter().map(|r| (r.face, r)).collect()
}

/// The first clash of each (file, PostScript name): with one of the app's
/// active faces when the other file is one the app registered, else with
/// the system or user font it is.
fn clash_map(
    found: &[fm_platform::Conflict],
    active: &HashMap<FaceId, ActiveRow>,
) -> HashMap<(PathBuf, String), ConflictWith> {
    let ours: HashMap<&PathBuf, FaceId> = active
        .values()
        .filter_map(|a| a.registration.as_ref().map(|r| (&r.path, a.face)))
        .collect();
    let mut out = HashMap::new();
    for c in found {
        let with = match ours.get(&c.with) {
            Some(&face) => ConflictWith::Active { face },
            None => ConflictWith::System {
                path: c.with.display().to_string(),
            },
        };
        out.entry((c.path.clone(), c.postscript_name.clone()))
            .or_insert(with);
    }
    out
}

/// One registration per handle, with every face that shares it, in first-seen order.
fn group_by_handle<'a>(
    items: impl Iterator<Item = (FaceId, &'a Registration)>,
) -> Vec<(Vec<FaceId>, Registration)> {
    let mut at: HashMap<OsString, usize> = HashMap::new();
    let mut out: Vec<(Vec<FaceId>, Registration)> = Vec::new();
    for (face, reg) in items {
        match at.get(&reg.handle) {
            Some(&i) => out[i].0.push(face),
            None => {
                at.insert(reg.handle.clone(), out.len());
                out.push((vec![face], reg.clone()));
            }
        }
    }
    out
}

fn finish(rows: &[FaceRow], results: Vec<Option<OutcomeStatus>>) -> Vec<ActivationOutcome> {
    rows.iter()
        .zip(results)
        .map(|(row, outcome)| ActivationOutcome {
            face: row.id,
            outcome: outcome.unwrap_or_else(|| {
                failed_status(&ActivationError::new(
                    ActivationErrorKind::Os,
                    "the activator returned fewer outcomes than faces",
                ))
            }),
        })
        .collect()
}

fn skipped(reason: String) -> OutcomeStatus {
    OutcomeStatus::Skipped { reason }
}

fn failed_status(err: &ActivationError) -> OutcomeStatus {
    OutcomeStatus::Failed {
        error: api_error(err),
    }
}

fn api_error(err: &ActivationError) -> ApiError {
    let code = match err.kind {
        ActivationErrorKind::FileNotFound => ErrorCode::NotFound,
        ActivationErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        ActivationErrorKind::Unsupported => ErrorCode::Unsupported,
        ActivationErrorKind::Os => ErrorCode::Io,
    };
    ApiError::new(code, err.to_string()).with_detail(err.detail.clone())
}

fn platform_error(err: ActivationError) -> FmError {
    api_error(&err).into()
}

fn log_failures(what: &str, outcomes: &[Outcome]) {
    for o in outcomes {
        if let Os::Failed(err) = &o.status {
            tracing::warn!(path = %o.path.display(), error = %err, detail = %err.detail, "{what}");
        }
    }
}

#[cfg(test)]
mod tests;
