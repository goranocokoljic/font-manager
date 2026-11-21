//! The cleanup requests of [`CoreApi`] (E56.7): plan, apply on a job,
//! cancel, list and undo sessions. The plan is `crate::cleanup`, the apply
//! and undo `crate::resolve::cleanup`; this file reads what they need.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use fm_api::activation::{
    ActivateRequest, ActivationMode, DeactivateRequest, OutcomeStatus, Target,
};
use fm_api::cleanup::{
    ApplyCleanupRequest, CLEANUP_PLAN_VERSION, CleanupAction, CleanupPlan, CleanupScope,
    CleanupSession, MetadataConflict, MetadataField, MetadataValue, PlanCleanupRequest,
    UndoCleanupRequest,
};
use fm_api::event::Event;
use fm_api::{ActivationService, ApiError, ApiResult, FaceId, JobId};

use super::{CoreApi, folder_roots, lock};
use crate::activation::ActivationManager;
use crate::index::{MergeField, MergeValue};

fn field(f: MergeField) -> MetadataField {
    match f {
        MergeField::Rating => MetadataField::Rating,
        MergeField::License => MetadataField::License,
        MergeField::Notes => MetadataField::Notes,
    }
}

fn value(v: MergeValue) -> MetadataValue {
    match v {
        MergeValue::Rating(stars) => MetadataValue::Rating { stars },
        MergeValue::License(license, seats) => MetadataValue::License { license, seats },
        MergeValue::Notes(text) => MetadataValue::Notes { text },
    }
}

/// Moves an activation from `leaving` to `kept` through `activation`:
/// deactivates first (one font of a name at a time), then activates; when
/// the kept copy cannot be activated, the leaving copy is activated again.
fn move_activation(
    activation: &ActivationManager,
    leaving: &[FaceId],
    kept: &[FaceId],
    mode: ActivationMode,
) -> Result<(), String> {
    let refused =
        "Its activation could not be moved to the copy that is kept, so it was not moved.";
    activation
        .deactivate(DeactivateRequest {
            target: Some(Target::Faces {
                faces: leaving.to_vec(),
            }),
            except: None,
        })
        .map_err(|_| refused.to_owned())?;
    let activate = |faces: &[FaceId]| {
        activation.activate(ActivateRequest {
            target: Target::Faces {
                faces: faces.to_vec(),
            },
            mode,
            on_conflict: fm_api::activation::ConflictPolicy::KeepExisting,
        })
    };
    let ok = activate(kept).is_ok_and(|outcomes| {
        outcomes
            .iter()
            .all(|o| matches!(o.outcome, OutcomeStatus::Done | OutcomeStatus::Unchanged))
    });
    if ok {
        return Ok(());
    }
    if let Err(e) = activate(leaving) {
        tracing::warn!(error = %e.message, "activating a copy again after a failed activation move");
    }
    Err(refused.to_owned())
}

impl CoreApi {
    pub(super) fn cleanup_plan(&self, req: PlanCleanupRequest) -> ApiResult<CleanupPlan> {
        let settings = self.settings.read()?;
        let langs = &settings.my_languages;
        let index = self.lock();
        if let CleanupScope::Source { source } = req.scope
            && !index.sources()?.iter().any(|s| s.id == source)
        {
            return Err(ApiError::not_found(format!("Source {source}")));
        }
        let files = index.duplicate_set_files(&crate::duplicate_sets::system_folders())?;
        let roots = folder_roots(&index)?;
        let facts = if crate::keeper_policy::needs_facts(&settings.keeper_policy, langs) {
            let faces: Vec<FaceId> = files
                .iter()
                .flat_map(|f| f.faces.iter().map(|x| x.face))
                .collect();
            index.copy_facts(&faces)?
        } else {
            HashMap::new()
        };
        let active = index.active_files()?;
        let names = crate::keeper_policy::LanguageNames::load(langs)?;
        let exists = |p: &str| std::fs::symlink_metadata(p).is_ok();
        let ctx = crate::cleanup::PlanContext {
            files: &files,
            roots: &roots,
            facts: &facts,
            active: &active,
            policy: &settings.keeper_policy,
            my_languages: langs,
            names: &names,
            exists: &exists,
        };
        let planned = crate::cleanup::plan(&ctx, &req)?;
        // The metadata conflicts of each set's leaving copies (E56.5).
        let faces_of: HashMap<&str, Vec<FaceId>> = files
            .iter()
            .map(|f| (f.path.as_str(), f.faces.iter().map(|x| x.face).collect()))
            .collect();
        let mut sets: Vec<&str> = Vec::new();
        for f in planned
            .files
            .iter()
            .filter(|f| f.action != CleanupAction::Stays)
        {
            if !sets.contains(&f.set.as_str()) {
                sets.push(&f.set);
            }
        }
        let mut conflicts = Vec::new();
        for set in sets {
            let group: Vec<_> = planned
                .files
                .iter()
                .filter(|f| f.set == set && f.action != CleanupAction::Stays)
                .collect();
            let kept: BTreeSet<&str> = group
                .iter()
                .flat_map(|f| f.kept.iter().chain(f.also_kept.iter()))
                .map(String::as_str)
                .collect();
            let kept: Vec<FaceId> = kept
                .iter()
                .filter_map(|p| faces_of.get(p))
                .flatten()
                .copied()
                .collect();
            let leaving: Vec<FaceId> = group.iter().flat_map(|f| f.faces.iter().copied()).collect();
            for c in index.merge_plan(&kept, &leaving)?.conflicts {
                conflicts.push(MetadataConflict {
                    set: set.to_owned(),
                    kept_face: c.kept_face,
                    field: field(c.field),
                    default_from: c.default_from,
                    default: value(c.default),
                    other_face: c.other_face,
                    other: value(c.other),
                });
            }
        }
        let planned_at = index.now()?;
        drop(index);
        let frees = planned
            .files
            .iter()
            .filter(|f| f.action == CleanupAction::Trash)
            .fold(0u32, |sum, f| sum.saturating_add(f.size));
        Ok(CleanupPlan {
            version: CLEANUP_PLAN_VERSION,
            planned_at,
            request: req,
            sets: planned.sets,
            files: planned.files,
            conflicts,
            frees,
            my_languages: planned.my_languages,
        })
    }

    pub(super) fn cleanup_apply(&self, req: ApplyCleanupRequest) -> ApiResult<JobId> {
        let resolver = self.resolver()?.clone();
        resolver.check_cleanup(&req)?;
        let job = JobId(
            self.next_job
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        resolver.register_cleanup(job);
        let activation = self.activation.clone();
        let files = Arc::clone(&self.files);
        let events = Arc::clone(&self.events);
        let spawned = std::thread::Builder::new()
            .name("fm-cleanup".into())
            .spawn(move || {
                let mover = move |leaving: &[FaceId], kept: &[FaceId], mode: ActivationMode| {
                    move_activation(&activation, leaving, kept, mode)
                };
                let result = resolver.apply_cleanup(job, &req, &mover);
                // The next scan reads the moved files' state from the index again.
                *lock(&files) = None;
                if let Err(error) = result {
                    events.send(&Event::JobFailed { job, error });
                }
            });
        if let Err(e) = spawned {
            return Err(ApiError::new(
                fm_api::ErrorCode::Internal,
                format!("starting the cleanup failed: {e}"),
            ));
        }
        Ok(job)
    }

    pub(super) fn cleanup_cancel(&self, job: JobId) -> ApiResult<()> {
        self.resolver()?.cancel_cleanup(job)
    }

    pub(super) fn cleanup_sessions(&self) -> ApiResult<Vec<CleanupSession>> {
        self.resolver()?.list_sessions()
    }

    pub(super) fn cleanup_undo(&self, req: UndoCleanupRequest) -> ApiResult<CleanupSession> {
        let session = self.resolver()?.undo_session(&req)?;
        *lock(&self.files) = None;
        Ok(session)
    }
}
