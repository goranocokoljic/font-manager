//! The last check of the release server (E55.10): when it ran and what it
//! found, manual or automatic, kept in `<data>/update-last-check.json` so
//! Settings → About shows it after a restart too. The file is the app's own,
//! not a user file (rule 2).

use std::fs;
use std::path::PathBuf;

use fm_api::update::{LastUpdateCheck, UpdateCheckOutcome};
use fm_platform::AppPaths;

use crate::{FmError, FmResult};

const FILE: &str = "update-last-check.json";

/// The record of the last update check under the app folders.
#[derive(Debug, Clone)]
pub struct LastCheck {
    path: PathBuf,
}

impl LastCheck {
    /// The record under the app folders `paths`.
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.data.join(FILE),
        }
    }

    /// Records that a check ran now and found `outcome`; it replaces the one
    /// before. Returns what was recorded.
    pub fn record(&self, outcome: UpdateCheckOutcome) -> FmResult<LastUpdateCheck> {
        self.record_at(crate::catalog::now_rfc3339(), outcome)
    }

    /// Records a check that ran `at` (RFC 3339) and found `outcome`.
    pub fn record_at(&self, at: String, outcome: UpdateCheckOutcome) -> FmResult<LastUpdateCheck> {
        let failed = "The time of the last update check could not be saved.";
        let check = LastUpdateCheck { at, outcome };
        let json = serde_json::to_vec(&check).map_err(|e| FmError::internal(e.to_string()))?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| FmError::io(failed, dir, &e))?;
        }
        super::on_quit::write_atomic(&self.path, &json, failed)?;
        Ok(check)
    }

    /// The last check; `None` when the app never checked or the record is
    /// unreadable.
    pub fn read(&self) -> Option<LastUpdateCheck> {
        let read = fs::read(&self.path).ok()?;
        match serde_json::from_slice(&read) {
            Ok(check) => Some(check),
            Err(e) => {
                tracing::warn!(error = %e, "the record of the last update check is unreadable");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Home {
        root: PathBuf,
        last: LastCheck,
    }

    impl Home {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("fm-last-check-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let last = LastCheck::new(&AppPaths::under(&root));
            Self { root, last }
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn the_last_check_is_kept_across_starts_and_replaced_by_the_next() {
        let home = Home::new("kept");
        assert_eq!(home.last.read(), None, "never checked");
        let found = home
            .last
            .record_at(
                "2026-11-19T08:00:00Z".into(),
                UpdateCheckOutcome::Found {
                    version: "1.5.0".into(),
                },
            )
            .expect("recorded");
        assert!(home.last.path.starts_with(&home.root));
        // A new start reads the same file.
        let again = LastCheck::new(&AppPaths::under(&home.root));
        assert_eq!(again.read(), Some(found));
        let failed = again
            .record(UpdateCheckOutcome::Failed {
                message: "offline".into(),
            })
            .expect("recorded");
        assert!(failed.at.ends_with('Z'), "{}", failed.at);
        assert_eq!(home.last.read(), Some(failed));
    }

    #[test]
    fn an_unreadable_record_reads_as_never_checked() {
        let home = Home::new("broken");
        let dir = home.last.path.parent().expect("parent").to_path_buf();
        fs::create_dir_all(&dir).expect("dir");
        fs::write(&home.last.path, b"{ not json").expect("write");
        assert_eq!(home.last.read(), None);
        // The next check writes over it.
        home.last
            .record(UpdateCheckOutcome::UpToDate)
            .expect("recorded");
        assert_eq!(
            home.last.read().map(|c| c.outcome),
            Some(UpdateCheckOutcome::UpToDate)
        );
    }
}
