//! The Recycle Bin.
//!
//! `SHFileOperationW` with the undo flag moves a file to the Recycle Bin of
//! its drive: `<drive>\$Recycle.Bin\<user SID>\$R<id>.<ext>`, next to a
//! `$I<id>.<ext>` record of the old path and the time. The call does not say
//! where the file went, so the record is found afterwards: the newest `$I`
//! file in the user's folder that names the old path.
//!
//! Only fixed drives have a Recycle Bin. On any other drive Windows would
//! delete the file for good, so such a file is refused before anything
//! moves. On a fixed drive a file larger than the bin would be deleted for
//! good too; `FOF_WANTNUKEWARNING` makes Windows ask the user first.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetVolumePathNameW};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
    SHFILEOPSTRUCTW, SHFileOperationW,
};
use windows::core::{PCWSTR, PWSTR};

use crate::trash::{Trash, TrashError, TrashErrorKind, Trashed};

/// `DRIVE_FIXED` from `GetDriveTypeW`: a hard disk or SSD.
const DRIVE_FIXED: u32 = 3;
/// `MAX_PATH`, the fixed path field of a version 1 `$I` record.
const MAX_PATH: usize = 260;

/// The Recycle Bin of the user running the app.
#[derive(Debug, Default)]
pub struct RecycleBin;

impl Trash for RecycleBin {
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError> {
        let path = plain(&std::path::absolute(path).map_err(|e| TrashError::io(&e, path))?);
        fs::symlink_metadata(&path).map_err(|e| TrashError::io(&e, &path))?;
        let root = volume_root(&path).map_err(|e| TrashError::io(&e, &path))?;
        let root_wide = nul_terminated(root.as_os_str().encode_wide());
        // SAFETY: `root_wide` is NUL-terminated and outlives the call.
        let drive = unsafe { GetDriveTypeW(PCWSTR(root_wide.as_ptr())) };
        if drive != DRIVE_FIXED {
            return Err(TrashError::new(
                TrashErrorKind::NoTrash,
                format!("{} is not a fixed drive (type {drive})", root.display()),
            ));
        }
        let bin = root
            .join("$Recycle.Bin")
            .join(user_sid().map_err(|e| TrashError::io(&e, &path))?);
        recycle(&path)?;
        if fs::symlink_metadata(&path).is_ok() {
            return Err(TrashError::new(
                TrashErrorKind::Failed,
                format!("{} is still there after the move", path.display()),
            ));
        }
        let (location, info) = find_record(&bin, &path).ok_or_else(|| {
            TrashError::new(
                TrashErrorKind::Failed,
                format!(
                    "no Recycle Bin record of {} in {}",
                    path.display(),
                    bin.display()
                ),
            )
        })?;
        Ok(Trashed {
            original: path,
            location,
            info: Some(info),
        })
    }
}

/// `path` without a `\\?\` prefix, which the shell does not take:
/// `\\?\C:\x` is `C:\x`, `\\?\UNC\server\share` is `\\server\share`.
fn plain(path: &Path) -> PathBuf {
    let units: Vec<u16> = path.as_os_str().encode_wide().collect();
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let out = if let Some(rest) = units.strip_prefix(unc.as_slice()) {
        let mut out: Vec<u16> = r"\\".encode_utf16().collect();
        out.extend_from_slice(rest);
        out
    } else if let Some(rest) = units.strip_prefix(verbatim.as_slice()) {
        rest.to_vec()
    } else {
        units
    };
    PathBuf::from(OsString::from_wide(&out))
}

/// `units` with a NUL appended.
fn nul_terminated(units: impl Iterator<Item = u16>) -> Vec<u16> {
    units.chain(std::iter::once(0)).collect()
}

/// The root of the volume `path` is on (`C:\`, a mounted folder, a share).
fn volume_root(path: &Path) -> io::Result<PathBuf> {
    let wide = nul_terminated(path.as_os_str().encode_wide());
    let mut buf = vec![0u16; wide.len().max(MAX_PATH) + 1];
    // SAFETY: `wide` is NUL-terminated; `buf` is a writable slice the call
    // fills with a NUL-terminated path no longer than the input.
    unsafe { GetVolumePathNameW(PCWSTR(wide.as_ptr()), &mut buf) }.map_err(io::Error::other)?;
    let len = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(OsString::from_wide(&buf[..len])))
}

/// Moves `path` to the Recycle Bin, without any dialog but the warning that
/// a file is too large for the bin.
fn recycle(path: &Path) -> Result<(), TrashError> {
    // `pFrom` is a list of NUL-terminated paths ended by another NUL.
    let from = nul_terminated(nul_terminated(path.as_os_str().encode_wide()).into_iter());
    let flags =
        FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING;
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        // The flags are documented as a 16-bit field; every one used fits.
        fFlags: u16::try_from(flags.0).unwrap_or(u16::MAX),
        ..Default::default()
    };
    // SAFETY: `op` is fully initialized; `from` is double-NUL-terminated and
    // outlives the call; no window, no name mappings are requested.
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 {
        return Err(TrashError::new(
            TrashErrorKind::Failed,
            format!(
                "SHFileOperationW failed with {code:#x} for {}",
                path.display()
            ),
        ));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err(TrashError::new(
            TrashErrorKind::Denied,
            format!("the move of {} was cancelled", path.display()),
        ));
    }
    Ok(())
}

/// The string SID of the user running the app (`S-1-5-21-…`), the name of
/// their folder in each `$Recycle.Bin`.
fn user_sid() -> io::Result<String> {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo handle of this process needs no closing; `token`
    // is a valid out pointer, closed below.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(io::Error::other)?;
    let result = token_sid(token);
    // SAFETY: `token` was opened above and is closed once.
    let _ = unsafe { CloseHandle(token) };
    result
}

fn token_sid(token: HANDLE) -> io::Result<String> {
    let mut len = 0u32;
    // SAFETY: a size query: no buffer, `len` receives the size. It fails
    // with ERROR_INSUFFICIENT_BUFFER by design.
    let _ = unsafe { GetTokenInformation(token, TokenUser, None, 0, &mut len) };
    // u64 words keep the buffer aligned for TOKEN_USER.
    let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
    // SAFETY: `buf` is writable, aligned and at least `len` bytes long.
    unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            len,
            &mut len,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: the call above wrote a TOKEN_USER at the start of `buf`; its
    // SID points into `buf`, which lives until the end of this function.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = PWSTR::null();
    // SAFETY: `sid` is valid (above); `text` receives a LocalAlloc'd string.
    unsafe { ConvertSidToStringSidW(sid, &mut text) }.map_err(io::Error::other)?;
    // SAFETY: `text` is a NUL-terminated string from the call above.
    let out = unsafe { text.to_string() }.map_err(io::Error::other);
    // SAFETY: `text` was allocated by ConvertSidToStringSidW and is freed once.
    let _ = unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    out
}

/// The `$R` file and `$I` record in `bin` of the newest move of `original`.
fn find_record(bin: &Path, original: &Path) -> Option<(PathBuf, PathBuf)> {
    let wanted = fold(original.as_os_str().encode_wide());
    let mut best: Option<(u64, PathBuf, PathBuf)> = None;
    for entry in fs::read_dir(bin).ok()?.flatten() {
        let name = entry.file_name();
        let units: Vec<u16> = name.encode_wide().collect();
        // `$I` + the id and extension the `$R` file shares.
        let Some(rest) = units.strip_prefix(&[u16::from(b'$'), u16::from(b'I')]) else {
            continue;
        };
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Some((deleted, path)) = parse_record(&bytes) else {
            continue;
        };
        if fold(path.into_iter()) != wanted {
            continue;
        }
        let mut r = vec![u16::from(b'$'), u16::from(b'R')];
        r.extend_from_slice(rest);
        let location = bin.join(OsString::from_wide(&r));
        if best.as_ref().is_none_or(|(t, _, _)| deleted >= *t) && location.exists() {
            best = Some((deleted, location, entry.path()));
        }
    }
    best.map(|(_, location, info)| (location, info))
}

/// Paths compare case-insensitively on Windows (ASCII folding is enough to
/// tell the copies of one group apart; exact case comes back from the record).
fn fold(units: impl Iterator<Item = u16>) -> Vec<u16> {
    units
        .map(|u| match u8::try_from(u) {
            Ok(b) if b.is_ascii_uppercase() => u16::from(b.to_ascii_lowercase()),
            _ => u,
        })
        .collect()
}

/// The deletion time (FILETIME) and old path of a `$I` record: version 1
/// (Vista to 8.1) has a fixed `MAX_PATH` field at offset 24, version 2
/// (Windows 10 and later) a length at 24 and the path from 28.
fn parse_record(bytes: &[u8]) -> Option<(u64, Vec<u16>)> {
    let u64_at = |at: usize| -> Option<u64> {
        Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
    };
    let version = u64_at(0)?;
    let deleted = u64_at(16)?;
    let (start, chars) = match version {
        1 => (24, MAX_PATH),
        2 => {
            let len = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?);
            (28, usize::try_from(len).ok()?)
        }
        _ => return None,
    };
    let raw = bytes.get(start..start + chars.checked_mul(2)?)?;
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0)
        .collect();
    Some((deleted, units))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2(path: &str, deleted: u64) -> Vec<u8> {
        let units: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut out = Vec::new();
        out.extend_from_slice(&2u64.to_le_bytes());
        out.extend_from_slice(&1234u64.to_le_bytes());
        out.extend_from_slice(&deleted.to_le_bytes());
        out.extend_from_slice(&u32::try_from(units.len()).expect("len").to_le_bytes());
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    #[test]
    fn verbatim_prefixes_are_dropped() {
        assert_eq!(
            plain(Path::new(r"\\?\C:\F\a.ttf")),
            Path::new(r"C:\F\a.ttf")
        );
        assert_eq!(
            plain(Path::new(r"\\?\UNC\s\x\a.ttf")),
            Path::new(r"\\s\x\a.ttf")
        );
        assert_eq!(plain(Path::new(r"C:\a.ttf")), Path::new(r"C:\a.ttf"));
    }

    #[test]
    fn a_version_2_record_gives_the_path_and_time() {
        let (deleted, path) = parse_record(&v2(r"C:\Fonts\A.ttf", 99)).expect("parses");
        assert_eq!(deleted, 99);
        assert_eq!(String::from_utf16_lossy(&path), r"C:\Fonts\A.ttf");
    }

    #[test]
    fn a_version_1_record_gives_the_path() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&5u64.to_le_bytes());
        bytes.extend_from_slice(&7u64.to_le_bytes());
        let mut field = vec![0u8; MAX_PATH * 2];
        for (i, u) in r"D:\x.otf".encode_utf16().enumerate() {
            field[i * 2..i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        bytes.extend_from_slice(&field);
        let (deleted, path) = parse_record(&bytes).expect("parses");
        assert_eq!(deleted, 7);
        assert_eq!(String::from_utf16_lossy(&path), r"D:\x.otf");
    }

    #[test]
    fn short_or_unknown_records_are_skipped() {
        assert_eq!(parse_record(&[]), None);
        assert_eq!(parse_record(&v2("C:\\a", 1)[..30]), None);
        let mut v9 = v2("C:\\a", 1);
        v9[0] = 9;
        assert_eq!(parse_record(&v9), None);
    }

    #[test]
    fn the_newest_record_of_the_path_wins() {
        let bin = std::env::temp_dir().join(format!("fm-platform-bin-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bin);
        fs::create_dir_all(&bin).expect("bin");
        for (id, path, time) in [
            ("ABC.ttf", r"C:\Fonts\A.ttf", 10),
            ("DEF.ttf", r"c:\fonts\a.TTF", 20),
            ("GHI.ttf", r"C:\Fonts\B.ttf", 30),
        ] {
            fs::write(bin.join(format!("$I{id}")), v2(path, time)).expect("record");
            fs::write(bin.join(format!("$R{id}")), b"font").expect("file");
        }
        let found = find_record(&bin, Path::new(r"C:\Fonts\A.ttf"));
        let _ = fs::remove_dir_all(&bin);
        let (location, info) = found.expect("found");
        assert_eq!(
            location.file_name().and_then(|n| n.to_str()),
            Some("$RDEF.ttf")
        );
        assert_eq!(info.file_name().and_then(|n| n.to_str()), Some("$IDEF.ttf"));
    }
}
