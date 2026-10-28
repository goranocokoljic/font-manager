//! What a font drag to another app carries (E34.1): the face's names and its
//! file as a `file://` URL, for a design app's "Font Switch" drop.
//!
//! The URL is built from the path's bytes, without `cfg`: a path that starts
//! with `/` is a POSIX path; any other is a Windows path (a drive, a UNC share
//! or either behind the `\\?\` prefix), whose `\` separators become `/`.

use std::path::Path;

use fm_api::library::{FaceSummary, FontSwitchPayload};

/// The drag payload of `face`, whose file is at `path`.
pub fn payload(face: &FaceSummary, path: &Path) -> FontSwitchPayload {
    FontSwitchPayload {
        postscript_name: face.postscript_name.clone(),
        family: face.family_name.clone(),
        style: face.style.clone(),
        file_url: file_url(path),
    }
}

/// `path` as a `file://` URL (RFC 8089): `/a b` is `file:///a%20b`,
/// `C:\a b` is `file:///C:/a%20b`, `\\server\share\a` is
/// `file://server/share/a`. Every byte but the unreserved ones, `/` and `:`
/// is percent-encoded, so non-ASCII and non-UTF-8 names survive.
pub fn file_url(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.first() == Some(&b'/') {
        return format!("file://{}", encode(bytes));
    }
    let rest = if let Some(unc) = strip(bytes, br"\\?\UNC\") {
        return unc_url(unc);
    } else if let Some(local) = strip(bytes, br"\\?\") {
        local
    } else if let Some(unc) = strip(bytes, br"\\") {
        return unc_url(unc);
    } else {
        bytes
    };
    format!("file:///{}", encode(&slashes(rest)))
}

/// `server\share\a` as `file://server/share/a`.
fn unc_url(unc: &[u8]) -> String {
    let unc = slashes(unc);
    let (host, path) = match unc.iter().position(|&b| b == b'/') {
        Some(i) => unc.split_at(i),
        None => (&unc[..], &[][..]),
    };
    format!("file://{}{}", encode(host), encode(path))
}

fn strip<'a>(bytes: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    bytes.strip_prefix(prefix)
}

/// A Windows path with `/` for its separators.
fn slashes(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| if b == b'\\' { b'/' } else { b })
        .collect()
}

fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
