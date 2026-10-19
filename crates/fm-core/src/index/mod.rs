//! The SQLite index (ADR 0003): one database file in the app-data folder.
//!
//! The index is a cache over the folders the user chose. [`Index::open`] runs
//! the migrations, and when the file is damaged it moves the file aside and
//! starts a fresh one instead of failing: the sources come back from the
//! `sources.json` backup next to the database, and the next scan reads the
//! folders again. [`Index::rebuild`] does the same on request, without losing
//! ids, tags, ratings, collections or activation state.
//!
//! Scan results land in `font_file` and `face` through
//! [`Index::persist_files`] and [`Index::apply_rescan`], one transaction per
//! [`BATCH_SIZE`] files, and come back as a scanner `FileIndex` through
//! [`Index::load_files`].
//!
//! [`Index::search`] answers a text query with ranked families, through the
//! FTS5 trigram table that triggers keep in step with the faces and tags.
//!
//! The user's metadata is mirrored to sidecar files in each source and
//! merged back from them on every scan ([`Index::sync_sidecars`], E7.3).
//!
//! [`Index::problems`] lists what is wrong with the library (E9.4), and
//! the resolution log keeps the "Keep this one" actions for undo (E8.3).

mod activation;
mod collections;
mod duplicates;
mod files;
mod languages;
mod problems;
mod resolutions;
mod samples;
mod schema;
mod search;
mod sets;
mod sidecars;
mod sources;
mod tags;

use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fm_api::ErrorCode;
use fm_api::library::Source;
use rusqlite::Connection;

use crate::error::{FmError, FmResult};

pub(crate) use activation::{ActiveRow, FaceRow, platform_mode};
pub use files::{BATCH_SIZE, Persisted};
pub(crate) use resolutions::StoredMove;
pub use schema::SCHEMA_VERSION;
pub use sidecars::SidecarSync;

/// The file name of the index in the app-data folder.
pub const INDEX_FILE: &str = "index.db";

/// Where a damaged index file is moved before a fresh one is created.
const CORRUPT_SUFFIX: &str = "corrupt";

/// How long a write waits for another process (the CLI, the app) to finish.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The open index database.
pub struct Index {
    conn: Connection,
    /// The database file; `None` for an in-memory index.
    path: Option<PathBuf>,
    /// What [`Index::search`] read last, until the database changes.
    search_cache: RefCell<search::SearchCache>,
    /// The damaged index this one replaced when it was opened, listed by
    /// [`Index::problems`] for the rest of the session.
    rebuilt: Option<Rebuilt>,
}

impl fmt::Debug for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Index").field("path", &self.path).finish()
    }
}

/// What [`Index::open`] found.
#[derive(Debug)]
pub struct Opened {
    /// The index, migrated to [`SCHEMA_VERSION`].
    pub index: Index,
    /// Set when the file was damaged and a fresh index replaced it. The caller
    /// scans every source in it.
    pub rebuilt: Option<Rebuilt>,
}

/// A damaged index that was replaced by a fresh one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// What was wrong, for the log.
    pub reason: String,
    /// Where the damaged file was moved, if it could be moved.
    pub moved_to: Option<PathBuf>,
    /// The sources restored from the backup; scan them to fill the index.
    pub sources: Vec<Source>,
}

impl Index {
    /// Opens (or creates) the index at `path` and runs the migrations.
    ///
    /// A damaged file is not an error: it is moved to `index.db.corrupt` and a
    /// fresh index takes its place, with the sources restored from the backup
    /// ([`Opened::rebuilt`]). An index written by a newer Font Manager is an
    /// [`ErrorCode::Unsupported`] error and is left untouched.
    pub fn open(path: &Path) -> FmResult<Opened> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| {
                FmError::io("The library index folder could not be created.", dir, &e)
            })?;
        }
        match Self::open_checked(path) {
            Ok(index) => Ok(Opened {
                index,
                rebuilt: None,
            }),
            Err(Damage::Other(e)) => Err(e),
            Err(Damage::Corrupt(reason)) => {
                tracing::warn!(path = %path.display(), %reason, "library index is damaged, rebuilding");
                let moved_to = move_aside(path);
                let mut index = Self::open_checked(path).map_err(Damage::into_error)?;
                let sources = index.restore_sources()?;
                let rebuilt = Rebuilt {
                    reason,
                    moved_to,
                    sources,
                };
                index.rebuilt = Some(rebuilt.clone());
                Ok(Opened {
                    index,
                    rebuilt: Some(rebuilt),
                })
            }
        }
    }

    /// The database connection, for tests outside this module that seed rows.
    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// A fresh index in memory, for tests and one-off tools.
    pub fn open_in_memory() -> FmResult<Self> {
        let conn = Connection::open_in_memory().map_err(|e| db_error(&e))?;
        let mut index = Self {
            conn,
            path: None,
            search_cache: RefCell::default(),
            rebuilt: None,
        };
        index.configure().map_err(Damage::into_error)?;
        index.migrate().map_err(Damage::into_error)?;
        index.refresh_languages()?;
        Ok(index)
    }

    /// The schema version of the open database.
    pub fn schema_version(&self) -> FmResult<u32> {
        user_version(&self.conn).map_err(|e| db_error(&e))
    }

    /// Makes the next scan read every file of every source again, and returns
    /// the sources to scan.
    ///
    /// Only the read stamps (hash and modification time) are cleared, in one
    /// transaction. Face ids stay, so tags, ratings, collections, sets and
    /// activation state survive; the scan replaces what it reads and removes
    /// the files it no longer finds.
    pub fn rebuild(&mut self) -> FmResult<Vec<Source>> {
        let tx = self.conn.transaction().map_err(|e| db_error(&e))?;
        tx.execute("UPDATE font_file SET hash = NULL, mtime = NULL", [])
            .map_err(|e| db_error(&e))?;
        tx.commit().map_err(|e| db_error(&e))?;
        self.sources()
    }

    fn open_checked(path: &Path) -> Result<Self, Damage> {
        let conn = Connection::open(path).map_err(Damage::from)?;
        let mut index = Self {
            conn,
            path: Some(path.to_path_buf()),
            search_cache: RefCell::default(),
            rebuilt: None,
        };
        index.configure()?;
        index.check_integrity()?;
        index.migrate()?;
        index.refresh_languages().map_err(Damage::Other)?;
        Ok(index)
    }

    fn configure(&self) -> Result<(), Damage> {
        self.conn.busy_timeout(BUSY_TIMEOUT)?;
        // The first statement that reads the file: a file that is not a
        // database fails here with `NotADatabase`.
        self.conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        self.conn.pragma_update(None, "foreign_keys", true)?;
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(())
    }

    /// `quick_check` reads every page without the cost of a full index check.
    fn check_integrity(&self) -> Result<(), Damage> {
        let result: String = self
            .conn
            .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(Damage::Corrupt(format!("quick_check: {result}")))
        }
    }

    fn migrate(&mut self) -> Result<(), Damage> {
        let current = user_version(&self.conn)?;
        if current > SCHEMA_VERSION {
            return Err(Damage::Other(
                FmError::unsupported(
                    "The library index was made by a newer Font Manager. Update the app to open it.",
                )
                .with_detail(format!(
                    "schema version {current}, this build knows {SCHEMA_VERSION}"
                )),
            ));
        }
        for (i, sql) in schema::MIGRATIONS.iter().enumerate().skip(current as usize) {
            let version = i as u32 + 1;
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
            tracing::info!(version, "library index migrated");
        }
        Ok(())
    }

    /// The file next to the database that mirrors the sources.
    fn backup_path(&self) -> Option<PathBuf> {
        self.path
            .as_ref()
            .map(|p| p.with_file_name(sources::BACKUP_FILE))
    }
}

/// Why opening failed: a damaged file (rebuild it) or anything else (report it).
enum Damage {
    Corrupt(String),
    Other(FmError),
}

impl Damage {
    fn into_error(self) -> FmError {
        match self {
            Self::Corrupt(reason) => FmError::new(
                ErrorCode::Internal,
                "The library index could not be opened.",
            )
            .with_detail(reason),
            Self::Other(e) => e,
        }
    }
}

impl From<rusqlite::Error> for Damage {
    fn from(e: rusqlite::Error) -> Self {
        match e.sqlite_error_code() {
            Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt) => {
                Self::Corrupt(e.to_string())
            }
            _ => Self::Other(db_error(&e)),
        }
    }
}

fn user_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Moves the damaged database and its WAL and shared-memory files to
/// `*.corrupt`, replacing an older damaged copy. Returns the new database path.
fn move_aside(path: &Path) -> Option<PathBuf> {
    let mut moved = None;
    for suffix in ["", "-wal", "-shm"] {
        let mut from = path.as_os_str().to_owned();
        from.push(suffix);
        let from = PathBuf::from(from);
        if !from.exists() {
            continue;
        }
        let mut to = from.clone().into_os_string();
        to.push(".");
        to.push(CORRUPT_SUFFIX);
        let to = PathBuf::from(to);
        match fs::rename(&from, &to) {
            Ok(()) if suffix.is_empty() => moved = Some(to),
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(from = %from.display(), error = %e, "damaged index file could not be moved");
            }
        }
    }
    moved
}

/// A database error. The user sees one sentence; SQLite's message goes into
/// the detail.
fn db_error(e: &rusqlite::Error) -> FmError {
    FmError::new(
        ErrorCode::Io,
        "The library index could not be read or written.",
    )
    .with_detail(e.to_string())
}

#[cfg(test)]
mod tests;
