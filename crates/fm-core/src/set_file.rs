//! Set files (E28.2): a project set saved as its [`SetExport`] JSON, so a
//! teammate on another machine imports the same fonts by file hash.
//!
//! Only the file the user picked is written, never a temporary file or a
//! copy next to it. The extension `.fmset.json` is added when the picked
//! name has none.

use std::path::{Path, PathBuf};

use fm_api::organization::{SET_FILE_EXTENSION, SetExport};

use crate::error::{FmError, FmResult};

/// The largest set file read, in bytes. A set of 10,000 fonts is about
/// 1.5 MB; anything far larger is not a set file.
const MAX_SET_FILE: u64 = 16 * 1024 * 1024;

/// `path`, with `.fmset.json` added when its file name has no extension.
pub fn file_path(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        return path.to_path_buf();
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(SET_FILE_EXTENSION);
    PathBuf::from(name)
}

/// Writes `export` as pretty JSON to [`file_path`] of `path`, replacing a
/// file already there; returns the path written.
pub fn save(path: &Path, export: &SetExport) -> FmResult<PathBuf> {
    let path = file_path(path);
    let mut json = serde_json::to_string_pretty(export)
        .map_err(|e| FmError::internal(format!("set export JSON: {e}")))?;
    json.push('\n');
    std::fs::write(&path, json)
        .map_err(|e| FmError::io("The set could not be saved there.", &path, &e))?;
    Ok(path)
}

/// Reads the set file at `path`. A missing file is `NotFound`, one that is
/// too large or not a set export is `InvalidRequest`.
pub fn load(path: &Path) -> FmResult<SetExport> {
    let not_a_set = || FmError::invalid("This file is not a Font Manager set file.");
    let len = std::fs::metadata(path)
        .map_err(|e| FmError::io("The set file could not be opened.", path, &e))?
        .len();
    if len > MAX_SET_FILE {
        return Err(not_a_set().with_detail(format!("{}: {len} bytes", path.display())));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| FmError::io("The set file could not be opened.", path, &e))?;
    let text = std::str::from_utf8(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes))
        .map_err(|e| not_a_set().with_detail(format!("{}: {e}", path.display())))?;
    serde_json::from_str(text)
        .map_err(|e| not_a_set().with_detail(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests;
