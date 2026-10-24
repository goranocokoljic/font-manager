//! Files only the user can read (ADR 0015): a protected DACL with one entry,
//! full access for the user's SID. Nothing is inherited from the folder, so
//! neither other users nor the Administrators group are on the list.
//!
//! The file is written under a temporary name whose DACL is set while it is
//! still empty, then renamed over the target; a rename on the same volume
//! keeps the security descriptor, so the content is never readable by others.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, PROTECTED_DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR,
};
use windows::core::{BOOL, PCWSTR, PWSTR};

use super::trash::user_sid;

/// Writes `contents` to `path` so only this user can read it.
pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let tmp = super::super::fs::temp_sibling(path);
    fs::File::create(&tmp)?;
    let written = restrict(&tmp).and_then(|()| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()
    });
    if let Err(e) = written.and_then(|()| fs::rename(&tmp, path)) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Whether the DACL of `path` is exactly the one [`write`] sets.
pub(crate) fn is_private(path: &Path) -> io::Result<bool> {
    let name = wide(path.as_os_str());
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `name` is NUL-terminated and outlives the call; `sd` receives a
    // LocalAlloc'd descriptor, freed below.
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            &mut sd,
        )
    };
    status.ok().map_err(io::Error::other)?;
    let mut text = PWSTR::null();
    // SAFETY: `sd` is the valid descriptor from above; `text` receives a
    // LocalAlloc'd string, freed below.
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            None,
        )
    };
    // SAFETY: `text` (when set) is a NUL-terminated string from the call above.
    let sddl = converted.and_then(|()| unsafe { text.to_string() }.map_err(Into::into));
    // SAFETY: both were allocated by the calls above and are freed once.
    unsafe {
        if !text.is_null() {
            let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        }
        let _ = LocalFree(Some(HLOCAL(sd.0)));
    }
    Ok(only_user(&sddl.map_err(io::Error::other)?, &user_sid()?))
}

/// Whether the DACL in `sddl` (`D:<flags>(<ace>)...`) is protected (`P`)
/// and holds one entry, full access for `sid`. Windows adds the `AI`
/// (auto-inherited) flag on its own, so the other flags do not matter.
fn only_user(sddl: &str, sid: &str) -> bool {
    let Some(dacl) = sddl.strip_prefix("D:") else {
        return false;
    };
    let aces_at = dacl.find('(').unwrap_or(dacl.len());
    let (flags, aces) = dacl.split_at(aces_at);
    flags.contains('P') && aces == format!("(A;;FA;;;{sid})")
}

/// `D:P(A;;FA;;;<user SID>)`: a protected DACL, full access for the user.
fn only_user_sddl() -> io::Result<String> {
    Ok(format!("D:P(A;;FA;;;{})", user_sid()?))
}

/// Replaces the DACL of `path` with [`only_user_sddl`].
fn restrict(path: &Path) -> io::Result<()> {
    let sddl = wide(OsStr::new(&only_user_sddl()?));
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `sddl` is NUL-terminated and outlives the call; `sd` receives a
    // LocalAlloc'd descriptor, freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
    }
    .map_err(io::Error::other)?;
    let result = set_dacl(path, sd);
    // SAFETY: `sd` was allocated by the conversion above and is freed once.
    let _ = unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    result
}

fn set_dacl(path: &Path, sd: PSECURITY_DESCRIPTOR) -> io::Result<()> {
    let mut present = BOOL::default();
    let mut defaulted = BOOL::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    // SAFETY: `sd` is a valid descriptor; the out pointers are valid. `dacl`
    // points into `sd`, which the caller frees only after this returns.
    unsafe { GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted) }
        .map_err(io::Error::other)?;
    let name = wide(path.as_os_str());
    // SAFETY: `name` is NUL-terminated and outlives the call; `dacl` is valid
    // (above). Only the DACL changes, and it no longer inherits.
    let status = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(dacl.cast_const()),
            None,
        )
    };
    status.ok().map_err(io::Error::other)
}

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::only_user;

    #[test]
    fn only_a_protected_single_user_dacl_is_private() {
        let sid = "S-1-5-21-1-2-3-1001";
        assert!(only_user(&format!("D:PAI(A;;FA;;;{sid})"), sid));
        assert!(only_user(&format!("D:P(A;;FA;;;{sid})"), sid));
        // Inherited from the folder: not protected.
        assert!(!only_user(&format!("D:AI(A;;FA;;;{sid})"), sid));
        // A second entry (Administrators) or another user.
        assert!(!only_user(&format!("D:P(A;;FA;;;{sid})(A;;FA;;;BA)"), sid));
        assert!(!only_user("D:P(A;;FA;;;S-1-5-21-9-9-9-1002)", sid));
        // Read access only, or no DACL at all.
        assert!(!only_user(&format!("D:P(A;;FR;;;{sid})"), sid));
        assert!(!only_user("", sid));
    }
}
