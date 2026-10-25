//! The preview cache (E4.3): rendered thumbnails kept on disk, so the grid,
//! the CLI and MCP draw a face once per look.
//!
//! An entry is one PNG file in the cache folder (`<cache>/previews`, ADR 0007),
//! named by its [`CacheKey`]: a hash of the face (the file's blake3 hash plus
//! the face's index in a collection) and a hash of the render settings (text,
//! size, axes, features, colours). A changed font file has a new hash, so its
//! old thumbnails are never served; they age out.
//!
//! The folder holds at most a byte limit (default [`DEFAULT_LIMIT_BYTES`]);
//! past it the least recently used entries are removed. Use order lives in
//! memory and in each file's modification time, so it survives a restart.
//! The cache only ever writes, reads and removes its own files in its own
//! folder; it never touches a font file.
//!
//! A cache that cannot be written is not an error for the caller:
//! [`PreviewCache::get_or_render`] still returns the render.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use fm_api::preview::{PreviewImage, PreviewRequest};
use fm_platform::AppPaths;

use crate::scanner::FileHash;
use crate::{FmError, FmResult};

/// The cache folder's name inside the app's cache folder.
pub const CACHE_DIR: &str = "previews";
/// The default size limit: 500 MB.
pub const DEFAULT_LIMIT_BYTES: u64 = 500 * MB;
/// Bumped when the rasterizer draws differently, so old entries miss.
const RENDER_VERSION: u32 = 1;
const MB: u64 = 1024 * 1024;
const EXTENSION: &str = "png";
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_IEND: &[u8; 12] = b"\0\0\0\0IEND\xaeB`\x82";

/// A size in megabytes (as in [`fm_api::settings::Settings::cache_limit_mb`])
/// in bytes.
pub fn limit_from_mb(mb: u32) -> u64 {
    u64::from(mb) * MB
}

/// The cache folder inside the app folders.
pub fn cache_dir(paths: &AppPaths) -> PathBuf {
    paths.cache.join(CACHE_DIR)
}

/// What a thumbnail is cached under: the face hash plus the settings hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    face: [u8; 16],
    settings: [u8; 16],
}

impl CacheKey {
    /// The key of `req` drawn from face `collection_index` of the file whose
    /// bytes hash to `file`. `req.face` is not part of the key: the same face
    /// found at two paths, or under a new id after a rebuild, shares entries.
    pub fn new(file: &FileHash, collection_index: u32, req: &PreviewRequest) -> Self {
        Self {
            face: face_hash(file, collection_index),
            settings: settings_hash(req),
        }
    }

    /// The key of a comparison (E27.3): the faces, in row order, each the
    /// file hash and collection index, plus the settings of one row
    /// (`req.face` is not part of it). It never equals a single face's key.
    pub fn compare(faces: &[(&FileHash, u32)], req: &PreviewRequest) -> Self {
        let mut h = blake3::Hasher::new();
        h.update(b"fm-preview-compare");
        h.update(&(faces.len() as u64).to_le_bytes());
        for (file, index) in faces {
            h.update(&face_hash(file, *index));
        }
        Self {
            face: truncate(h.finalize()),
            settings: settings_hash(req),
        }
    }

    fn file_name(&self) -> String {
        format!("{}-{}.{EXTENSION}", hex(&self.face), hex(&self.settings))
    }

    fn parse(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(&format!(".{EXTENSION}"))?;
        let (face, settings) = stem.split_once('-')?;
        Some(Self {
            face: unhex(face)?,
            settings: unhex(settings)?,
        })
    }
}

fn face_hash(file: &FileHash, collection_index: u32) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    h.update(b"fm-preview-face");
    h.update(file.as_bytes());
    h.update(&collection_index.to_le_bytes());
    truncate(h.finalize())
}

/// Every field of the request that changes the pixels, length-prefixed so
/// two requests never encode the same.
fn settings_hash(req: &PreviewRequest) -> [u8; 16] {
    fn bytes(h: &mut blake3::Hasher, b: &[u8]) {
        h.update(&(b.len() as u64).to_le_bytes());
        h.update(b);
    }
    fn colour(h: &mut blake3::Hasher, c: Option<&String>) {
        match c {
            Some(c) => {
                h.update(&[1]);
                bytes(h, c.as_bytes());
            }
            None => {
                h.update(&[0]);
            }
        }
    }
    let mut h = blake3::Hasher::new();
    h.update(b"fm-preview-settings");
    h.update(&RENDER_VERSION.to_le_bytes());
    bytes(&mut h, req.text.as_bytes());
    h.update(&req.size_px.to_bits().to_le_bytes());
    h.update(&(req.variations.len() as u64).to_le_bytes());
    for v in &req.variations {
        bytes(&mut h, v.tag.as_bytes());
        h.update(&v.value.to_bits().to_le_bytes());
    }
    h.update(&(req.features.len() as u64).to_le_bytes());
    for f in &req.features {
        bytes(&mut h, f.tag.as_bytes());
        h.update(&[u8::from(f.on)]);
    }
    colour(&mut h, req.foreground.as_ref());
    colour(&mut h, req.background.as_ref());
    truncate(h.finalize())
}

fn truncate(hash: blake3::Hash) -> [u8; 16] {
    let mut out = [0; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
}

fn hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut out = [0; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Rendered thumbnails on disk with a least-recently-used size limit. Safe to
/// share between threads.
#[derive(Debug)]
pub struct PreviewCache {
    dir: PathBuf,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    limit: u64,
    total: u64,
    /// Ticks on every use; a higher `used` is more recent.
    clock: u64,
    entries: HashMap<CacheKey, Entry>,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    size: u64,
    used: u64,
}

impl PreviewCache {
    /// Opens the cache in `dir`, creating the folder, and learns its entries
    /// from the files there (oldest modification time = least recently used).
    /// Files left half-written by a crash are removed; files the cache did not
    /// name are left alone. When the folder holds more than `limit_bytes`, the
    /// oldest entries are removed now.
    pub fn open(dir: &Path, limit_bytes: u64) -> FmResult<Self> {
        let failed = "The preview cache could not be opened.";
        fs::create_dir_all(dir).map_err(|e| FmError::io(failed, dir, &e))?;
        let mut found = Vec::new();
        for item in fs::read_dir(dir).map_err(|e| FmError::io(failed, dir, &e))? {
            let Ok(item) = item else { continue };
            let name = item.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.ends_with(".tmp") {
                let _ = fs::remove_file(item.path());
                continue;
            }
            let Some(key) = CacheKey::parse(name) else {
                continue;
            };
            let Ok(meta) = item.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            found.push((meta.modified().ok(), key, meta.len()));
        }
        found.sort_by_key(|(modified, _, _)| *modified);
        let mut state = State {
            limit: limit_bytes,
            total: 0,
            clock: 0,
            entries: HashMap::with_capacity(found.len()),
        };
        for (_, key, size) in found {
            state.clock += 1;
            state.total += size;
            state.entries.insert(
                key,
                Entry {
                    size,
                    used: state.clock,
                },
            );
        }
        let cache = Self {
            dir: dir.to_path_buf(),
            state: Mutex::new(state),
        };
        cache.evict(&mut cache.lock());
        Ok(cache)
    }

    /// The cache folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The size limit in bytes.
    pub fn limit(&self) -> u64 {
        self.lock().limit
    }

    /// Changes the size limit; entries past it are removed now.
    pub fn set_limit(&self, limit_bytes: u64) {
        let mut state = self.lock();
        state.limit = limit_bytes;
        self.evict(&mut state);
    }

    /// The bytes the entries take on disk.
    pub fn total_bytes(&self) -> u64 {
        self.lock().total
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    /// Whether the cache holds no entry.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The cached thumbnail of `key`, now the most recently used, or `None`.
    /// A file that is gone or damaged is a miss and leaves the cache.
    pub fn get(&self, key: &CacheKey) -> Option<PreviewImage> {
        if !self.lock().entries.contains_key(key) {
            return None;
        }
        let path = self.dir.join(key.file_name());
        let image = read_entry(&path);
        let mut state = self.lock();
        match image {
            Some(image) => {
                state.clock += 1;
                let clock = state.clock;
                if let Some(entry) = state.entries.get_mut(key) {
                    entry.used = clock;
                }
                Some(image)
            }
            None => {
                if let Some(entry) = state.entries.remove(key) {
                    state.total -= entry.size;
                }
                let _ = fs::remove_file(&path);
                None
            }
        }
    }

    /// Stores `image` under `key` as the most recently used entry, then
    /// removes the least recently used entries past the limit. An image
    /// larger than the whole limit is not stored.
    pub fn put(&self, key: &CacheKey, image: &PreviewImage) -> FmResult<()> {
        let size = image.png.len() as u64;
        if size > self.limit() || png_size(&image.png) != Some((image.width, image.height)) {
            return Ok(());
        }
        let path = self.dir.join(key.file_name());
        write_atomic(&path, &image.png)
            .map_err(|e| FmError::io("A preview could not be cached.", &path, &e))?;
        let mut state = self.lock();
        state.clock += 1;
        let used = state.clock;
        if let Some(old) = state.entries.insert(*key, Entry { size, used }) {
            state.total -= old.size;
        }
        state.total += size;
        self.evict(&mut state);
        Ok(())
    }

    /// The cached thumbnail of `key`, or `render()`'s, which is then cached.
    /// A failed cache write is logged, not returned: the render still counts.
    pub fn get_or_render(
        &self,
        key: &CacheKey,
        render: impl FnOnce() -> FmResult<PreviewImage>,
    ) -> FmResult<PreviewImage> {
        if let Some(hit) = self.get(key) {
            return Ok(hit);
        }
        let image = render()?;
        if let Err(e) = self.put(key, &image) {
            tracing::warn!(error = %e, "preview not cached");
        }
        Ok(image)
    }

    /// Removes every entry.
    pub fn clear(&self) {
        let mut state = self.lock();
        for key in state.entries.keys() {
            let _ = fs::remove_file(self.dir.join(key.file_name()));
        }
        state.entries.clear();
        state.total = 0;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // The state holds plain numbers; a panic elsewhere cannot leave it
        // half-updated in a way that matters more than a miss.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn evict(&self, state: &mut State) {
        if state.total <= state.limit {
            return;
        }
        let mut by_use: Vec<(u64, CacheKey)> =
            state.entries.iter().map(|(k, e)| (e.used, *k)).collect();
        by_use.sort_unstable_by_key(|(used, _)| *used);
        for (_, key) in by_use {
            if state.total <= state.limit {
                break;
            }
            if let Some(entry) = state.entries.remove(&key) {
                state.total -= entry.size;
                if let Err(e) = fs::remove_file(self.dir.join(key.file_name()))
                    && e.kind() != io::ErrorKind::NotFound
                {
                    tracing::warn!(error = %e, "stale preview not removed");
                }
            }
        }
    }
}

/// Reads an entry and marks it used on disk (its modification time), so the
/// order survives a restart. `None` when the file is gone or is not a whole PNG.
fn read_entry(path: &Path) -> Option<PreviewImage> {
    let mut png = Vec::new();
    match File::options().read(true).write(true).open(path) {
        Ok(mut file) => {
            file.read_to_end(&mut png).ok()?;
            let _ = file.set_modified(SystemTime::now());
        }
        // Read-only file: still a hit, only the order on disk stays old.
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => png = fs::read(path).ok()?,
        Err(_) => return None,
    }
    let (width, height) = png_size(&png)?;
    Some(PreviewImage { width, height, png })
}

/// Width and height from the IHDR chunk of a PNG that starts and ends like one.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 8 + 25 + PNG_IEND.len()
        || !png.starts_with(PNG_SIGNATURE)
        || !png.ends_with(PNG_IEND)
        || png.get(12..16)? != b"IHDR"
    {
        return None;
    }
    let width = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// Writes to a temporary file in the same folder, then renames it into place:
/// a reader, or a crash, never sees half a PNG.
fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.{n}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests;
