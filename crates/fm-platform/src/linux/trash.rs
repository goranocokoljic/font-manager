//! The freedesktop.org Trash specification (1.0), the trash GNOME, KDE and
//! the other Linux desktops share.
//!
//! - A file on the home volume goes to the home trash,
//!   `$XDG_DATA_HOME/Trash` (default `~/.local/share/Trash`), its info file
//!   naming the absolute old path.
//! - A file on another volume goes to that volume's trash, so it is renamed,
//!   never copied: `$topdir/.Trash/$uid` when the administrator made a
//!   shared `.Trash` (a folder, not a link, with the sticky bit), else
//!   `$topdir/.Trash-$uid`; the info file names the path relative to `$topdir`.
//! - The file goes to `files/<name>` and its record to
//!   `info/<name>.trashinfo`. The record is created first and exclusively,
//!   which reserves the name against another program trashing at once.

use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
#[cfg(test)]
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::trash::{Trash, TrashError, TrashErrorKind, Trashed};

/// The sticky bit (`S_ISVTX`) a shared `$topdir/.Trash` must have.
const STICKY: u32 = 0o1000;
/// How many names `name`, `name.2.ext`, … are tried before giving up.
const MAX_NAMES: u32 = 10_000;

/// The freedesktop.org trash of one user.
#[derive(Debug)]
pub struct FreedesktopTrash {
    /// `$XDG_DATA_HOME/Trash`.
    home_trash: PathBuf,
    /// The user's id, for the per-volume trash folders.
    uid: u32,
}

impl FreedesktopTrash {
    /// The trash of the user running the app: the home trash from
    /// `XDG_DATA_HOME` (a relative value is ignored, as the XDG specification
    /// requires) or `HOME`, and the uid of this process.
    pub(crate) fn from_env() -> Result<Self, TrashError> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|home| home.join(".local").join("share"))
            })
            .ok_or_else(|| {
                TrashError::new(
                    TrashErrorKind::Unsupported,
                    "neither XDG_DATA_HOME nor HOME is set",
                )
            })?;
        // `/proc/self` belongs to the user the process runs as.
        let uid = fs::metadata("/proc/self")
            .map_err(|e| TrashError::io(&e, Path::new("/proc/self")))?
            .uid();
        Ok(Self::new(data.join("Trash"), uid))
    }

    /// A trash whose home trash is `home_trash`, for the user `uid`.
    pub(crate) fn new(home_trash: PathBuf, uid: u32) -> Self {
        Self { home_trash, uid }
    }

    /// The trash folder for a file on device `dev`, and the path its info
    /// file names: absolute in the home trash, relative to the volume's top
    /// folder in a volume trash.
    fn trash_for(&self, path: &Path, dev: u64) -> Result<(PathBuf, PathBuf), TrashError> {
        if device_of_nearest(&self.home_trash) == Some(dev) {
            return Ok((self.home_trash.clone(), path.to_path_buf()));
        }
        let top = top_dir(path, dev);
        let relative = path.strip_prefix(&top).unwrap_or(path).to_path_buf();
        let shared = top.join(".Trash");
        let shared_ok = fs::symlink_metadata(&shared)
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink() && m.mode() & STICKY != 0);
        if shared_ok {
            let mine = shared.join(self.uid.to_string());
            if make_dir(&mine).is_ok() {
                return Ok((mine, relative));
            }
        }
        let mine = top.join(format!(".Trash-{}", self.uid));
        make_dir(&mine).map_err(|e| {
            TrashError::new(TrashErrorKind::NoTrash, format!("{}: {e}", mine.display()))
        })?;
        Ok((mine, relative))
    }
}

impl Trash for FreedesktopTrash {
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError> {
        let path = std::path::absolute(path).map_err(|e| TrashError::io(&e, path))?;
        let meta = fs::symlink_metadata(&path).map_err(|e| TrashError::io(&e, &path))?;
        let (trash, named) = self.trash_for(&path, meta.dev())?;
        let files = trash.join("files");
        let info_dir = trash.join("info");
        for dir in [&files, &info_dir] {
            make_dir(dir).map_err(|e| TrashError::io(&e, dir))?;
        }
        let name = path
            .file_name()
            .ok_or_else(|| TrashError::new(TrashErrorKind::Failed, "the path has no file name"))?;
        let record = trash_info(&named, SystemTime::now());
        for n in 1..=MAX_NAMES {
            let candidate = numbered(Path::new(name), n);
            let location = files.join(&candidate);
            let mut info_name = candidate.clone();
            info_name.push(".trashinfo");
            let info = info_dir.join(info_name);
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&info) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(TrashError::io(&e, &info)),
            };
            if fs::symlink_metadata(&location).is_ok() {
                // A file without a record: leave it, take the next name.
                drop(file);
                let _ = fs::remove_file(&info);
                continue;
            }
            let written = file
                .write_all(record.as_bytes())
                .and_then(|()| file.sync_all());
            drop(file);
            if let Err(e) = written.and_then(|()| fs::rename(&path, &location)) {
                let _ = fs::remove_file(&info);
                return Err(TrashError::io(&e, &path));
            }
            return Ok(Trashed {
                original: path,
                location,
                info: Some(info),
            });
        }
        Err(TrashError::new(
            TrashErrorKind::Failed,
            format!("no free name in {}", files.display()),
        ))
    }
}

/// Creates `dir` and its parents, private to the user (`0700`) as the
/// specification asks.
fn make_dir(dir: &Path) -> std::io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

/// The device of `path`, or of its nearest folder that exists.
fn device_of_nearest(path: &Path) -> Option<u64> {
    path.ancestors()
        .find_map(|p| fs::metadata(p).ok())
        .map(|m| m.dev())
}

/// The top folder of the volume `path` (on device `dev`) is on: the last
/// ancestor still on `dev`.
fn top_dir(path: &Path, dev: u64) -> PathBuf {
    let mut top = path.parent().unwrap_or(path).to_path_buf();
    while let Some(parent) = top.parent() {
        if fs::metadata(parent).map(|m| m.dev()).ok() != Some(dev) {
            break;
        }
        top = parent.to_path_buf();
    }
    top
}

/// `name` for `n = 1`, else `stem.n.ext` (`stem.n` without an extension).
fn numbered(name: &Path, n: u32) -> OsString {
    if n == 1 {
        return name.as_os_str().to_os_string();
    }
    let stem = name.file_stem().unwrap_or(name.as_os_str());
    let mut out = stem.to_os_string();
    out.push(format!(".{n}"));
    if let Some(ext) = name.extension() {
        out.push(".");
        out.push(ext);
    }
    out
}

/// The `.trashinfo` record of a file at `path` deleted at `when`.
fn trash_info(path: &Path, when: SystemTime) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(path.as_os_str()),
        rfc3339_utc(when)
    )
}

/// The path as the specification stores it: URL escapes for every byte but
/// the unreserved ones and `/`, so any file name (not UTF-8 included) fits.
fn percent_encode(path: &OsStr) -> String {
    let mut out = String::new();
    for &b in path.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The inverse of [`percent_encode`], for tests and diagnostics.
#[cfg(test)]
fn percent_decode(s: &str) -> PathBuf {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(Ok(b)) = s.get(i + 1..i + 3).map(|h| u8::from_str_radix(h, 16))
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    PathBuf::from(OsString::from_vec(out))
}

/// `YYYY-MM-DDThh:mm:ss` in UTC. The specification asks for local time
/// without a zone; no crate in the library table knows the local zone, and
/// file managers only show the value.
fn rfc3339_utc(when: SystemTime) -> String {
    let secs = when.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// The proleptic Gregorian date of `days` since 1970-01-01 (Howard
/// Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "fm-platform-xdg-trash-{label}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_on_the_home_volume_goes_to_the_home_trash_with_its_record() {
        let dir = Dir::new("home");
        let home_trash = dir.0.join("data").join("Trash");
        let file = dir.0.join("Schrift ü.ttf");
        fs::write(&file, b"font").expect("write");
        let trash = FreedesktopTrash::new(home_trash.clone(), 1000);

        let item = trash.trash(&file).expect("trash");
        assert_eq!(
            item.location,
            home_trash.join("files").join("Schrift ü.ttf")
        );
        let info = fs::read_to_string(item.info.as_ref().expect("info")).expect("record");
        let path_line = info
            .lines()
            .find_map(|l| l.strip_prefix("Path="))
            .expect("Path=");
        assert_eq!(percent_decode(path_line), file);
        assert!(info.starts_with("[Trash Info]\n"));
        assert!(info.contains("\nDeletionDate="));
        assert!(!file.exists());

        trash.restore(&item).expect("restore");
        assert_eq!(fs::read(&file).expect("back"), b"font");
        assert!(!item.info.as_ref().expect("info").exists());
    }

    #[test]
    fn a_second_file_of_the_same_name_gets_a_numbered_name() {
        let dir = Dir::new("names");
        let trash = FreedesktopTrash::new(dir.0.join("Trash"), 1000);
        let a = dir.0.join("a");
        let b = dir.0.join("b");
        for d in [&a, &b] {
            fs::create_dir_all(d).expect("dir");
            fs::write(d.join("Font.ttf"), d.to_string_lossy().as_bytes()).expect("write");
        }
        let first = trash.trash(&a.join("Font.ttf")).expect("first");
        let second = trash.trash(&b.join("Font.ttf")).expect("second");
        assert_eq!(first.location.file_name(), Some(OsStr::new("Font.ttf")));
        assert_eq!(second.location.file_name(), Some(OsStr::new("Font.2.ttf")));
        trash.restore(&second).expect("restore second");
        trash.restore(&first).expect("restore first");
        assert!(a.join("Font.ttf").exists() && b.join("Font.ttf").exists());
    }

    #[test]
    fn a_non_utf8_name_is_escaped_in_the_record() {
        let path = OsStr::from_bytes(b"/f/\xff a%.ttf");
        assert_eq!(percent_encode(path), "/f/%FF%20a%25.ttf");
        assert_eq!(percent_decode(&percent_encode(path)), Path::new(path));
    }

    #[test]
    fn deletion_dates_are_calendar_dates() {
        assert_eq!(rfc3339_utc(UNIX_EPOCH), "1970-01-01T00:00:00");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400 + 3_723); // 2000-02-29 01:02:03
        assert_eq!(rfc3339_utc(leap), "2000-02-29T01:02:03");
    }

    #[test]
    fn numbered_names_keep_the_extension() {
        assert_eq!(numbered(Path::new("A.otf"), 1), OsString::from("A.otf"));
        assert_eq!(numbered(Path::new("A.otf"), 3), OsString::from("A.3.otf"));
        assert_eq!(numbered(Path::new("README"), 2), OsString::from("README.2"));
    }
}
