//! Online catalogs (E22.1, E22.5, ADR 0014): a [`CatalogAdapter`] per font service
//! (Google Fonts, Fontshare)
//! and [`Catalogs`], their on/off state and metadata cache in the app's data
//! folder.
//!
//! - **Off by default.** A catalog is fetched only after the user turned it
//!   on, and only by [`Catalogs::refresh`] (and, for a download,
//!   [`Catalogs::resolve_files`]). Listing catalogs and their families reads
//!   the cache and never makes a request.
//! - **One fetcher.** Every request goes through the [`Fetcher`] the
//!   catalogs were made with: [`HttpFetcher`] in the app, a local or a
//!   failing one in tests.
//! - **The cache.** A refresh parses the whole answer first and then
//!   replaces `<catalog>.json` through a temporary file, so a failed or
//!   malformed answer leaves the previous cache as it was. The on/off state
//!   is `catalogs.json` in the same folder.
//! - **Previews (E22.2).** [`Catalogs::font_source`] fetches one file of a
//!   listed family into the app's cache folder the first time a row shows
//!   it, and reads it from there after; refused while the catalog is off.
//! - **Downloads (E22.3).** [`Catalogs::download`] writes a family's files
//!   into a subfolder named after it inside a folder the user chose: every
//!   file is fetched and checked first, then each goes through a temporary
//!   file and is linked into place without replacing a file already there.
//! - **Updates (E22.4).** [`Catalogs::check_updates`] compares the version
//!   recorded at each download with the catalog's list; it refreshes the
//!   catalogs first only when the caller asks (the user's "Check for
//!   updates"), never on its own.

pub mod fontshare;
pub mod google;

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fm_api::ErrorCode;
use fm_api::catalog::{
    Catalog, CatalogDownload, CatalogFamily, CatalogFileKind, CatalogId, CatalogUpdate,
    CatalogUpdateStatus,
};
use fm_api::common::{Page, Paged};
use fm_api::library::OpenLicense;
use serde::{Deserialize, Serialize};

use crate::error::{FmError, FmResult};

/// The catalogs folder inside the app's data folder.
pub const CATALOGS_DIR: &str = "catalogs";
/// The on/off state of every catalog, inside the catalogs folder.
const STATE_FILE: &str = "catalogs.json";
/// The version of the cache file's layout; a cache of another version is
/// read as no cache.
const CACHE_FORMAT: u32 = 1;
/// Most families one page lists.
pub const MAX_PAGE: u32 = 500;
/// The largest answer a catalog may send (Google Fonts' index is about 3 MB).
pub const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// Gets the body of a URL. The only way a catalog reaches the network.
pub trait Fetcher: Send + Sync {
    /// The body of a successful `GET url`; a failed request, an error status
    /// or an answer over [`MAX_BODY_BYTES`] is an [`ErrorCode::Io`] error.
    fn get(&self, url: &str) -> FmResult<Vec<u8>>;
}

/// A downloadable file of a catalog family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFile {
    /// Its file name ("Lato-Bold.ttf").
    pub name: String,
    /// Where to download it.
    pub url: String,
    /// It is a variable font (`Inter[opsz,wght].ttf`), not one static style.
    pub variable: bool,
}

/// The files of one catalog family and the license they come with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFiles {
    /// The catalog's id of the family: the name for Google Fonts, the slug
    /// for Fontshare.
    pub family_id: String,
    /// The font files.
    pub files: Vec<CatalogFile>,
    /// The license, from the license file shipped with them.
    pub license: Option<OpenLicense>,
}

/// One font service: where its index is, how to read it, and where a
/// family's files are.
pub trait CatalogAdapter: Send + Sync {
    /// Which catalog this is.
    fn id(&self) -> CatalogId;
    /// The catalog's web site.
    fn homepage(&self) -> String;
    /// Fetches the catalog's index through `fetcher`.
    fn fetch_index(&self, fetcher: &dyn Fetcher) -> FmResult<Vec<u8>>;
    /// Reads an index into families. An answer that is not the catalog's
    /// index, or names no family, is an error.
    fn parse_index(&self, body: &[u8]) -> FmResult<Vec<CatalogFamily>>;
    /// Fetches where the files of `family` are, through `fetcher`.
    fn resolve_files(&self, fetcher: &dyn Fetcher, family: &str) -> FmResult<CatalogFiles>;
}

/// Every catalog the app ships, at its real address.
pub fn default_adapters() -> Vec<Arc<dyn CatalogAdapter>> {
    vec![
        Arc::new(google::GoogleFonts::new()),
        Arc::new(fontshare::Fontshare::new()),
    ]
}

/// The name of a catalog's cache file.
fn cache_file(id: CatalogId) -> &'static str {
    match id {
        CatalogId::GoogleFonts => "google-fonts.json",
        CatalogId::Fontshare => "fontshare.json",
    }
}

/// A catalog's name in file names (`google-fonts`).
fn cache_stem(id: CatalogId) -> &'static str {
    cache_file(id).trim_end_matches(".json")
}

/// The extensions a preview file may have, best first.
const PREVIEW_EXTENSIONS: [&str; 4] = ["woff2", "woff", "ttf", "otf"];

/// The file name (without extension) of a family's preview: a hash of the
/// name, so any name is a safe file name and no two names share one.
fn preview_stem(family: &str) -> String {
    blake3::hash(family.as_bytes()).to_hex()[..32].to_owned()
}

/// The preview file already in `dir`, if any.
fn cached_preview(dir: &Path, stem: &str) -> Option<PathBuf> {
    PREVIEW_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

/// The file that previews a family, with its extension: the upright
/// Regular, else an upright variable file (`Inter[opsz,wght].ttf`), else any
/// upright file, else the first; only font files count.
pub fn preview_file(files: &[CatalogFile]) -> Option<(&CatalogFile, &'static str)> {
    let fonts: Vec<(&CatalogFile, &'static str, String)> = files
        .iter()
        .filter_map(|f| {
            let base = f.name.rsplit('/').next().unwrap_or(&f.name);
            let (stem, ext) = base.rsplit_once('.')?;
            let ext = PREVIEW_EXTENSIONS
                .iter()
                .find(|e| e.eq_ignore_ascii_case(ext))?;
            Some((f, *ext, stem.to_ascii_lowercase()))
        })
        .collect();
    let upright = |stem: &str| !stem.contains("italic") && !stem.contains("oblique");
    let rank = |stem: &str| -> u8 {
        if !upright(stem) {
            3
        } else if stem.ends_with("-regular") || stem == "regular" {
            0
        } else if stem.contains('[') {
            1
        } else {
            2
        }
    };
    fonts
        .iter()
        .enumerate()
        .min_by_key(|(i, (_, _, stem))| (rank(stem), *i))
        .map(|(_, (f, ext, _))| (*f, *ext))
}

/// The folder [`Catalogs::download`] puts `family` into inside `into`: a
/// subfolder named after the family.
pub fn family_folder(into: &Path, family: &str) -> PathBuf {
    into.join(safe_file_name(family).unwrap_or_else(|| "Font".to_owned()))
}

/// One file a download wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedFile {
    /// Its file name.
    pub name: String,
    /// Where it was written.
    pub path: PathBuf,
    /// The blake3 hash of its bytes.
    pub hash: [u8; 32],
}

/// What [`Catalogs::download`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// The catalog's id of the family.
    pub family_id: String,
    /// The family's version, else the date the catalog last changed it.
    pub version: Option<String>,
    /// The kind of files written (the other kind when the family has none
    /// of the kind asked for).
    pub kind: CatalogFileKind,
    /// The family's folder.
    pub folder: PathBuf,
    /// The files written, in the catalog's order.
    pub written: Vec<DownloadedFile>,
    /// The names not written because a file of that name was there.
    pub skipped: Vec<String>,
    /// Why the download stopped after [`Downloaded::written`] were placed
    /// (a file that could not be saved); the files after it were not
    /// written. `None` when every file was written or skipped. A failure
    /// before any file was written is an `Err` of [`Catalogs::download`].
    pub stopped: Option<FmError>,
}

/// The font files of `kind` (else of the other kind) with the name each is
/// saved under, in the catalog's order; a name that comes twice is kept
/// once. `None` when the family has no font file at all.
fn download_files(
    files: &[CatalogFile],
    kind: CatalogFileKind,
) -> Option<(CatalogFileKind, Vec<(String, &CatalogFile)>)> {
    let fonts: Vec<(String, &CatalogFile)> = files
        .iter()
        .filter_map(|f| {
            let base = f.name.rsplit(['/', '\\']).next().unwrap_or(&f.name);
            let (_, ext) = base.rsplit_once('.')?;
            if !PREVIEW_EXTENSIONS
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
            {
                return None;
            }
            Some((safe_file_name(base)?, f))
        })
        .collect();
    let pick = |variable: bool| -> Vec<(String, &CatalogFile)> {
        let mut seen = BTreeSet::new();
        fonts
            .iter()
            .filter(|(name, f)| f.variable == variable && seen.insert(name.to_lowercase()))
            .cloned()
            .collect()
    };
    let other = match kind {
        CatalogFileKind::Static => CatalogFileKind::Variable,
        CatalogFileKind::Variable => CatalogFileKind::Static,
    };
    [kind, other].into_iter().find_map(|k| {
        let chosen = pick(k == CatalogFileKind::Variable);
        (!chosen.is_empty()).then_some((k, chosen))
    })
}

/// `name` as a file or folder name that is safe on every OS: no path
/// separator, no character Windows refuses, no control character, no
/// leading or trailing dot or space, not a name Windows reserves (`CON`).
/// `None` when nothing is left.
pub fn safe_file_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '_') {
        return None;
    }
    let stem = cleaned.split('.').next().unwrap_or(cleaned).trim_end();
    let reserved = ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|r| stem.eq_ignore_ascii_case(r))
        || (stem.len() == 4
            && stem.is_char_boundary(3)
            && ["COM", "LPT"]
                .iter()
                .any(|r| stem[..3].eq_ignore_ascii_case(r))
            && stem.as_bytes()[3].is_ascii_digit());
    Some(if reserved {
        format!("_{cleaned}")
    } else {
        cleaned.to_owned()
    })
}

/// `available` is a newer version than `downloaded`, both as a catalog
/// gives them: a version (`1.002`, `Version 2.1`) or a date (`2025-09-04`).
/// Their runs of digits are compared as numbers, left to right
/// (`1.10` > `1.9`, `2026-01-02` > `2025-12-31`); a missing run counts as
/// 0, so `1.0` equals `1`. When either has no digit, only a different text
/// counts as newer.
pub fn is_newer_version(available: &str, downloaded: &str) -> bool {
    /// The runs of digits, without leading zeros.
    fn runs(text: &str) -> Vec<&str> {
        text.split(|c: char| !c.is_ascii_digit())
            .filter(|run| !run.is_empty())
            .map(|run| match run.trim_start_matches('0') {
                "" => "0",
                trimmed => trimmed,
            })
            .collect()
    }
    let (now, had) = (runs(available), runs(downloaded));
    if now.is_empty() || had.is_empty() {
        return available.trim() != downloaded.trim();
    }
    for i in 0..now.len().max(had.len()) {
        let (a, b) = (
            now.get(i).copied().unwrap_or("0"),
            had.get(i).copied().unwrap_or("0"),
        );
        // Without leading zeros, the longer run is the larger number.
        match a.len().cmp(&b.len()).then_with(|| a.cmp(b)) {
            std::cmp::Ordering::Equal => {}
            order => return order.is_gt(),
        }
    }
    false
}

/// Writes `data` to a new file at `path` through a temporary file next to
/// it, without ever replacing a file: `false` (and nothing written) when
/// `path` is already there. The temporary file is hard-linked into place,
/// which fails when the name is taken; on a file system without hard links
/// it is renamed after a check.
fn place_new(path: &Path, data: &[u8]) -> io::Result<bool> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if fs::symlink_metadata(path).is_ok() {
        return Ok(false);
    }
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{name}.{}-{n}.part", std::process::id()));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| {
            io::Write::write_all(&mut f, data)?;
            f.sync_all()
        });
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let placed = match fs::hard_link(&tmp, path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) if fs::symlink_metadata(path).is_ok() => Ok(false),
        Err(_) => fs::rename(&tmp, path).map(|()| true),
    };
    // The app's own temporary file, never the user's: gone in every case.
    let _ = fs::remove_file(&tmp);
    placed
}

/// A catalog's cache file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cache {
    format: u32,
    /// When it was refreshed (RFC 3339, UTC).
    refreshed_at: String,
    /// The families, by name.
    families: Vec<CatalogFamily>,
}

/// The on/off state file.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct State {
    #[serde(default)]
    enabled: BTreeSet<CatalogId>,
}

/// The catalogs, their on/off state and their caches in one folder.
pub struct Catalogs {
    dir: PathBuf,
    fetcher: Arc<dyn Fetcher>,
    adapters: Vec<Arc<dyn CatalogAdapter>>,
    /// Caches read so far; `None` when a catalog has no cache.
    loaded: Mutex<HashMap<CatalogId, Option<Arc<Cache>>>>,
    /// Held by every write, so two refreshes never interleave.
    writing: Mutex<()>,
    /// Where preview files go (`<cache>/catalogs`); `None` without one.
    previews: Option<PathBuf>,
    /// One lock per preview file being fetched, so two rows showing the same
    /// family make one request.
    fetching: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl std::fmt::Debug for Catalogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Catalogs")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl Catalogs {
    /// The shipped catalogs over HTTP, kept in `dir` (the app passes
    /// `<data>/catalogs`). Makes no request.
    pub fn open(dir: PathBuf) -> Self {
        Self::new(dir, Arc::new(HttpFetcher::new()), default_adapters())
    }

    /// `adapters` fetched through `fetcher`, kept in `dir`; tests pass a
    /// local server or a failing fetcher.
    pub fn new(
        dir: PathBuf,
        fetcher: Arc<dyn Fetcher>,
        adapters: Vec<Arc<dyn CatalogAdapter>>,
    ) -> Self {
        Self {
            dir,
            fetcher,
            adapters,
            loaded: Mutex::new(HashMap::new()),
            writing: Mutex::new(()),
            previews: None,
            fetching: Mutex::new(HashMap::new()),
        }
    }

    /// The same catalogs, keeping preview files in `dir` (the app passes
    /// `<cache>/catalogs`).
    #[must_use]
    pub fn with_previews_in(mut self, dir: PathBuf) -> Self {
        self.previews = Some(dir);
        self
    }

    /// Every catalog with its state; makes no request.
    pub fn list(&self) -> FmResult<Vec<Catalog>> {
        let state = self.state();
        self.adapters
            .iter()
            .map(|a| self.catalog(a.as_ref(), &state))
            .collect()
    }

    /// Turns a catalog on or off; makes no request and keeps the cache.
    pub fn set_enabled(&self, id: CatalogId, enabled: bool) -> FmResult<Catalog> {
        let adapter = self.adapter(id)?;
        let _writing = lock(&self.writing);
        let mut state = self.state();
        if enabled {
            state.enabled.insert(id);
        } else {
            state.enabled.remove(&id);
        }
        let text = serde_json::to_string_pretty(&state)
            .map_err(|e| FmError::internal(format!("catalog state: {e}")))?;
        write_atomic(&self.dir.join(STATE_FILE), text.as_bytes())?;
        self.catalog(adapter, &state)
    }

    /// Fetches the catalog's index and replaces its cache. Refused with
    /// `Conflict` while the catalog is off, before any request. A failed or
    /// malformed answer is an error and the previous cache stays.
    pub fn refresh(&self, id: CatalogId) -> FmResult<Catalog> {
        let adapter = self.enabled_adapter(id)?;
        let _writing = lock(&self.writing);
        let failed = |e: FmError| {
            let refreshed = FmError::new(
                e.code(),
                format!("{id} could not be refreshed. {}", e.message()),
            );
            match e.detail() {
                Some(detail) => refreshed.with_detail(detail),
                None => refreshed,
            }
        };
        let body = adapter.fetch_index(self.fetcher.as_ref()).map_err(failed)?;
        let mut families = adapter.parse_index(&body).map_err(failed)?;
        families.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
        let cache = Cache {
            format: CACHE_FORMAT,
            refreshed_at: now_rfc3339(),
            families,
        };
        let text = serde_json::to_vec(&cache)
            .map_err(|e| FmError::internal(format!("catalog cache: {e}")))?;
        write_atomic(&self.dir.join(cache_file(id)), &text)?;
        lock(&self.loaded).insert(id, Some(Arc::new(cache)));
        self.catalog(adapter, &self.state())
    }

    /// A page of the cached families whose name contains `query`, ignoring
    /// case; makes no request. An off catalog, or one never refreshed, has
    /// none.
    pub fn families(
        &self,
        id: CatalogId,
        query: &str,
        page: Page,
    ) -> FmResult<Paged<CatalogFamily>> {
        self.adapter(id)?;
        let empty = Paged {
            items: Vec::new(),
            total: 0,
        };
        if !self.state().enabled.contains(&id) {
            return Ok(empty);
        }
        let Some(cache) = self.cache(id)? else {
            return Ok(empty);
        };
        let query = query.trim().to_lowercase();
        let hits: Vec<&CatalogFamily> = cache
            .families
            .iter()
            .filter(|f| query.is_empty() || f.name.to_lowercase().contains(&query))
            .collect();
        Ok(Paged {
            total: count(hits.len()),
            items: hits
                .into_iter()
                .skip(page.offset as usize)
                .take(page.limit.min(MAX_PAGE) as usize)
                .cloned()
                .collect(),
        })
    }

    /// Where the files of `family` are and their license: a request, so
    /// refused with `Conflict` while the catalog is off.
    pub fn resolve_files(&self, id: CatalogId, family: &str) -> FmResult<CatalogFiles> {
        self.enabled_adapter(id)?
            .resolve_files(self.fetcher.as_ref(), family)
    }

    /// The preview file of `family`, a family in the cached list: fetched
    /// once into the previews folder (two requests: the file list, then the
    /// file [`preview_file`] picks), read from there after. Refused with
    /// `Conflict` while the catalog is off and `NotFound` for a family the
    /// list does not have, both before any request.
    pub fn font_source(&self, id: CatalogId, family: &str) -> FmResult<PathBuf> {
        let adapter = self.enabled_adapter(id)?;
        let dir = self
            .previews
            .as_ref()
            .ok_or_else(|| FmError::unsupported("Catalog previews are not available here."))?
            .join(cache_stem(id));
        let listed = self
            .cache(id)?
            .is_some_and(|c| c.families.iter().any(|f| f.name == family));
        if !listed {
            return Err(FmError::new(
                ErrorCode::NotFound,
                format!("{family} is not in the {id} list. Refresh the catalog."),
            ));
        }
        let stem = preview_stem(family);
        if let Some(path) = cached_preview(&dir, &stem) {
            return Ok(path);
        }
        let gate = Arc::clone(
            lock(&self.fetching)
                .entry(dir.join(&stem))
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        );
        let _fetching = lock(&gate);
        // Another row may have fetched it while this one waited.
        if let Some(path) = cached_preview(&dir, &stem) {
            return Ok(path);
        }
        let unavailable = |e: FmError| {
            let failed = FmError::new(
                e.code(),
                format!(
                    "The preview of {family} could not be fetched. {}",
                    e.message()
                ),
            );
            match e.detail() {
                Some(detail) => failed.with_detail(detail),
                None => failed,
            }
        };
        let files = adapter
            .resolve_files(self.fetcher.as_ref(), family)
            .map_err(unavailable)?;
        let (file, ext) = preview_file(&files.files).ok_or_else(|| {
            unavailable(malformed(
                "a file list",
                format!("{family}: no TTF, OTF or WOFF file"),
            ))
        })?;
        let bytes = self.fetcher.get(&file.url).map_err(unavailable)?;
        if crate::font_file::font_mime(&bytes).is_none() {
            return Err(unavailable(malformed(
                "a file",
                format!("{}: not a font", file.url),
            )));
        }
        let path = dir.join(format!("{stem}.{ext}"));
        write_atomic(&path, &bytes)?;
        Ok(path)
    }

    /// Downloads the `kind` files of `family`, a family in the cached list,
    /// into `<into>/<family>/` (the subfolder is made when missing). A family
    /// without files of that kind gets the other kind. Every file is fetched
    /// and checked to be a font before any is written, so a failed request
    /// writes nothing; each is then written to a temporary file and linked
    /// into place, and a file already there is never replaced (it is listed
    /// in [`Downloaded::skipped`]). Refused with `Conflict` while the catalog
    /// is off and `NotFound` for a family the list does not have or a folder
    /// that is not there, all before any request. A file that cannot be
    /// saved stops the download: with files already placed the answer is
    /// `Ok` with [`Downloaded::stopped`], else the error.
    pub fn download(
        &self,
        id: CatalogId,
        family: &str,
        kind: CatalogFileKind,
        into: &Path,
    ) -> FmResult<Downloaded> {
        self.download_with(self.fetcher.as_ref(), id, family, kind, into)
    }

    /// [`Self::download`] whose requests all end by `deadline` (E29.5): a
    /// request still running then is left to finish on its own and its
    /// answer dropped, so nothing it fetched is written; the download is a
    /// `Io` error like any failed request.
    pub fn download_until(
        &self,
        deadline: Instant,
        id: CatalogId,
        family: &str,
        kind: CatalogFileKind,
        into: &Path,
    ) -> FmResult<Downloaded> {
        let until = Until {
            inner: Arc::clone(&self.fetcher),
            deadline,
        };
        self.download_with(&until, id, family, kind, into)
    }

    fn download_with(
        &self,
        fetcher: &dyn Fetcher,
        id: CatalogId,
        family: &str,
        kind: CatalogFileKind,
        into: &Path,
    ) -> FmResult<Downloaded> {
        let adapter = self.enabled_adapter(id)?;
        let listed = self
            .cache(id)?
            .and_then(|c| c.families.iter().find(|f| f.name == family).cloned())
            .ok_or_else(|| {
                FmError::new(
                    ErrorCode::NotFound,
                    format!("{family} is not in the {id} list. Refresh the catalog."),
                )
            })?;
        if !into.is_dir() {
            return Err(FmError::not_found(format!("The folder {}", into.display())));
        }
        let failed = |e: FmError| {
            let failed = FmError::new(
                e.code(),
                format!("{family} could not be downloaded. {}", e.message()),
            );
            match e.detail() {
                Some(detail) => failed.with_detail(detail),
                None => failed,
            }
        };
        let files = adapter.resolve_files(fetcher, family).map_err(failed)?;
        let (kind, chosen) = download_files(&files.files, kind).ok_or_else(|| {
            failed(malformed(
                "a file list",
                format!("{family}: no TTF, OTF or WOFF file"),
            ))
        })?;
        let mut fetched = Vec::with_capacity(chosen.len());
        for (name, file) in chosen {
            let bytes = fetcher.get(&file.url).map_err(failed)?;
            if crate::font_file::font_mime(&bytes).is_none() {
                return Err(failed(malformed(
                    "a file",
                    format!("{}: not a font", file.url),
                )));
            }
            fetched.push((name, bytes));
        }
        let folder = family_folder(into, family);
        let cannot = "The downloaded fonts could not be saved.";
        fs::create_dir_all(&folder).map_err(|e| FmError::io(cannot, &folder, &e))?;
        let mut written = Vec::new();
        let mut skipped = Vec::new();
        let mut stopped = None;
        for (name, bytes) in fetched {
            let path = folder.join(&name);
            match place_new(&path, &bytes) {
                Ok(true) => {
                    let hash = *blake3::hash(&bytes).as_bytes();
                    written.push(DownloadedFile { name, path, hash });
                }
                Ok(false) => skipped.push(name),
                // The files placed so far stay (they are whole, and the user's
                // folder is never cleaned up behind their back); the caller
                // records them and says the download stopped.
                Err(e) => {
                    stopped = Some(failed(FmError::io(cannot, &path, &e)));
                    break;
                }
            }
        }
        if written.is_empty()
            && let Some(e) = stopped
        {
            return Err(e);
        }
        Ok(Downloaded {
            family_id: files.family_id,
            version: listed.version.or(listed.last_modified),
            kind,
            folder,
            written,
            skipped,
            stopped,
        })
    }

    /// The updates of `downloads` (newest first, as the index lists them):
    /// with `refresh`, every catalog that is on and has a download is
    /// refreshed first (a request each; a failure is the error and the
    /// caches stay), else the cached lists are read as they are and no
    /// request is made. Each family is compared once, at its newest
    /// download: reported when the catalog's version (or last change, as
    /// recorded at the download) is newer, or when the catalog no longer
    /// lists it. A catalog that is off or was never refreshed is skipped,
    /// and so is a family with no version to compare. By catalog, then name.
    pub fn check_updates(
        &self,
        downloads: &[CatalogDownload],
        refresh: bool,
    ) -> FmResult<Vec<CatalogUpdate>> {
        let state = self.state();
        let catalogs: BTreeSet<CatalogId> = downloads
            .iter()
            .map(|d| d.catalog)
            .filter(|id| state.enabled.contains(id))
            .collect();
        if refresh {
            for &id in &catalogs {
                self.refresh(id)?;
            }
        }
        let mut seen = BTreeSet::new();
        let mut updates = Vec::new();
        for download in downloads {
            if !catalogs.contains(&download.catalog)
                || !seen.insert((download.catalog, download.family.as_str()))
            {
                continue;
            }
            let Some(cache) = self.cache(download.catalog)? else {
                continue;
            };
            let listed = cache.families.iter().find(|f| f.name == download.family);
            let available =
                listed.and_then(|f| f.version.clone().or_else(|| f.last_modified.clone()));
            let status = match (listed, &download.version, &available) {
                (None, _, _) => CatalogUpdateStatus::NoLongerListed,
                (Some(_), Some(had), Some(now)) if is_newer_version(now, had) => {
                    CatalogUpdateStatus::UpdateAvailable
                }
                _ => continue,
            };
            updates.push(CatalogUpdate {
                catalog: download.catalog,
                family: download.family.clone(),
                downloaded_version: download.version.clone(),
                available_version: available,
                status,
            });
        }
        updates.sort_by(|a, b| {
            a.catalog
                .cmp(&b.catalog)
                .then_with(|| a.family.to_lowercase().cmp(&b.family.to_lowercase()))
                .then_with(|| a.family.cmp(&b.family))
        });
        Ok(updates)
    }

    fn adapter(&self, id: CatalogId) -> FmResult<&dyn CatalogAdapter> {
        self.adapters
            .iter()
            .find(|a| a.id() == id)
            .map(|a| a.as_ref())
            .ok_or_else(|| FmError::unsupported(format!("{id} is not available in this version.")))
    }

    /// The cached family a document font names (E29.5), in the first
    /// catalog that is on and has it: `family` compared ignoring case and
    /// surrounding space; without a family, the longest listed name the
    /// full name starts with, followed by a space or nothing ("Alegreya Bold
    /// Italic" finds Alegreya). Reads only the cached lists, so it makes no
    /// request: a catalog never refreshed finds nothing, and one whose cache
    /// cannot be read is skipped with a warning.
    pub fn find_cached_family(
        &self,
        family: Option<&str>,
        full_name: Option<&str>,
    ) -> Option<(CatalogId, String)> {
        let clean = |s: Option<&str>| s.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
        let family = clean(family);
        let full_name = clean(full_name);
        if family.is_none() && full_name.is_none() {
            return None;
        }
        let state = self.state();
        for adapter in &self.adapters {
            let id = adapter.id();
            if !state.enabled.contains(&id) {
                continue;
            }
            let cache = match self.cache(id) {
                Ok(Some(cache)) => cache,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(catalog = %id, error = %e, "catalog cache unreadable; skipped");
                    continue;
                }
            };
            let found = match (&family, &full_name) {
                (Some(family), _) => cache
                    .families
                    .iter()
                    .find(|f| f.name.trim().to_lowercase() == *family),
                (None, Some(full)) => cache
                    .families
                    .iter()
                    .filter(|f| {
                        let name = f.name.trim().to_lowercase();
                        !name.is_empty()
                            && full
                                .strip_prefix(&name)
                                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
                    })
                    .max_by_key(|f| f.name.trim().len()),
                (None, None) => None,
            };
            if let Some(found) = found {
                return Some((id, found.name.clone()));
            }
        }
        None
    }

    /// `Conflict` when the catalog `id` is off (or `Unsupported` when the app
    /// has no adapter for it); makes no request.
    pub fn ensure_enabled(&self, id: CatalogId) -> FmResult<()> {
        self.enabled_adapter(id).map(|_| ())
    }

    /// The adapter of a catalog that is on; `Conflict` when it is off.
    fn enabled_adapter(&self, id: CatalogId) -> FmResult<&dyn CatalogAdapter> {
        let adapter = self.adapter(id)?;
        if !self.state().enabled.contains(&id) {
            return Err(FmError::new(
                ErrorCode::Conflict,
                format!("{id} is turned off. Turn it on to fetch its fonts."),
            ));
        }
        Ok(adapter)
    }

    fn catalog(&self, adapter: &dyn CatalogAdapter, state: &State) -> FmResult<Catalog> {
        let id = adapter.id();
        let cache = self.cache(id)?;
        Ok(Catalog {
            id,
            name: id.name().to_owned(),
            homepage: adapter.homepage(),
            enabled: state.enabled.contains(&id),
            refreshed_at: cache.as_ref().map(|c| c.refreshed_at.clone()),
            family_count: cache.as_ref().map_or(0, |c| count(c.families.len())),
        })
    }

    /// The on/off state; a missing file is every catalog off, and so is one
    /// that cannot be read (off is the private choice), with a warning.
    fn state(&self) -> State {
        let path = self.dir.join(STATE_FILE);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!(path = %path.display(), error = %e, "catalog state unreadable; all off");
                State::default()
            }),
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    tracing::warn!(path = %path.display(), error = %e, "catalog state unreadable; all off");
                }
                State::default()
            }
        }
    }

    /// The catalog's cache, read once; `None` when there is none. A cache of
    /// another format is no cache; a damaged one is an error that tells the
    /// user to refresh.
    fn cache(&self, id: CatalogId) -> FmResult<Option<Arc<Cache>>> {
        if let Some(cache) = lock(&self.loaded).get(&id) {
            return Ok(cache.clone());
        }
        let path = self.dir.join(cache_file(id));
        let cache = match fs::read(&path) {
            Ok(bytes) => {
                let cache: Cache = serde_json::from_slice(&bytes).map_err(|e| {
                    FmError::new(
                        ErrorCode::Io,
                        format!("The {id} cache cannot be read. Refresh the catalog."),
                    )
                    .with_detail(format!("{}: {e}", path.display()))
                })?;
                (cache.format == CACHE_FORMAT).then(|| Arc::new(cache))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(FmError::io(
                    format!("The {id} cache cannot be read."),
                    &path,
                    &e,
                ));
            }
        };
        lock(&self.loaded).insert(id, cache.clone());
        Ok(cache)
    }
}

/// A [`Fetcher`] whose every call ends by `deadline` (E29.5). The call runs
/// on its own thread; when the deadline passes first it is left to finish
/// (the inner fetcher's own timeouts end it) and its answer is dropped.
struct Until {
    inner: Arc<dyn Fetcher>,
    deadline: Instant,
}

impl Fetcher for Until {
    fn get(&self, url: &str) -> FmResult<Vec<u8>> {
        let too_long = || {
            FmError::new(
                ErrorCode::Io,
                "The catalog took too long to answer. Try again later.",
            )
            .with_detail(format!("GET {url}: past the deadline"))
        };
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(too_long());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let inner = Arc::clone(&self.inner);
        let owned = url.to_owned();
        std::thread::Builder::new()
            .name("catalog-fetch".to_owned())
            .spawn(move || {
                // The receiver is gone when the deadline passed: nothing to do.
                let _ = tx.send(inner.get(&owned));
            })
            .map_err(|e| FmError::internal(format!("catalog fetch thread: {e}")))?;
        rx.recv_timeout(left).unwrap_or_else(|_| Err(too_long()))
    }
}

/// [`Fetcher`] over HTTPS (reqwest, blocking, rustls). The client is made on
/// the first request, so opening the app costs nothing.
#[derive(Debug, Default)]
pub struct HttpFetcher {
    client: Mutex<Option<reqwest::blocking::Client>>,
}

impl HttpFetcher {
    /// A fetcher that has made no client yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn client(&self) -> FmResult<reqwest::blocking::Client> {
        let mut client = lock(&self.client);
        if let Some(c) = client.as_ref() {
            return Ok(c.clone());
        }
        let made = reqwest::blocking::Client::builder()
            .user_agent(concat!("FontManager/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| FmError::internal(format!("http client: {e}")))?;
        *client = Some(made.clone());
        Ok(made)
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> FmResult<Vec<u8>> {
        let unreachable = |e: &dyn std::fmt::Display| {
            FmError::new(
                ErrorCode::Io,
                "The catalog could not be reached. Check the connection and try again.",
            )
            .with_detail(format!("GET {url}: {e}"))
        };
        let response = self
            .client()?
            .get(url)
            .send()
            .map_err(|e| unreachable(&e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(FmError::new(
                ErrorCode::Io,
                format!("The catalog answered with an error ({status}). Try again later."),
            )
            .with_detail(format!("GET {url}: {status}")));
        }
        let mut body = Vec::new();
        response
            .take(MAX_BODY_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|e| unreachable(&e))?;
        if body.len() as u64 > MAX_BODY_BYTES {
            return Err(FmError::new(
                ErrorCode::Io,
                "The catalog sent more data than Font Manager accepts.",
            )
            .with_detail(format!("GET {url}: over {MAX_BODY_BYTES} bytes")));
        }
        Ok(body)
    }
}

/// An answer that is not what the catalog should send.
pub(crate) fn malformed(what: &str, detail: impl std::fmt::Display) -> FmError {
    FmError::new(
        ErrorCode::Io,
        format!("The catalog sent {what} Font Manager cannot read."),
    )
    .with_detail(detail.to_string())
}

/// Writes `data` through a temporary file, so a crash or a failed write
/// never leaves half a file in place of the old one.
fn write_atomic(path: &Path, data: &[u8]) -> FmResult<()> {
    let failed = "The catalog cache could not be saved.";
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| FmError::io(failed, dir, &e))?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("json")
    ));
    fs::write(&tmp, data).map_err(|e| FmError::io(failed, &tmp, &e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        FmError::io(failed, path, &e)
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Now as RFC 3339 in UTC, whole seconds (`2026-10-24T08:00:00Z`).
pub(crate) fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

/// `secs` since 1970 as RFC 3339 in UTC.
fn rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rest = secs % 86_400;
    // The civil date of a day number (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
pub(crate) mod tests;
