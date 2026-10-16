//! The typed event stream. Library changes, scan progress and activation
//! changes are pushed to subscribers; the UI subscribes, it does not poll.

use std::sync::mpsc::Receiver;

use serde::{Deserialize, Serialize};

use crate::{ApiError, FaceId, JobId, SourceId};

/// Something changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Event {
    /// A scan made progress.
    #[serde(rename_all = "camelCase")]
    ScanProgress {
        /// The scan.
        job: JobId,
        /// Files done.
        done: u32,
        /// Files found so far.
        total: u32,
    },
    /// A scan finished.
    #[serde(rename_all = "camelCase")]
    ScanFinished {
        /// The scan.
        job: JobId,
        /// Faces added.
        added: u32,
        /// Faces removed.
        removed: u32,
        /// Problems found.
        problems: u32,
    },
    /// A job failed or was cancelled.
    #[serde(rename_all = "camelCase")]
    JobFailed {
        /// The job.
        job: JobId,
        /// Why.
        error: ApiError,
    },
    /// Faces were added, removed or changed (scan, watcher, user metadata).
    #[serde(rename_all = "camelCase")]
    LibraryChanged {
        /// The source, when the change came from one.
        source: Option<SourceId>,
        /// Faces added.
        added: Vec<FaceId>,
        /// Faces removed.
        removed: Vec<FaceId>,
        /// Faces whose data changed.
        updated: Vec<FaceId>,
    },
    /// Faces were activated or deactivated.
    #[serde(rename_all = "camelCase")]
    ActivationChanged {
        /// Now active.
        activated: Vec<FaceId>,
        /// Now inactive.
        deactivated: Vec<FaceId>,
    },
}

/// The event stream.
pub trait EventSource {
    /// A new subscription; it sees every event sent after this call. Dropping
    /// the receiver ends the subscription.
    fn subscribe(&self) -> Receiver<Event>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_tagged_by_type() {
        let e = Event::ScanProgress {
            job: JobId(3),
            done: 10,
            total: 40,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"type":"scanProgress","job":3,"done":10,"total":40}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
    }
}
