//! The crash log (E9.4): a panic hook that writes what happened to a file in
//! the logs folder ([`fm_platform::AppPaths::logs`], ADR 0007) before the
//! process goes down, and the next start that finds it.
//!
//! The log is written synchronously from the hook, so it is on disk even when
//! the panic ends the process before the buffered `tracing` writer flushes.
//! It is a local file only: nothing is sent anywhere (rule 7); the telemetry
//! hook ([`crate::Telemetry::install_crash_hook`]) decides that separately.
//!
//! A start after a crash is a normal start: the index survives a crash (WAL,
//! ADR 0003), so [`take_unreported`] only hands the newest new crash log to
//! the caller once, to log or show, and marks every new one as reported.

use std::backtrace::Backtrace;
use std::fs;
use std::io::{self, Write as _};
use std::panic::{self, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::telemetry::CrashReport;

/// Crash log names start with this.
pub const FILE_PREFIX: &str = "crash-";
/// A crash log nobody has seen yet ends with this.
pub const NEW_SUFFIX: &str = ".log";
/// A crash log that a start has already reported ends with this.
pub const REPORTED_SUFFIX: &str = ".reported.log";
/// How many crash logs are kept; older ones are removed at start.
pub const KEEP_FILES: usize = 10;

/// Installs a panic hook that writes a crash log to `dir`, then runs the
/// previous hook (the default one prints the message to stderr). The hook
/// never panics itself: a log that cannot be written is skipped.
pub fn install(dir: &Path) {
    let dir = dir.to_path_buf();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = write_log(&dir, info);
        previous(info);
    }));
}

/// Writes the crash log for one panic and returns its path.
fn write_log(dir: &Path, info: &PanicHookInfo<'_>) -> io::Result<PathBuf> {
    let report = CrashReport::from_panic(info);
    let thread = std::thread::current();
    let text = format_log(
        &report,
        thread.name().unwrap_or("unnamed"),
        &Backtrace::force_capture().to_string(),
    );
    write_text(dir, &text)
}

/// The text of a crash log.
fn format_log(report: &CrashReport, thread: &str, backtrace: &str) -> String {
    format!(
        "Font Manager {} crashed.\nmessage: {}\nlocation: {}\nthread: {thread}\nos: {} {}\n\nbacktrace:\n{backtrace}\n",
        env!("CARGO_PKG_VERSION"),
        report.message,
        report.location.as_deref().unwrap_or("unknown"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Writes `text` to a new crash log in `dir`, named by time and process id so
/// two crashes never share a file.
fn write_text(dir: &Path, text: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let path = dir.join(format!(
        "{FILE_PREFIX}{millis:016}-{}{NEW_SUFFIX}",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(path)
}

/// Crash logs in `dir` that no start has reported yet: returns the newest
/// one (by name, which starts with the time) and marks every new one as
/// reported. Removes all but the newest [`KEEP_FILES`] crash logs. A missing
/// or unreadable folder is no crash.
pub fn take_unreported(dir: &Path) -> Option<PathBuf> {
    let mut logs: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| is_crash_log(p))
        .collect();
    logs.sort();
    let excess = logs.len().saturating_sub(KEEP_FILES);
    for old in logs.drain(..excess) {
        let _ = fs::remove_file(old);
    }
    let mut newest = None;
    for log in logs.iter().filter(|p| !is_reported(p)) {
        let reported = reported_name(log);
        match fs::rename(log, &reported) {
            Ok(()) => newest = Some(reported),
            Err(e) => {
                tracing::warn!(path = %log.display(), error = %e, "crash log could not be marked as reported");
                newest = Some(log.clone());
            }
        }
    }
    newest
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn is_crash_log(path: &Path) -> bool {
    let name = file_name(path);
    name.starts_with(FILE_PREFIX) && name.ends_with(NEW_SUFFIX)
}

fn is_reported(path: &Path) -> bool {
    file_name(path).ends_with(REPORTED_SUFFIX)
}

fn reported_name(path: &Path) -> PathBuf {
    let name = file_name(path);
    let stem = name.strip_suffix(NEW_SUFFIX).unwrap_or(name);
    path.with_file_name(format!("{stem}{REPORTED_SUFFIX}"))
}

#[cfg(test)]
mod tests;
