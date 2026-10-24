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
    let actual = dacl_text(sd);
    // SAFETY: `sd` was allocated by GetNamedSecurityInfoW and is freed once.
    let _ = unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    Ok(only_user(&actual?, aces_of(&expected_dacl_text()?)))
}

/// The DACL [`write`] sets, as Windows writes it back. Windows writes some
/// SIDs as aliases (the built-in Administrator account, RID 500, is `LA`),
/// so the expected text goes through the same conversion as the file's.
fn expected_dacl_text() -> io::Result<String> {
    let sd = parse_sddl(&only_user_sddl()?)?;
    let text = dacl_text(sd);
    // SAFETY: `sd` was allocated by the conversion in `parse_sddl`; freed once.
    let _ = unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    text
}

/// The DACL of `sd` as SDDL text (`D:<flags>(<ace>)...`).
fn dacl_text(sd: PSECURITY_DESCRIPTOR) -> io::Result<String> {
    let mut text = PWSTR::null();
    // SAFETY: `sd` is a valid descriptor (the caller's); `text` receives a
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
    if !text.is_null() {
        // SAFETY: allocated by the conversion above and freed once.
        let _ = unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    }
    sddl.map_err(io::Error::other)
}

/// The ACE list of a DACL in SDDL text: everything from the first `(`.
fn aces_of(sddl: &str) -> &str {
    sddl.find('(').map_or("", |at| &sddl[at..])
}

/// Whether the DACL in `sddl` (`D:<flags>(<ace>)...`) is protected (`P`)
/// and holds exactly the entries `aces`. Windows adds the `AI`
/// (auto-inherited) flag on its own, so the other flags do not matter.
fn only_user(sddl: &str, aces: &str) -> bool {
    let Some(dacl) = sddl.strip_prefix("D:") else {
        return false;
    };
    let aces_at = dacl.find('(').unwrap_or(dacl.len());
    let (flags, actual) = dacl.split_at(aces_at);
    flags.contains('P') && !aces.is_empty() && actual == aces
}

/// `D:P(A;;FA;;;<user SID>)`: a protected DACL, full access for the user.
fn only_user_sddl() -> io::Result<String> {
    Ok(format!("D:P(A;;FA;;;{})", user_sid()?))
}

/// A LocalAlloc'd security descriptor from SDDL text; the caller frees it.
fn parse_sddl(sddl: &str) -> io::Result<PSECURITY_DESCRIPTOR> {
    let text = wide(OsStr::new(sddl));
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `text` is NUL-terminated and outlives the call; `sd` receives a
    // LocalAlloc'd descriptor the caller frees.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(text.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
    }
    .map_err(io::Error::other)?;
    Ok(sd)
}

/// Replaces the DACL of `path` with [`only_user_sddl`].
fn restrict(path: &Path) -> io::Result<()> {
    let sd = parse_sddl(&only_user_sddl()?)?;
    let result = set_dacl(path, sd);
    // SAFETY: `sd` was allocated by `parse_sddl` and is freed once.
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
    use super::{aces_of, only_user};

    #[test]
    fn only_a_protected_single_user_dacl_is_private() {
        let sid = "S-1-5-21-1-2-3-1001";
        let ours = format!("(A;;FA;;;{sid})");
        assert!(only_user(&format!("D:PAI{ours}"), &ours));
        assert!(only_user(&format!("D:P{ours}"), &ours));
        // Inherited from the folder: not protected.
        assert!(!only_user(&format!("D:AI{ours}"), &ours));
        // A second entry (Administrators) or another user.
        assert!(!only_user(&format!("D:P{ours}(A;;FA;;;BA)"), &ours));
        assert!(!only_user("D:P(A;;FA;;;S-1-5-21-9-9-9-1002)", &ours));
        // Read access only, or no DACL at all.
        assert!(!only_user(&format!("D:P(A;;FR;;;{sid})"), &ours));
        assert!(!only_user("", &ours));
        assert!(!only_user("D:P", ""));
    }

    /// The built-in Administrator of this machine (RID 500, the CI runner's
    /// account) comes back from Windows as the alias `LA`, not as its SID.
    #[test]
    fn an_aliased_sid_matches_through_the_same_conversion() {
        let user = super::user_sid().expect("sid");
        let Some((domain, _rid)) = user.rsplit_once('-') else {
            panic!("not a SID: {user}");
        };
        let admin = format!("{domain}-500");
        let expected = super::parse_sddl(&format!("D:P(A;;FA;;;{admin})"))
            .and_then(|sd| {
                let text = super::dacl_text(sd);
                // SAFETY: allocated by `parse_sddl`; freed once.
                #[allow(unsafe_code)]
                let _ = unsafe {
                    windows::Win32::Foundation::LocalFree(Some(windows::Win32::Foundation::HLOCAL(
                        sd.0,
                    )))
                };
                text
            })
            .expect("round trip");
        assert_eq!(aces_of(&expected), "(A;;FA;;;LA)");
        assert!(only_user("D:PAI(A;;FA;;;LA)", aces_of(&expected)));
    }

    #[test]
    fn the_current_users_dacl_round_trips() {
        let expected = super::expected_dacl_text().expect("expected");
        assert!(only_user(&expected, aces_of(&expected)), "{expected}");
    }
}
