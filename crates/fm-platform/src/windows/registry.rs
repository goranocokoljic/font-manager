//! A small safe wrapper over the Win32 registry calls the activator needs:
//! open or create a key under `HKEY_CURRENT_USER`, read, write, list and
//! delete `REG_SZ` values. Names and data are `OsString`s, so a value that is
//! not valid UTF-16 (a path with an unpaired surrogate) round-trips exactly.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegEnumValueW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::PCWSTR;

/// An open key under `HKEY_CURRENT_USER`, closed on drop.
#[derive(Debug)]
pub(crate) struct Key(HKEY);

/// `s` as a NUL-terminated UTF-16 string. A NUL inside `s` is refused:
/// Win32 would stop reading there and act on a different name or value.
fn wide(s: &OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<u16> = s.encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registry name or data contains a NUL character",
        ));
    }
    units.push(0);
    Ok(units)
}

fn check(code: windows::Win32::Foundation::WIN32_ERROR) -> io::Result<()> {
    if code.is_ok() {
        Ok(())
    } else {
        // Win32 error codes are small positive numbers; the cast keeps them.
        Err(io::Error::from_raw_os_error(code.0 as i32))
    }
}

impl Key {
    /// Opens `subkey` of `HKEY_CURRENT_USER` for reading and writing,
    /// creating it (and its parents) when it is not there. Needs no admin rights.
    pub(crate) fn create_current_user(subkey: &str) -> io::Result<Key> {
        let name = wide(OsStr::new(subkey))?;
        let mut key = HKEY::default();
        // SAFETY: `name` is NUL-terminated and outlives the call; `key` is a
        // valid out pointer; no class, no security attributes, no disposition.
        let code = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(name.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                None,
                &mut key,
                None,
            )
        };
        check(code)?;
        Ok(Key(key))
    }

    /// Deletes `subkey` of `HKEY_CURRENT_USER` with everything under it.
    /// A key that is not there is no error. Tests use it for their scratch keys.
    #[cfg(test)]
    pub(crate) fn delete_current_user_tree(subkey: &str) -> io::Result<()> {
        use windows::Win32::System::Registry::RegDeleteTreeW;
        let name = wide(OsStr::new(subkey))?;
        // SAFETY: `name` is NUL-terminated and outlives the call.
        let code = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(name.as_ptr())) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(code)
    }

    /// The string data of value `name`; `None` when there is no such value or
    /// it is not a string.
    pub(crate) fn get_string(&self, name: &OsStr) -> io::Result<Option<OsString>> {
        let name = wide(name)?;
        let mut kind = REG_VALUE_TYPE::default();
        let mut buf: Vec<u16> = vec![0; 260];
        loop {
            let mut bytes = u32::try_from(buf.len() * 2).unwrap_or(u32::MAX);
            // SAFETY: `name` is NUL-terminated; `buf` holds `bytes` writable
            // bytes; `kind` and `bytes` are valid out pointers.
            let code = unsafe {
                RegQueryValueExW(
                    self.0,
                    PCWSTR(name.as_ptr()),
                    None,
                    Some(&mut kind),
                    Some(buf.as_mut_ptr().cast()),
                    Some(&mut bytes),
                )
            };
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            if code == ERROR_MORE_DATA {
                buf.resize((bytes as usize).div_ceil(2) + 1, 0);
                continue;
            }
            check(code)?;
            if kind != REG_SZ && kind != REG_EXPAND_SZ {
                return Ok(None);
            }
            buf.truncate((bytes as usize) / 2);
            return Ok(Some(from_wide_data(&buf)));
        }
    }

    /// Writes value `name` as `REG_SZ` with `data`.
    pub(crate) fn set_string(&self, name: &OsStr, data: &OsStr) -> io::Result<()> {
        let name = wide(name)?;
        let bytes: Vec<u8> = wide(data)?.into_iter().flat_map(u16::to_le_bytes).collect();
        // SAFETY: `name` is NUL-terminated; `bytes` is the NUL-terminated
        // UTF-16 data, as `REG_SZ` expects.
        let code =
            unsafe { RegSetValueExW(self.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&bytes)) };
        check(code)
    }

    /// Deletes value `name`. `false` when there was no such value.
    pub(crate) fn delete_value(&self, name: &OsStr) -> io::Result<bool> {
        let name = wide(name)?;
        // SAFETY: `name` is NUL-terminated and outlives the call.
        let code = unsafe { RegDeleteValueW(self.0, PCWSTR(name.as_ptr())) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        check(code).map(|()| true)
    }

    /// Every string value of the key: name and data. Values of other types
    /// are skipped.
    pub(crate) fn string_values(&self) -> io::Result<Vec<(OsString, OsString)>> {
        let mut out = Vec::new();
        let mut index = 0u32;
        // Value names are at most 16,383 characters.
        let mut name: Vec<u16> = vec![0; 16_384];
        let mut data: Vec<u16> = vec![0; 260];
        loop {
            let mut name_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
            let mut data_bytes = u32::try_from(data.len() * 2).unwrap_or(u32::MAX);
            let mut kind = 0u32;
            // SAFETY: `name` holds `name_len` UTF-16 units and `data` holds
            // `data_bytes` bytes; every out pointer is valid for the call.
            let code = unsafe {
                RegEnumValueW(
                    self.0,
                    index,
                    Some(windows::core::PWSTR(name.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr().cast()),
                    Some(&mut data_bytes),
                )
            };
            if code == ERROR_NO_MORE_ITEMS {
                return Ok(out);
            }
            if code == ERROR_MORE_DATA {
                // Only the data can be too long: the name buffer has the maximum.
                data.resize((data_bytes as usize).div_ceil(2) + 1, 0);
                continue;
            }
            check(code)?;
            if kind == REG_SZ.0 || kind == REG_EXPAND_SZ.0 {
                let value = &data[..(data_bytes as usize) / 2];
                out.push((
                    OsString::from_wide(&name[..name_len as usize]),
                    from_wide_data(value),
                ));
            }
            index += 1;
        }
    }
}

/// String data as stored, without the terminating NUL(s).
fn from_wide_data(units: &[u16]) -> OsString {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    OsString::from_wide(&units[..end])
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: `self.0` was opened by `RegCreateKeyExW` and is closed once.
        // A close error leaves nothing to do.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}
