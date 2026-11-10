//! The typed event stream. Library changes, scan progress and activation
//! changes are pushed to subscribers; the UI subscribes, it does not poll.

use std::sync::mpsc::Receiver;

use serde::{Deserialize, Serialize};

use crate::{ApiError, FaceId, JobId, SourceId};

/// Something changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Event {
    /// A scan started. Progress and the finish follow under the same job.
    #[serde(rename_all = "camelCase")]
    ScanStarted {
        /// The scan.
        job: JobId,
        /// The source it scans, or every source when absent.
        #[serde(default)]
        source: Option<SourceId>,
    },
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
        /// Faces whose file changed and was read again.
        #[serde(default)]
        changed: u32,
        /// Faces removed.
        removed: u32,
        /// Problems found.
        problems: u32,
    },
    /// The background glyph trait pass (E49.4) made progress. It runs after
    /// a scan, under its own job: `done` of the `total` faces that needed
    /// their traits measured when it started.
    #[serde(rename_all = "camelCase")]
    TraitPassProgress {
        /// The pass.
        job: JobId,
        /// Faces measured so far.
        done: u32,
        /// Faces to measure.
        total: u32,
    },
    /// The trait pass ended: every face measured, or `cancelled` (a new scan
    /// started, which runs the pass again when it ends, or the app closed;
    /// the next pass resumes where this one stopped).
    #[serde(rename_all = "camelCase")]
    TraitPassFinished {
        /// The pass.
        job: JobId,
        /// Faces whose traits were measured and stored.
        measured: u32,
        /// Whether the pass stopped before it was done.
        cancelled: bool,
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
    /// An AI assistant (an MCP client) wants to run a tool that needs the
    /// user's Allow (E27.6); the call waits for
    /// `SettingsService::answer_mcp_confirmation` or the timeout.
    #[serde(rename_all = "camelCase")]
    McpConfirmationRequested {
        /// The confirmation, for the answer.
        id: u32,
        /// The tool's MCP name, e.g. `deactivate_fonts`.
        tool: String,
        /// How many faces the call acts on, when it names them.
        #[serde(default)]
        faces: Option<u32>,
        /// How long the call waits, in seconds; no answer is a Deny.
        timeout_secs: u32,
    },
    /// A confirmation was answered or timed out; the UI takes its toast away.
    #[serde(rename_all = "camelCase")]
    McpConfirmationClosed {
        /// The confirmation.
        id: u32,
        /// Whether the call may run.
        allowed: bool,
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

    #[test]
    fn scan_started_names_its_source() {
        let e = Event::ScanStarted {
            job: JobId(4),
            source: Some(SourceId(2)),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"type":"scanStarted","job":4,"source":2}"#);
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
    }

    #[test]
    fn scan_finished_reports_changed_faces() {
        let e = Event::ScanFinished {
            job: JobId(5),
            added: 7,
            changed: 3,
            removed: 2,
            problems: 1,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"type":"scanFinished","job":5,"added":7,"changed":3,"removed":2,"problems":1}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
    }

    #[test]
    fn scan_finished_without_changed_still_reads() {
        // Events written before `changed` existed still deserialize.
        let old = r#"{"type":"scanFinished","job":5,"added":7,"removed":2,"problems":1}"#;
        let e: Event = serde_json::from_str(old).unwrap();
        assert!(matches!(
            e,
            Event::ScanFinished {
                changed: 0,
                added: 7,
                ..
            }
        ));
    }

    #[test]
    fn trait_pass_events_are_camel_case() {
        let e = Event::TraitPassProgress {
            job: JobId(6),
            done: 64,
            total: 100,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"type":"traitPassProgress","job":6,"done":64,"total":100}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
        let end = Event::TraitPassFinished {
            job: JobId(6),
            measured: 64,
            cancelled: true,
        };
        let json = serde_json::to_string(&end).unwrap();
        assert_eq!(
            json,
            r#"{"type":"traitPassFinished","job":6,"measured":64,"cancelled":true}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), end);
    }

    #[test]
    fn mcp_confirmations_are_camel_case() {
        let e = Event::McpConfirmationRequested {
            id: 2,
            tool: "deactivate_fonts".into(),
            faces: Some(3),
            timeout_secs: 60,
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"type":"mcpConfirmationRequested","id":2,"tool":"deactivate_fonts","faces":3,"timeoutSecs":60}"#
        );
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), e);
        let closed = Event::McpConfirmationClosed {
            id: 2,
            allowed: false,
        };
        assert_eq!(
            serde_json::to_string(&closed).unwrap(),
            r#"{"type":"mcpConfirmationClosed","id":2,"allowed":false}"#
        );
    }
}
