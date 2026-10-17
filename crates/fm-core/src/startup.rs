//! What every entry point (the app, the CLI) does first: find the app
//! folders, read the settings, start logging and open the index (which runs
//! its migrations).

use std::fs;

use fm_api::{ApiError, ErrorCode};
use fm_platform::AppPaths;

use crate::config::Config;
use crate::index::{INDEX_FILE, Index, Opened};
use crate::logging::{self, LogGuard};

/// The app folders, the settings and the log guard. Keep it alive until exit.
#[derive(Debug)]
pub struct Startup {
    /// The per-user app folders on this OS.
    pub paths: AppPaths,
    /// The settings read from `config.json`.
    pub config: Config,
    /// Flushes the log file when dropped.
    pub log: LogGuard,
    /// The library index, migrated to the current schema; when
    /// [`Opened::rebuilt`] is set, the caller scans [`Rebuilt::sources`](crate::index::Rebuilt::sources)
    /// to fill it again. An index that cannot be opened (one made by a newer
    /// app, a locked file) is an error here, not a failed start: the paths,
    /// the settings and the log stay usable without it.
    pub index: Result<Opened, ApiError>,
}

/// Resolves the folders for the current user, reads the settings and makes
/// the rotating log file the process-wide logger.
pub fn init() -> Result<Startup, ApiError> {
    let paths = AppPaths::current().map_err(|e| {
        ApiError::new(ErrorCode::Io, "The app folders could not be found.")
            .with_detail(e.to_string())
    })?;
    let config = prepare(&paths)?;
    let log = logging::init(&paths.logs, config.log_level)?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %paths.config_file().display(),
        data = %paths.data.display(),
        cache = %paths.cache.display(),
        logs = %paths.logs.display(),
        "font manager starting"
    );
    let index = open_index(&paths);
    match &index {
        Ok(Opened {
            rebuilt: Some(r), ..
        }) => tracing::warn!(
            reason = %r.reason,
            sources = r.sources.len(),
            "library index was damaged and is rebuilt from the sources"
        ),
        Ok(_) => {}
        Err(e) => tracing::error!(error = %e, "library index could not be opened"),
    }
    Ok(Startup {
        paths,
        config,
        log,
        index,
    })
}

/// Opens the index in the app-data folder and runs its migrations. A damaged
/// file is replaced, not reported (see [`Index::open`]).
pub fn open_index(paths: &AppPaths) -> Result<Opened, ApiError> {
    Ok(Index::open(&paths.data.join(INDEX_FILE))?)
}

/// Creates the app-owned folders (never the library folder, which the user
/// confirms first) and reads or creates the settings file.
pub fn prepare(paths: &AppPaths) -> Result<Config, ApiError> {
    for dir in [&paths.config, &paths.data, &paths.cache, &paths.logs] {
        fs::create_dir_all(dir).map_err(|e| {
            ApiError::new(ErrorCode::Io, "An app folder could not be created.")
                .with_detail(format!("{}: {e}", dir.display()))
        })?;
    }
    Config::load_or_create(&paths.config_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn paths_in(dir: &TempDir) -> AppPaths {
        let root = dir.path();
        AppPaths {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            logs: root.join("logs"),
            library: root.join("Fonts"),
        }
    }

    #[test]
    fn prepare_creates_app_folders_and_the_config_but_not_the_library() {
        let dir = TempDir::new("startup-prepare");
        let paths = paths_in(&dir);
        let config = prepare(&paths).expect("prepared");
        assert_eq!(config, Config::default());
        assert!(paths.config_file().is_file());
        for d in [&paths.data, &paths.cache, &paths.logs] {
            assert!(d.is_dir(), "{}", d.display());
        }
        assert!(
            !paths.library.exists(),
            "the library folder is the user's to create"
        );
    }

    #[test]
    fn open_index_creates_and_migrates_the_database_in_the_data_folder() {
        let dir = TempDir::new("startup-index");
        let paths = paths_in(&dir);
        prepare(&paths).expect("prepared");
        let opened = open_index(&paths).expect("opened");
        assert!(paths.data.join(INDEX_FILE).is_file());
        assert!(opened.rebuilt.is_none());
        assert_eq!(
            opened.index.schema_version().expect("version"),
            crate::index::SCHEMA_VERSION
        );
    }

    #[test]
    fn open_index_rebuilds_a_damaged_file_instead_of_failing() {
        let dir = TempDir::new("startup-index-damaged");
        let paths = paths_in(&dir);
        prepare(&paths).expect("prepared");
        fs::write(paths.data.join(INDEX_FILE), b"not a database").expect("damage");
        let opened = open_index(&paths).expect("rebuilt, not an error");
        assert!(opened.rebuilt.is_some());
    }

    #[test]
    fn open_index_returns_a_newer_index_as_an_error_value() {
        let dir = TempDir::new("startup-index-newer");
        let paths = paths_in(&dir);
        prepare(&paths).expect("prepared");
        let opened = open_index(&paths).expect("opened");
        drop(opened);
        rusqlite::Connection::open(paths.data.join(INDEX_FILE))
            .and_then(|c| c.pragma_update(None, "user_version", crate::index::SCHEMA_VERSION + 1))
            .expect("bump");
        // `Startup::index` holds this value; `init` does not fail with it.
        let err = open_index(&paths).expect_err("newer index");
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert!(paths.config_file().is_file(), "the settings stay usable");
    }

    #[test]
    fn prepare_reports_a_blocked_folder() {
        let dir = TempDir::new("startup-blocked");
        let paths = paths_in(&dir);
        fs::write(&paths.cache, "").expect("seed");
        assert_eq!(prepare(&paths).expect_err("blocked").code, ErrorCode::Io);
    }
}
