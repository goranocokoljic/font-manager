//! Declared licenses (E15.2): checking what the user declares, and the copy
//! of a receipt the app keeps.
//!
//! A receipt is copied, never moved: the file the user picked stays where it
//! is (rule 2). The copy goes into a `receipts` folder, either in the font's
//! source folder (`.fontlibrary/receipts/`, synced with the sidecars) or in
//! the app's data folder. A copy never overwrites a file: a name that is
//! taken gets a number. Replacing or clearing a receipt moves the app's copy
//! to the OS trash (the API does it, through `fm_platform::Trash`); nothing
//! here deletes a file.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{FmError, FmResult};
use crate::sidecar::SIDECAR_DIR;

/// The folder a receipt copy goes into, inside `.fontlibrary` or the app's
/// data folder.
pub const RECEIPTS_DIR: &str = "receipts";

/// How many numbered names a copy tries before it gives up.
const MAX_NAMES: u32 = 1000;

/// Checks a purchase date: `YYYY-MM-DD`, a day that exists. Anything else is
/// `InvalidRequest`.
pub fn check_date(date: &str) -> FmResult<()> {
    if is_date(date) {
        Ok(())
    } else {
        Err(FmError::invalid(format!(
            "\"{date}\" is not a date. Write the purchase date as YYYY-MM-DD, for example 2026-03-01."
        )))
    }
}

fn is_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if !date.is_ascii() || bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let num = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &date[range];
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// Checks the seats the user declared: 0 or more.
pub fn check_seats(seats: i32) -> FmResult<u32> {
    u32::try_from(seats).map_err(|_| FmError::invalid("The number of seats cannot be below 0."))
}

/// Checks that the receipt the user picked is a file that is there.
pub fn check_receipt(path: &Path) -> FmResult<()> {
    if path.as_os_str().is_empty() || !path.is_file() {
        return Err(FmError::invalid(format!(
            "The receipt file \"{}\" is not there. Pick the file again.",
            path.display()
        )));
    }
    Ok(())
}

/// Copies the receipt at `source` into `dir` (created when missing, its
/// parent must exist) under its own file name, numbered when the name is
/// taken. Returns the copy's path. `source` is only read.
pub fn copy_into(source: &Path, dir: &Path) -> FmResult<PathBuf> {
    let not_copied = "The receipt could not be copied.";
    fs::create_dir_all(dir).map_err(|e| FmError::io(not_copied, dir, &e))?;
    let name = source
        .file_name()
        .map(Path::new)
        .ok_or_else(|| FmError::invalid("The receipt must be a file."))?;
    let stem = name.file_stem().unwrap_or(name.as_os_str());
    let ext = name.extension();
    let mut input = File::open(source).map_err(|e| FmError::io(not_copied, source, &e))?;
    for n in 1..=MAX_NAMES {
        let mut file_name = stem.to_owned();
        if n > 1 {
            file_name.push(format!("-{n}"));
        }
        if let Some(ext) = ext {
            file_name.push(".");
            file_name.push(ext);
        }
        let target = dir.join(file_name);
        // `create_new` never opens a file that is there: nothing is
        // overwritten.
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut output) => {
                return io::copy(&mut input, &mut output)
                    .map(|_| target.clone())
                    .map_err(|e| FmError::io(not_copied, &target, &e));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(FmError::io(not_copied, &target, &e)),
        }
    }
    Err(FmError::invalid(format!(
        "The folder \"{}\" already holds too many receipts named like this one.",
        dir.display()
    )))
}

/// Whether `receipt` is a copy the app made: a file directly in
/// `app_receipts` or in a `.fontlibrary/receipts` folder. Only such a copy is
/// ever moved to the trash; a path another machine wrote that points
/// anywhere else is only forgotten.
pub fn is_app_copy(receipt: &Path, app_receipts: Option<&Path>) -> bool {
    if !receipt.is_absolute() {
        return false;
    }
    let Some(dir) = receipt.parent() else {
        return false;
    };
    if app_receipts.is_some_and(|a| a == dir) {
        return true;
    }
    dir.file_name().is_some_and(|n| n == RECEIPTS_DIR)
        && dir
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == SIDECAR_DIR)
}

#[cfg(test)]
mod tests;
