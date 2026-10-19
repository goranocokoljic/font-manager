//! A trash that is a plain folder, for tests of the core: moves are real
//! renames, so the tests see files leave and come back, without filling the
//! developer's own trash.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

use super::{Trash, TrashError, TrashErrorKind, Trashed};

/// A trash in the folder `dir` (created on the first move).
#[derive(Debug)]
pub struct MockTrash {
    dir: PathBuf,
    next: AtomicU32,
    refuse: Mutex<HashSet<PathBuf>>,
}

impl MockTrash {
    /// A trash in `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            next: AtomicU32::new(0),
            refuse: Mutex::new(HashSet::new()),
        }
    }

    /// Makes every move of `path` fail as if its drive had no trash.
    pub fn refuse(&self, path: impl Into<PathBuf>) {
        self.refuse
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path.into());
    }

    /// The folder the trashed files are in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Trash for MockTrash {
    fn trash(&self, path: &Path) -> Result<Trashed, TrashError> {
        if self
            .refuse
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(path)
        {
            return Err(TrashError::new(
                TrashErrorKind::NoTrash,
                path.display().to_string(),
            ));
        }
        fs::create_dir_all(&self.dir).map_err(|e| TrashError::io(&e, &self.dir))?;
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let name = path.file_name().map_or_else(
            || format!("{n}"),
            |name| format!("{n}-{}", name.to_string_lossy()),
        );
        let location = self.dir.join(name);
        let info = self.dir.join(format!("{n}.info"));
        fs::rename(path, &location).map_err(|e| TrashError::io(&e, path))?;
        fs::write(&info, path.display().to_string()).map_err(|e| TrashError::io(&e, &info))?;
        Ok(Trashed {
            original: path.to_path_buf(),
            location,
            info: Some(info),
        })
    }
}
