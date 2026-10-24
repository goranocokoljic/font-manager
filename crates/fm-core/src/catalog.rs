//! Online catalogs (E22.1, ADR 0014): a [`CatalogAdapter`] per font service
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

pub mod google;

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fm_api::ErrorCode;
use fm_api::catalog::{Catalog, CatalogFamily, CatalogId};
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
}

/// The files of one catalog family and the license they come with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFiles {
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
    vec![Arc::new(google::GoogleFonts::new())]
}

/// The name of a catalog's cache file.
fn cache_file(id: CatalogId) -> &'static str {
    match id {
        CatalogId::GoogleFonts => "google-fonts.json",
    }
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
        }
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

    fn adapter(&self, id: CatalogId) -> FmResult<&dyn CatalogAdapter> {
        self.adapters
            .iter()
            .find(|a| a.id() == id)
            .map(|a| a.as_ref())
            .ok_or_else(|| FmError::unsupported(format!("{id} is not available in this version.")))
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
    let tmp = path.with_extension("json.tmp");
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
fn now_rfc3339() -> String {
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
mod tests;
