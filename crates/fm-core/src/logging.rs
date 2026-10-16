//! Logging with `tracing` into a rotating file in the platform logs folder
//! ([`fm_platform::AppPaths::logs`]).
//!
//! A new file starts every day (`fm.YYYY-MM-DD.log`) and only the newest
//! [`KEEP_FILES`] are kept; older ones are removed when the logger starts and
//! at each rollover. Writes go through a background thread, so logging never
//! blocks a scan or the UI. Keep the returned [`LogGuard`] alive until exit:
//! dropping it flushes what is buffered.

use std::fs;
use std::path::Path;

use fm_api::{ApiError, ErrorCode};
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

use crate::config::LogLevel;

/// Log file names start with this.
pub const FILE_PREFIX: &str = "fm";
/// Log file names end with this.
pub const FILE_SUFFIX: &str = "log";
/// How many daily log files are kept.
pub const KEEP_FILES: usize = 7;

/// Flushes the log file when dropped. Hold it for the life of the process.
#[derive(Debug)]
#[must_use = "dropping the guard stops writing to the log file"]
pub struct LogGuard(#[allow(dead_code)] WorkerGuard);

/// Makes the rotating log file in `dir` the process-wide log destination.
/// Fails when a logger is already installed.
pub fn init(dir: &Path, level: LogLevel) -> Result<LogGuard, ApiError> {
    let (subscriber, guard) = subscriber(dir, level)?;
    tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        ApiError::new(ErrorCode::Conflict, "Logging is already set up.").with_detail(e.to_string())
    })?;
    Ok(guard)
}

/// A subscriber that writes to the rotating log file in `dir`, creating the folder.
pub fn subscriber(
    dir: &Path,
    level: LogLevel,
) -> Result<(impl Subscriber + Send + Sync + 'static, LogGuard), ApiError> {
    let failed = |e: &dyn std::fmt::Display| {
        ApiError::new(ErrorCode::Io, "The log file could not be opened.")
            .with_detail(format!("{}: {e}", dir.display()))
    };
    fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(FILE_PREFIX)
        .filename_suffix(FILE_SUFFIX)
        .max_log_files(KEEP_FILES)
        .build(dir)
        .map_err(|e| failed(&e))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);
    let subscriber = tracing_subscriber::registry()
        .with(layer)
        .with(level_filter(level));
    Ok((subscriber, LogGuard(guard)))
}

fn level_filter(level: LogLevel) -> LevelFilter {
    match level {
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Debug => LevelFilter::DEBUG,
        LogLevel::Trace => LevelFilter::TRACE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::path::PathBuf;

    fn log_files(dir: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .expect("read logs")
            .map(|e| e.expect("entry").path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.starts_with("fm.") && name.ends_with(".log")
            })
            .collect();
        files.sort();
        files
    }

    fn write_logs(dir: &Path, level: LogLevel, f: impl FnOnce()) {
        let (subscriber, guard) = subscriber(dir, level).expect("subscriber");
        tracing::subscriber::with_default(subscriber, f);
        drop(guard);
    }

    #[test]
    fn events_land_in_a_dated_file_in_the_logs_folder() {
        let dir = TempDir::new("log-write");
        let logs = dir.path().join("logs");
        write_logs(&logs, LogLevel::Info, || {
            tracing::info!(faces = 3, "scan finished");
        });
        let files = log_files(&logs);
        assert_eq!(files.len(), 1, "{files:?}");
        let name = files[0].file_name().and_then(|n| n.to_str()).expect("name");
        // fm.YYYY-MM-DD.log
        assert_eq!(name.len(), "fm.2026-01-01.log".len(), "{name}");
        let text = fs::read_to_string(&files[0]).expect("read");
        assert!(
            text.contains("scan finished") && text.contains("faces=3"),
            "{text}"
        );
        assert!(!text.contains('\u{1b}'), "no colour codes in the file");
    }

    #[test]
    fn the_level_from_config_filters_events() {
        let dir = TempDir::new("log-level");
        write_logs(dir.path(), LogLevel::Warn, || {
            tracing::info!("quiet detail");
            tracing::warn!("loud problem");
        });
        let text: String = log_files(dir.path())
            .iter()
            .map(|p| fs::read_to_string(p).expect("read"))
            .collect();
        assert!(text.contains("loud problem"));
        assert!(!text.contains("quiet detail"));
    }

    #[test]
    fn old_files_rotate_out_and_other_files_stay() {
        let dir = TempDir::new("log-rotate");
        for day in 1..=10 {
            fs::write(dir.path().join(format!("fm.2020-01-{day:02}.log")), "old").expect("seed");
        }
        fs::write(dir.path().join("notes.txt"), "not a log").expect("seed");
        write_logs(dir.path(), LogLevel::Info, || tracing::info!("today"));
        assert!(
            log_files(dir.path()).len() <= KEEP_FILES,
            "{:?}",
            log_files(dir.path())
        );
        assert!(dir.path().join("notes.txt").is_file());
    }

    #[test]
    fn a_file_in_place_of_the_folder_is_an_io_error() {
        let dir = TempDir::new("log-blocked");
        let blocker = dir.path().join("logs");
        fs::write(&blocker, "").expect("seed");
        let err = subscriber(&blocker, LogLevel::Info)
            .err()
            .expect("not a folder");
        assert_eq!(err.code, ErrorCode::Io);
    }
}
