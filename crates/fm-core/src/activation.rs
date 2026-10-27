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
//! - **Documents (E29.3).** A face `fonts_needed` activates for a document
//!   is document-scoped: tagged with every open document that asks for it,
//!   offered for deactivation by [`ActivationService::document_closed`] when
//!   none is left, and never restored at launch. A face the user activates
//!   is the user's again, whoever activated it first.
//! - **Events.** Every change is sent as one [`Event::ActivationChanged`].

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fm_api::activation::{
    ActivateRequest, ActivationMode, ActivationOutcome, ActivationService, ActiveFace, Clash,
    ClashWinner, Conflict, ConflictPolicy, ConflictWith, DeactivateRequest, DocumentClosedRequest,
    DocumentClosedResponse, FontsNeededRequest, FontsNeededResponse, OutcomeStatus, Target,
};
use fm_api::common::FontFormat;
use fm_api::event::Event;
use fm_api::library::Problem;
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId};
use fm_platform::{
    ActivationError, ActivationErrorKind, FaceFile, FontActivator, Outcome, OutcomeStatus as Os,
    Registration,
};

use crate::error::{FmError, FmResult};
use crate::index::{ActiveRow, FaceRow, Index, platform_mode};
use crate::needed;

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
    /// (a crash skips [`ActivationManager::shutdown`]) and remembers the
    /// faces the user activated of them as the last session; then, when `reactivate` is set ("reactivate
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
        let left: Vec<ActiveRow> = index
            .active_rows()?
            .into_iter()
            .filter(|r| r.mode == ActivationMode::Session)
            .collect();
        if !left.is_empty() {
            tracing::info!(
                faces = left.len(),
                "cleaned up session fonts of a crashed run"
            );
            index.set_last_session(&users(&left))?;
            let faces: Vec<FaceId> = left.iter().map(|r| r.face).collect();
            index.remove_activations(&faces)?;
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

    /// What the app does at quit: remembers the session faces the user
    /// activated for "reactivate on launch" and deregisters every session
    /// face, the document-scoped ones too. Permanent ones stay.
    pub fn shutdown(&self) -> FmResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        let session: Vec<ActiveRow> = index
            .active_rows()?
            .into_iter()
            .filter(|r| r.mode == ActivationMode::Session)
            .collect();
        index.set_last_session(&users(&session))?;
        self.deactivate_locked(&mut index, session.into_iter().map(|r| r.face).collect())
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

    /// "Replace for this session": a permanent request with `Replace`
    /// activates its clashing faces in session mode and the others
    /// permanently, so the replaced font is back after the session.
    fn replace_for_session(
        &self,
        index: &mut Index,
        req: &ActivateRequest,
    ) -> FmResult<Vec<ActivationOutcome>> {
        let ids = index.target_faces(&req.target)?;
        let active = by_face(index.active_rows()?);
        // A face already permanent stays as it is.
        let open: Vec<FaceId> = ids
            .iter()
            .copied()
            .filter(|f| active.get(f).is_none_or(|a| a.mode != req.mode))
            .collect();
        let (rows, clashes) = self.clashes_of(index, &open)?;
        let clashing: HashSet<FaceId> = rows
            .iter()
            .filter(|r| clashes.contains_key(&(r.path.clone(), r.postscript_name.clone())))
            .map(|r| r.id)
            .collect();
        if clashing.is_empty() {
            return self.activate_locked(index, req);
        }
        let (session, permanent): (Vec<FaceId>, Vec<FaceId>) =
            ids.iter().copied().partition(|f| clashing.contains(f));
        let mut results: HashMap<FaceId, OutcomeStatus> = HashMap::new();
        for (faces, mode) in [
            (permanent, ActivationMode::Permanent),
            (session, ActivationMode::Session),
        ] {
            if faces.is_empty() {
                continue;
            }
            let part = ActivateRequest {
                target: Target::Faces { faces },
                mode,
                on_conflict: ConflictPolicy::Replace,
            };
            for o in self.activate_locked(index, &part)? {
                results.insert(o.face, o.outcome);
            }
        }
        Ok(ids
            .into_iter()
            .map(|face| ActivationOutcome {
                face,
                outcome: results
                    .get(&face)
                    .cloned()
                    .unwrap_or_else(|| skipped(NOT_OURS.to_owned())),
            })
            .collect())
    }

    /// The faces of `ids` that can be activated, with the first clash of
    /// each (by file and PostScript name). Activates nothing.
    fn clashes_of(&self, index: &Index, ids: &[FaceId]) -> FmResult<(Vec<FaceRow>, ClashMap)> {
        let rows: Vec<FaceRow> = index
            .activation_faces(ids)?
            .into_iter()
            .filter(|r| unusable(r).is_none())
            .collect();
        if rows.is_empty() {
            return Ok((rows, HashMap::new()));
        }
        let active = by_face(index.active_rows()?);
        let files: Vec<FaceFile> = rows.iter().map(face_file).collect();
        let found = self.activator.conflicts(&files).map_err(platform_error)?;
        Ok((rows, clash_map(&found, &active)))
    }

    /// The Problems entries of the activation conflicts (E8.2): one per row
    /// of [`ActivationService::list_conflicts`], with the files of the faces
    /// involved. Changes nothing.
    pub fn problems(&self) -> ApiResult<Vec<Problem>> {
        let clashes = self.list_conflicts()?;
        if clashes.is_empty() {
            return Ok(Vec::new());
        }
        let mut ids: Vec<FaceId> = Vec::new();
        for c in &clashes {
            ids.push(c.face);
            if let ConflictWith::Active { face } = c.with {
                ids.push(face);
            }
        }
        ids.sort();
        ids.dedup();
        let paths: HashMap<FaceId, String> = self
            .lock()
            .activation_faces(&ids)?
            .into_iter()
            .map(|row| (row.id, row.path.display().to_string()))
            .collect();
        let path = |face: FaceId| paths.get(&face).cloned();
        Ok(clashes
            .iter()
            .map(|c| {
                let other = match c.with {
                    ConflictWith::Active { face } => path(face),
                    ConflictWith::System { .. } => None,
                };
                c.problem(path(c.face).unwrap_or_default(), other)
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
        let outcomes = if req.on_conflict == ConflictPolicy::Replace
            && req.mode == ActivationMode::Permanent
        {
            self.replace_for_session(&mut index, &req)?
        } else {
            self.activate_locked(&mut index, &req)?
        };
        // The user activated these: they are the user's now, also when a
        // document activated them first (E29.3).
        let took: Vec<FaceId> = outcomes
            .iter()
            .filter(|o| matches!(o.outcome, OutcomeStatus::Done | OutcomeStatus::Unchanged))
            .map(|o| o.face)
            .collect();
        index.clear_documents(&took)?;
        Ok(outcomes)
    }

    fn deactivate(&self, req: DeactivateRequest) -> ApiResult<Vec<ActivationOutcome>> {
        let mut index = self.lock();
        let mut ids = match &req.target {
            Some(target) => index.target_faces(target)?,
            None => index.active_rows()?.into_iter().map(|r| r.face).collect(),
        };
        if let Some(except) = &req.except {
            let keep: HashSet<FaceId> = index.target_faces(except)?.into_iter().collect();
            ids.retain(|id| !keep.contains(id));
        }
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
        let (rows, clashes) = self.clashes_of(&index, &ids)?;
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

    fn list_conflicts(&self) -> ApiResult<Vec<Clash>> {
        let index = self.lock();
        let active = by_face(index.active_rows()?);
        let mut ids: Vec<FaceId> = active.keys().copied().collect();
        ids.sort();
        let rows = index.activation_faces(&ids)?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let files: Vec<FaceFile> = rows.iter().map(face_file).collect();
        let found = self.activator.conflicts(&files).map_err(platform_error)?;
        let all = clashes(&found, &active);
        let mut out: Vec<Clash> = Vec::new();
        for row in &rows {
            let key = (row.path.clone(), row.postscript_name.clone());
            for (_, with) in all.iter().filter(|(k, _)| *k == key) {
                let winner = match with {
                    // Each pair of the app's faces once, under the lower id.
                    ConflictWith::Active { face: other } if *other < row.id => continue,
                    ConflictWith::Active { face: other } if later(&active, *other, row.id) => {
                        ClashWinner::With
                    }
                    ConflictWith::Active { .. } | ConflictWith::System { .. } => ClashWinner::Face,
                };
                let clash = Clash {
                    face: row.id,
                    postscript_name: row.postscript_name.clone(),
                    with: with.clone(),
                    winner,
                };
                if !out.contains(&clash) {
                    out.push(clash);
                }
            }
        }
        Ok(out)
    }

    fn fonts_needed(&self, req: FontsNeededRequest) -> ApiResult<FontsNeededResponse> {
        let mut index = self.lock();
        let (matched, unmatched) = needed::match_fonts(req.fonts, &index.needed_faces()?);
        let mut outcomes = Vec::new();
        if req.activate && !matched.is_empty() {
            // A face already active stays as it is, in its mode: a session
            // request would turn a permanent activation into a session one.
            let active: HashSet<FaceId> = index.active_rows()?.iter().map(|r| r.face).collect();
            let mut seen = HashSet::new();
            let faces: Vec<FaceId> = matched
                .iter()
                .map(|m| m.face)
                .filter(|f| seen.insert(*f))
                .collect();
            let wanted: Vec<FaceId> = faces
                .iter()
                .copied()
                .filter(|f| !active.contains(f))
                .collect();
            let mut done: HashMap<FaceId, OutcomeStatus> = HashMap::new();
            if !wanted.is_empty() {
                // The Activate button's path: session mode, clashes reported.
                let act = ActivateRequest {
                    target: Target::Faces { faces: wanted },
                    mode: ActivationMode::Session,
                    on_conflict: ConflictPolicy::KeepExisting,
                };
                for o in self.activate_locked(&mut index, &act)? {
                    done.insert(o.face, o.outcome);
                }
            }
            outcomes = faces
                .into_iter()
                .map(|face| ActivationOutcome {
                    face,
                    outcome: done.remove(&face).unwrap_or(OutcomeStatus::Unchanged),
                })
                .collect();
            if let Some(document) = &req.document {
                // The faces this request activated, and the document-scoped
                // ones it asks for again (E29.3). A face the user activated,
                // or a permanent one, stays the user's and takes no tag.
                let active = by_face(index.active_rows()?);
                let tagged: Vec<FaceId> = outcomes
                    .iter()
                    .filter(|o| {
                        active.get(&o.face).is_some_and(|a| {
                            a.mode == ActivationMode::Session
                                && (o.outcome == OutcomeStatus::Done || a.document.is_some())
                        })
                    })
                    .map(|o| o.face)
                    .collect();
                index.tag_document(&tagged, document)?;
            }
        }
        Ok(FontsNeededResponse {
            missing: u32::try_from(unmatched.len()).unwrap_or(u32::MAX),
            matched,
            unmatched,
            outcomes,
        })
    }

    fn document_closed(&self, req: DocumentClosedRequest) -> ApiResult<DocumentClosedResponse> {
        let offer = self.lock().close_document(&req.document)?;
        Ok(DocumentClosedResponse { offer })
    }
}

/// The faces of `rows` the user activated (not only documents, E29.3): the
/// ones "reactivate on launch" brings back.
fn users(rows: &[ActiveRow]) -> Vec<FaceId> {
    rows.iter()
        .filter(|r| r.document.is_none())
        .map(|r| r.face)
        .collect()
}

/// `a` was activated after `b`: the later `since`, then the higher id
/// (one request activates its faces in the same millisecond).
fn later(active: &HashMap<FaceId, ActiveRow>, a: FaceId, b: FaceId) -> bool {
    let since = |f: FaceId| active.get(&f).map(|r| r.since.clone());
    (since(a), a) > (since(b), b)
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

/// Clashes by (file, PostScript name).
type ClashMap = HashMap<(PathBuf, String), ConflictWith>;

/// Every clash, keyed by (file, PostScript name), in the activator's order:
/// with one of the app's active faces when the other file is one the app
/// registered (by the file or by the OS record, such as a Linux link), else
/// with the system or user font it is.
fn clashes(
    found: &[fm_platform::Conflict],
    active: &HashMap<FaceId, ActiveRow>,
) -> Vec<((PathBuf, String), ConflictWith)> {
    // Lowest id first, so a file shared by several faces names the same one.
    let mut faces: Vec<&ActiveRow> = active.values().collect();
    faces.sort_by_key(|a| a.face);
    let mut ours: HashMap<PathBuf, FaceId> = HashMap::new();
    for a in faces {
        if let Some(r) = &a.registration {
            ours.entry(r.path.clone()).or_insert(a.face);
            ours.entry(PathBuf::from(&r.handle)).or_insert(a.face);
        }
    }
    found
        .iter()
        .map(|c| {
            let with = match ours.get(&c.with) {
                Some(&face) => ConflictWith::Active { face },
                None => ConflictWith::System {
                    path: c.with.display().to_string(),
                },
            };
            ((c.path.clone(), c.postscript_name.clone()), with)
        })
        .collect()
}

/// The first clash of each (file, PostScript name); see [`clashes`].
fn clash_map(found: &[fm_platform::Conflict], active: &HashMap<FaceId, ActiveRow>) -> ClashMap {
    let mut out = HashMap::new();
    for (key, with) in clashes(found, active) {
        out.entry(key).or_insert(with);
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
