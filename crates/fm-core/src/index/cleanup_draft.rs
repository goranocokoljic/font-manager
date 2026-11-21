//! The Duplicates module's draft plan (E56.11): the decisions collected for
//! the next cleanup, kept as one JSON value in `index_meta`, so they survive
//! leaving the module and a restart. No schema change: the draft is one
//! small value read and written whole.

use fm_api::cleanup_draft::CleanupDraft;
use rusqlite::OptionalExtension;

use super::{Index, db_error};
use crate::error::{FmError, FmResult};

/// The `index_meta` key of the draft.
const DRAFT_KEY: &str = "cleanup_draft";

impl Index {
    /// The stored draft; an empty one when none is stored, or when the
    /// stored value cannot be read (a newer version's format): the draft
    /// holds no file, so starting again loses no data.
    pub fn cleanup_draft(&self) -> FmResult<CleanupDraft> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM index_meta WHERE key = ?1",
                [DRAFT_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| db_error(&e))?;
        Ok(value
            .and_then(|v| match serde_json::from_str(&v) {
                Ok(draft) => Some(draft),
                Err(e) => {
                    tracing::warn!(error = %e, "the stored cleanup draft cannot be read; starting again");
                    None
                }
            })
            .unwrap_or_default())
    }

    /// Stores `draft`, replacing the one stored; an empty draft removes it.
    pub fn set_cleanup_draft(&self, draft: &CleanupDraft) -> FmResult<()> {
        if *draft == CleanupDraft::default() {
            self.conn
                .execute("DELETE FROM index_meta WHERE key = ?1", [DRAFT_KEY])
                .map_err(|e| db_error(&e))?;
            return Ok(());
        }
        let value = serde_json::to_string(draft)
            .map_err(|e| FmError::internal(format!("cleanup draft: {e}")))?;
        self.conn
            .execute(
                "INSERT INTO index_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [DRAFT_KEY, value.as_str()],
            )
            .map_err(|e| db_error(&e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use fm_api::cleanup_draft::{CleanupDraft, DraftDecision, SetDecision};
    use fm_api::library::DuplicateLevel;

    use crate::index::Index;

    #[test]
    fn the_draft_is_kept_across_a_reopen_and_an_unreadable_one_reads_as_empty() {
        let dir = crate::test_support::TempDir::new("cleanup-draft");
        let path = dir.path().join(crate::index::INDEX_FILE);
        let draft = CleanupDraft {
            identical: true,
            decisions: vec![DraftDecision {
                set: "name:A".to_owned(),
                level: DuplicateLevel::OtherVersion,
                postscript_name: Some("A".to_owned()),
                decision: SetDecision::NotDuplicates,
                at: "2026-11-21T10:00:00Z".to_owned(),
            }],
        };
        {
            let index = Index::open(&path).expect("open").index;
            assert_eq!(
                index.cleanup_draft().expect("read"),
                CleanupDraft::default()
            );
            index.set_cleanup_draft(&draft).expect("write");
        }
        let index = Index::open(&path).expect("open again").index;
        assert_eq!(index.cleanup_draft().expect("read"), draft);
        index
            .conn
            .execute(
                "UPDATE index_meta SET value = '{\"decisions\":7}' WHERE key = 'cleanup_draft'",
                [],
            )
            .expect("break it");
        assert_eq!(
            index.cleanup_draft().expect("read"),
            CleanupDraft::default()
        );
        index
            .set_cleanup_draft(&CleanupDraft::default())
            .expect("clear");
        let left: u32 = index
            .conn
            .query_row(
                "SELECT COUNT(*) FROM index_meta WHERE key = 'cleanup_draft'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(left, 0, "an empty draft is not stored");
    }
}
