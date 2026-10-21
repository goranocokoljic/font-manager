//! Layout template files (E12.5): every layout is one JSON file in the
//! per-user layouts folder ([`fm_platform::AppPaths::layouts_dir`], ADR 0007).
//!
//! - **Plain text.** A template is a [`Layout`] as pretty JSON through
//!   `serde_json`, so a user can edit it by hand. Every call reads the folder
//!   again, so a hand edit is seen the next time layouts are listed. A file
//!   that is not a layout is listed as broken with the reason; it never stops
//!   the other files loading and it is never overwritten unless the user saves
//!   over it (its content is then kept as a version).
//! - **Presets.** The six presets are written on first use and never written
//!   again: `.presets` records which ones were written, so a preset file the
//!   user deleted stays deleted and an edited one keeps its edits.
//!   [`LayoutStore::revert`] with `to_original` brings back the shipped one.
//! - **Versions.** A save that changes the file moves the content it replaces
//!   to `.history/<id>/<number>-<unix ms>.json`, the newest
//!   [`MAX_VERSIONS`] kept. These are app-owned copies in the app's config
//!   folder, not user files, so the oldest are removed, not trashed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use fm_api::layout::{Layout, LayoutEntry, LayoutPreset, LayoutVersion, SavedLayout};
use fm_api::{ApiError, ApiResult, ErrorCode};

use super::{PRESETS, clamp_layout, preset, preset_name};

/// How many earlier versions of one template are kept.
pub const MAX_VERSIONS: usize = 20;
/// The extension of a template file.
const EXT: &str = "json";
/// The folder of kept versions, inside the layouts folder.
const HISTORY_DIR: &str = ".history";
/// The list of presets written so far, inside the layouts folder.
const PRESETS_FILE: &str = ".presets";
/// The longest id a template file gets.
const MAX_ID_CHARS: usize = 64;

/// The template file id of a preset ("book").
pub fn preset_id(p: LayoutPreset) -> String {
    preset_name(p).to_lowercase()
}

/// The preset whose shipped file has this id.
fn preset_of_id(id: &str) -> Option<LayoutPreset> {
    PRESETS.into_iter().find(|p| preset_id(*p) == id)
}

/// Whether `id` can name a template file on every OS: not empty, not hidden,
/// no path separator or character Windows forbids, no trailing dot or space,
/// not a reserved device name.
pub fn is_valid_id(id: &str) -> bool {
    const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    const RESERVED: &[&str] = &["con", "prn", "aux", "nul"];
    let lower = id.to_lowercase();
    let device = RESERVED.contains(&lower.as_str())
        || ((lower.starts_with("com") || lower.starts_with("lpt"))
            && lower.len() == 4
            && lower.as_bytes()[3].is_ascii_digit());
    !id.is_empty()
        && id.chars().count() <= MAX_ID_CHARS
        && !id.starts_with('.')
        && !id.ends_with(['.', ' '])
        && !id.chars().any(|c| c.is_control() || FORBIDDEN.contains(&c))
        && !device
}

/// A file-name id for `name`: lowercase letters and digits (any script), the
/// rest turned into single dashes; "layout" when nothing is left.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out.chars().take(MAX_ID_CHARS - 4).collect();
    while out.ends_with('-') {
        out.pop();
    }
    if is_valid_id(&out) {
        out
    } else if out.is_empty() {
        "layout".to_owned()
    } else {
        format!("{out}-layout")
    }
}

/// What a template file's content is.
enum Content {
    Layout(Layout),
    Broken(String),
}

/// Reads template content; values out of range are clamped.
fn parse(text: &str) -> Content {
    match serde_json::from_str::<Layout>(text) {
        Ok(layout) => Content::Layout(clamp_layout(layout)),
        Err(e) if e.is_data() => Content::Broken(format!("Not a layout: {e}")),
        Err(e) => Content::Broken(format!("Not valid JSON: {e}")),
    }
}

/// Reads a file's bytes as content; bytes that are not UTF-8 are broken too.
fn parse_bytes(bytes: &[u8]) -> Content {
    match std::str::from_utf8(bytes) {
        Ok(text) => parse(text),
        Err(e) => Content::Broken(format!("Not UTF-8 text: {e}")),
    }
}

/// The text a layout is written as.
fn to_text(layout: &Layout) -> ApiResult<String> {
    serde_json::to_string_pretty(layout)
        .map(|json| json + "\n")
        .map_err(|e| {
            ApiError::new(ErrorCode::Internal, "The layout could not be written.")
                .with_detail(e.to_string())
        })
}

fn io_error(message: &str, path: &Path, e: &io::Error) -> ApiError {
    let code = if e.kind() == io::ErrorKind::PermissionDenied {
        ErrorCode::PermissionDenied
    } else {
        ErrorCode::Io
    };
    ApiError::new(code, message).with_detail(format!("{}: {e}", path.display()))
}

fn invalid_id(id: &str) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        "That is not a layout name the app can use.",
    )
    .with_detail(format!("invalid layout id {id:?}"))
}

fn not_found(id: &str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, "That layout no longer exists.")
        .with_detail(format!("no layout file {id:?}"))
}

fn broken(reason: &str) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidRequest,
        "This layout file cannot be read. Fix it by hand or save over it.",
    )
    .with_detail(reason.to_owned())
}

/// Writes `text` through a temporary file, so a crash never leaves half a file.
fn write_atomic(path: &Path, text: &str) -> ApiResult<()> {
    let failed = "The layout could not be saved.";
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(failed, dir, &e))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text).map_err(|e| io_error(failed, &tmp, &e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_error(failed, path, &e)
    })
}

/// Milliseconds since 1970 now.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `ms` since 1970 as UTC `YYYY-MM-DDTHH:MM:SSZ` (days to civil date after
/// Howard Hinnant's algorithm; no date crate needed).
pub fn iso_utc(ms: u64) -> String {
    let secs = ms / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// One kept version on disk.
struct VersionFile {
    number: u32,
    saved_ms: u64,
    path: PathBuf,
}

/// The layout template files of one folder.
#[derive(Debug)]
pub struct LayoutStore {
    dir: PathBuf,
    /// Serializes read-change-write, so two saves never lose a version.
    lock: Mutex<()>,
}

impl LayoutStore {
    /// The store over the layouts folder `dir`; nothing is written until first use.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            lock: Mutex::new(()),
        }
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.{EXT}"))
    }

    fn history_dir(&self, id: &str) -> PathBuf {
        self.dir.join(HISTORY_DIR).join(id)
    }

    /// Writes the presets not written before; a preset file that exists is
    /// never replaced. Called by every operation, under the lock.
    fn seed(&self) -> ApiResult<()> {
        let record = self.dir.join(PRESETS_FILE);
        let mut written: Vec<String> = fs::read_to_string(&record)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let before = written.len();
        for p in PRESETS {
            let id = preset_id(p);
            if written.contains(&id) {
                continue;
            }
            let path = self.file(&id);
            if !path.exists() {
                write_atomic(&path, &to_text(&preset(p))?)?;
            }
            written.push(id);
        }
        if written.len() != before {
            let json = serde_json::to_string(&written).unwrap_or_else(|_| "[]".to_owned());
            write_atomic(&record, &(json + "\n"))?;
        }
        Ok(())
    }

    fn guard(&self) -> ApiResult<std::sync::MutexGuard<'_, ()>> {
        let guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.seed()?;
        Ok(guard)
    }

    /// The raw bytes of a template file, or `None` when there is none.
    fn read(&self, id: &str) -> ApiResult<Option<Vec<u8>>> {
        let path = self.file(id);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error("The layout file could not be read.", &path, &e)),
        }
    }

    fn load(&self, id: &str) -> ApiResult<Layout> {
        if !is_valid_id(id) {
            return Err(invalid_id(id));
        }
        match parse_bytes(&self.read(id)?.ok_or_else(|| not_found(id))?) {
            Content::Layout(layout) => Ok(layout),
            Content::Broken(reason) => Err(broken(&reason)),
        }
    }

    /// Every template file: the presets in picker order, then the others by
    /// name. A file that cannot be read is listed with the reason.
    pub fn list(&self) -> ApiResult<Vec<LayoutEntry>> {
        let _guard = self.guard()?;
        let mut entries = self.scan()?;
        let rank = |e: &LayoutEntry| {
            PRESETS
                .iter()
                .position(|p| preset_id(*p) == e.id)
                .unwrap_or(PRESETS.len())
        };
        entries.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(entries)
    }

    /// The template files in the folder, unsorted; the caller holds the lock.
    fn scan(&self) -> ApiResult<Vec<LayoutEntry>> {
        let read_dir = fs::read_dir(&self.dir)
            .map_err(|e| io_error("The layouts folder could not be read.", &self.dir, &e))?;
        let mut entries = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !is_valid_id(id) {
                continue;
            }
            let content = match fs::read(&path) {
                Ok(bytes) => parse_bytes(&bytes),
                Err(e) => Content::Broken(format!("Cannot be read: {e}")),
            };
            entries.push(match content {
                Content::Layout(layout) => LayoutEntry {
                    id: id.to_owned(),
                    name: layout.name,
                    preset: layout.preset,
                    broken: None,
                },
                Content::Broken(reason) => LayoutEntry {
                    id: id.to_owned(),
                    name: id.to_owned(),
                    preset: preset_of_id(id),
                    broken: Some(reason),
                },
            });
        }
        Ok(entries)
    }

    /// One template file, clamped; a broken one answers `InvalidRequest`.
    pub fn get(&self, id: &str) -> ApiResult<SavedLayout> {
        let _guard = self.guard()?;
        let layout = self.load(id)?;
        Ok(SavedLayout {
            id: id.to_owned(),
            layout,
        })
    }

    /// Writes `layout` (clamped) to `id`; the content it replaces becomes
    /// the newest version. A save that changes nothing keeps no version.
    pub fn save(&self, id: &str, layout: Layout) -> ApiResult<SavedLayout> {
        let _guard = self.guard()?;
        self.write(id, layout)
    }

    fn write(&self, id: &str, layout: Layout) -> ApiResult<SavedLayout> {
        if !is_valid_id(id) {
            return Err(invalid_id(id));
        }
        if layout.name.trim().is_empty() {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "A layout needs a name.",
            ));
        }
        let layout = clamp_layout(layout);
        let text = to_text(&layout)?;
        if let Some(old) = self.read(id)? {
            if old == text.as_bytes() {
                return Ok(SavedLayout {
                    id: id.to_owned(),
                    layout,
                });
            }
            self.keep_version(id, &old)?;
        }
        write_atomic(&self.file(id), &text)?;
        Ok(SavedLayout {
            id: id.to_owned(),
            layout,
        })
    }

    /// The kept versions of `id`, oldest first; names that are not
    /// `<number>-<ms>.json` are ignored.
    fn versions(&self, id: &str) -> ApiResult<Vec<VersionFile>> {
        let dir = self.history_dir(id);
        let read_dir = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_error("The layout history could not be read.", &dir, &e)),
        };
        let mut out: Vec<VersionFile> = read_dir
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                    return None;
                }
                let stem = path.file_stem()?.to_str()?;
                let (number, ms) = stem.split_once('-')?;
                Some(VersionFile {
                    number: number.parse().ok()?,
                    saved_ms: ms.parse().ok()?,
                    path,
                })
            })
            .collect();
        out.sort_by_key(|v| v.number);
        Ok(out)
    }

    /// Keeps `old` as the newest version of `id` and drops the oldest past
    /// [`MAX_VERSIONS`].
    fn keep_version(&self, id: &str, old: &[u8]) -> ApiResult<()> {
        let versions = self.versions(id)?;
        let number = versions.last().map_or(1, |v| v.number.saturating_add(1));
        let path = self
            .history_dir(id)
            .join(format!("{number:06}-{}.{EXT}", now_ms()));
        let failed = "The previous version of the layout could not be kept.";
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| io_error(failed, dir, &e))?;
        }
        fs::write(&path, old).map_err(|e| io_error(failed, &path, &e))?;
        let excess = (versions.len() + 1).saturating_sub(MAX_VERSIONS);
        for v in versions.iter().take(excess) {
            if let Err(e) = fs::remove_file(&v.path) {
                tracing::warn!(path = %v.path.display(), error = %e, "old layout version not removed");
            }
        }
        Ok(())
    }

    /// Copies `id` into a new file named `name` (or "<name> copy", "<name>
    /// copy 2", …). A name another layout has answers `Conflict`.
    pub fn duplicate(&self, id: &str, name: Option<&str>) -> ApiResult<SavedLayout> {
        let _guard = self.guard()?;
        let source = self.load(id)?;
        let taken: Vec<String> = self
            .scan()?
            .into_iter()
            .map(|e| e.name.to_lowercase())
            .collect();
        let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) if taken.contains(&n.to_lowercase()) => {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    format!("There is already a layout named \u{201c}{n}\u{201d}."),
                ));
            }
            Some(n) => n.to_owned(),
            None => (1..)
                .map(|i| match i {
                    1 => format!("{} copy", source.name),
                    _ => format!("{} copy {i}", source.name),
                })
                .find(|n| !taken.contains(&n.to_lowercase()))
                .unwrap_or_else(|| format!("{} copy", source.name)),
        };
        let base = slug(&name);
        let new_id = (1..)
            .map(|i| match i {
                1 => base.clone(),
                _ => format!("{base}-{i}"),
            })
            .find(|candidate| !self.file(candidate).exists())
            .unwrap_or(base);
        self.write(&new_id, Layout { name, ..source })
    }

    /// The saved file again; with `to_original`, writes the shipped original
    /// of the layout's preset (keeping the layout's own name) and keeps what
    /// it replaces as a version.
    pub fn revert(&self, id: &str, to_original: bool) -> ApiResult<SavedLayout> {
        let _guard = self.guard()?;
        if !to_original {
            let layout = self.load(id)?;
            return Ok(SavedLayout {
                id: id.to_owned(),
                layout,
            });
        }
        if !is_valid_id(id) {
            return Err(invalid_id(id));
        }
        let current = self.read(id)?.map(|b| parse_bytes(&b));
        let (shipped, name) = match current {
            Some(Content::Layout(l)) => (l.preset, Some(l.name)),
            Some(Content::Broken(_)) => (preset_of_id(id), None),
            None => match preset_of_id(id) {
                Some(p) => (Some(p), None),
                None => return Err(not_found(id)),
            },
        };
        let Some(p) = shipped else {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "Only a layout that started from a preset can be reset to its original.",
            ));
        };
        let original = preset(p);
        let name = name.unwrap_or_else(|| original.name.clone());
        self.write(id, Layout { name, ..original })
    }

    /// The kept versions of `id`, newest first.
    pub fn history(&self, id: &str) -> ApiResult<Vec<LayoutVersion>> {
        let _guard = self.guard()?;
        if !is_valid_id(id) {
            return Err(invalid_id(id));
        }
        let versions = self.versions(id)?;
        if versions.is_empty() && !self.file(id).exists() {
            return Err(not_found(id));
        }
        Ok(versions
            .into_iter()
            .rev()
            .map(|v| {
                let content = fs::read(&v.path).map_or_else(
                    |e| Content::Broken(format!("Cannot be read: {e}")),
                    |b| parse_bytes(&b),
                );
                let (name, broken) = match content {
                    Content::Layout(l) => (Some(l.name), None),
                    Content::Broken(reason) => (None, Some(reason)),
                };
                LayoutVersion {
                    number: v.number,
                    saved_at: iso_utc(v.saved_ms),
                    name,
                    broken,
                }
            })
            .collect())
    }

    /// Makes version `number` current; the content it replaces is kept as
    /// the newest version, and the restored version stays in the history.
    pub fn restore(&self, id: &str, number: u32) -> ApiResult<SavedLayout> {
        let _guard = self.guard()?;
        if !is_valid_id(id) {
            return Err(invalid_id(id));
        }
        let version = self
            .versions(id)?
            .into_iter()
            .find(|v| v.number == number)
            .ok_or_else(|| {
                ApiError::new(ErrorCode::NotFound, "That version is no longer kept.")
                    .with_detail(format!("layout {id:?} has no version {number}"))
            })?;
        let bytes = fs::read(&version.path)
            .map_err(|e| io_error("The version could not be read.", &version.path, &e))?;
        match parse_bytes(&bytes) {
            Content::Layout(layout) => self.write(id, layout),
            Content::Broken(reason) => Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "This version cannot be restored: it is not a valid layout.",
            )
            .with_detail(reason)),
        }
    }
}

#[cfg(test)]
mod tests;
