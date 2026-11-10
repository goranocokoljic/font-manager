//! The background trait pass (E49.4): runs the glyph trait detectors
//! ([`crate::glyphs::traits`]) over every face the index lists as needing
//! them ([`Index::trait_pass_todo`]) and writes the results.
//!
//! - **After the scan, never in it.** The scan writes the faces and is done;
//!   the pass starts after it, so the first-scan budget is untouched.
//! - **Batches.** The pass lists [`BATCH`] faces, reads and measures them
//!   off the index lock on half the CPU cores (the UI keeps the rest), then
//!   writes them in one short transaction. Progress is reported per batch.
//! - **Cancellable.** The flag is checked between faces; a cancelled pass
//!   writes what it finished and stops. A new scan cancels the pass and
//!   starts it again when it ends.
//! - **Resumes.** Every measured face is stamped in the index with its
//!   detector versions and file hash, so a pass stopped by a cancel, an exit
//!   or a crash continues with the faces it had not measured.
//! - A file that cannot be read is skipped and tried again by the next
//!   pass; a face whose data cannot be parsed is stamped without traits.

use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use fm_api::{Event, JobId};

use crate::error::FmResult;
use crate::glyphs::traits;
use crate::index::{Index, MeasuredFace, TraitTodo};

/// The faces one batch lists, measures and writes.
pub const BATCH: u32 = 64;

/// What one pass did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassReport {
    /// Faces measured and written.
    pub measured: u32,
    /// Faces that needed traits when the pass started.
    pub total: u32,
    /// Whether the pass stopped on the cancel flag before it was done.
    pub cancelled: bool,
}

/// The threads that measure: half the cores, at least one.
fn workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, NonZero::get)
        .div_ceil(2)
}

/// Runs the pass over `index` until every face is measured or `cancel` is
/// set. `progress` gets `(done, total)` at the start and after each batch.
pub fn run(
    index: &Mutex<Index>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32, u32),
) -> FmResult<PassReport> {
    let versions: Vec<(&str, u32)> = traits::versions().collect();
    run_at(index, cancel, &versions, progress)
}

/// [`run`] with the detector `versions` given, in detector order: a test
/// bumps one to see that only its trait is measured again.
pub(crate) fn run_at(
    index: &Mutex<Index>,
    cancel: &AtomicBool,
    versions: &[(&str, u32)],
    progress: &mut dyn FnMut(u32, u32),
) -> FmResult<PassReport> {
    let lock = || index.lock().unwrap_or_else(PoisonError::into_inner);
    let total = lock().trait_pass_remaining(versions)?;
    let mut report = PassReport {
        measured: 0,
        total,
        cancelled: false,
    };
    progress(0, total);
    let mut done = 0;
    let mut after = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let todo = lock().trait_pass_todo(versions, after, BATCH)?;
        let Some(last) = todo.last() else {
            break;
        };
        after = i64::from(last.face.0);
        let measured = measure_all(&todo, versions, cancel);
        report.measured += lock().write_glyph_traits(&measured)?;
        done += u32::try_from(todo.len()).unwrap_or(u32::MAX);
        progress(done.min(total), total);
    }
    Ok(report)
}

/// Measures `todo` on [`workers`] threads; a face whose file cannot be read,
/// or that the cancel flag stopped before, is left out.
fn measure_all(
    todo: &[TraitTodo],
    versions: &[(&str, u32)],
    cancel: &AtomicBool,
) -> Vec<MeasuredFace> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(todo.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers().min(todo.len()) {
            scope.spawn(|| {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = todo.get(i) else {
                        break;
                    };
                    if let Some(m) = measure_one(item, versions) {
                        out.lock().unwrap_or_else(PoisonError::into_inner).push(m);
                    }
                }
            });
        }
    });
    let mut out = out.into_inner().unwrap_or_else(PoisonError::into_inner);
    out.sort_by_key(|m| m.face.0);
    out
}

/// One face: its file read and the traits it needs measured. `None` when the
/// file cannot be read now.
fn measure_one(item: &TraitTodo, versions: &[(&str, u32)]) -> Option<MeasuredFace> {
    let data = match std::fs::read(&item.path) {
        Ok(data) => data,
        Err(e) => {
            tracing::debug!(path = %item.path, error = %e, "trait pass: file not read");
            return None;
        }
    };
    let wanted = |id: &str| item.traits.iter().any(|t| t == id);
    let found = traits::detect_only(&data, item.collection_index, &wanted);
    let variable = found.as_ref().is_some_and(|f| f.variable);
    let mut detections: Vec<Option<traits::Detection>> =
        found.map(|f| f.traits).unwrap_or_default();
    detections.resize(versions.len(), None);
    let traits = versions
        .iter()
        .zip(detections)
        .filter(|((id, _), _)| wanted(id))
        .map(|((id, version), d)| ((*id).to_owned(), *version, d))
        .collect();
    Some(MeasuredFace {
        face: item.face,
        file_hash: item.file_hash.clone(),
        variable,
        traits,
    })
}

/// The pass on its own thread (`fm-traits`): at most one at a time.
#[derive(Debug, Default)]
pub struct Runner {
    running: Mutex<Option<Running>>,
}

#[derive(Debug)]
struct Running {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Runner {
    /// Starts a pass over `index` as `job`, after the one running (if any)
    /// has stopped: it is cancelled first. `emit` gets
    /// [`Event::TraitPassProgress`] and one [`Event::TraitPassFinished`],
    /// or [`Event::JobFailed`] when the index cannot be read or written.
    pub fn start(
        &self,
        index: Arc<Mutex<Index>>,
        job: JobId,
        emit: impl Fn(Event) + Send + 'static,
    ) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(old) = running.take() {
            old.cancel.store(true, Ordering::Relaxed);
            let _ = old.thread.join();
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let spawned = std::thread::Builder::new()
            .name("fm-traits".into())
            .spawn(move || {
                let mut progress = |done, total| {
                    emit(Event::TraitPassProgress { job, done, total });
                };
                match run(&index, &flag, &mut progress) {
                    Ok(report) => {
                        tracing::info!(%job, measured = report.measured, cancelled = report.cancelled, "trait pass ended");
                        emit(Event::TraitPassFinished {
                            job,
                            measured: report.measured,
                            cancelled: report.cancelled,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(%job, error = %e, "trait pass failed");
                        emit(Event::JobFailed {
                            job,
                            error: e.into(),
                        });
                    }
                }
            });
        match spawned {
            Ok(thread) => *running = Some(Running { cancel, thread }),
            Err(e) => tracing::warn!(error = %e, "trait pass not started"),
        }
    }

    /// Asks the running pass to stop after the face it is measuring; does
    /// not wait for it.
    pub fn cancel(&self) {
        if let Some(r) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            r.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Cancels the running pass and waits until it has stopped.
    pub fn stop(&self) {
        let old = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(old) = old {
            old.cancel.store(true, Ordering::Relaxed);
            let _ = old.thread.join();
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
