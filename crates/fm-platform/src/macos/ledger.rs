//! The macOS activation ledger: the files this app registered with CoreText,
//! and in which mode (rule 3, ADR 0008).
//!
//! CoreText has no per-app record of registrations, so the activator keeps one
//! in a file in the app's data folder. One line per file: `S` (session) or `P`
//! (permanent), a space, and the path's bytes in hex, so a path that is not
//! valid UTF-8 or holds a newline survives exactly. A line that cannot be read
//! is skipped. The file is replaced atomically (write a temporary file, then
//! rename), so a crash leaves the old ledger or the new one, never half of one.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::activator::Mode;
use crate::fs::{path_from_bytes, path_to_bytes};

/// The ledger's file name inside the app's data folder.
pub const LEDGER_FILE: &str = "activation-ledger";

/// The registrations the ledger lists, in the order they were added.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Ledger {
    entries: Vec<(PathBuf, Mode)>,
}

impl Ledger {
    /// Reads the ledger at `file`; a missing file is an empty ledger. A
    /// parent that is a file means the ledger is missing too: Unix says "not
    /// a directory" where Windows says "not found", and both must read the
    /// same (saving there fails later, per face).
    pub(crate) fn load(file: &Path) -> io::Result<Self> {
        match std::fs::read(file) {
            Ok(bytes) => Ok(Self::parse(&bytes)),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(Self::default())
            }
            Err(e) => Err(e),
        }
    }

    fn parse(bytes: &[u8]) -> Self {
        let mut ledger = Self::default();
        for line in bytes.split(|b| *b == b'\n') {
            let Some((mode, hex)) = line.split_first().and_then(|(m, rest)| {
                let mode = match m {
                    b'S' => Mode::Session,
                    b'P' => Mode::Permanent,
                    _ => return None,
                };
                rest.strip_prefix(b" ").map(|hex| (mode, hex))
            }) else {
                continue;
            };
            if let Some(path) = from_hex(hex).and_then(|b| path_from_bytes(&b))
                && ledger.mode_of(&path).is_none()
            {
                ledger.entries.push((path, mode));
            }
        }
        ledger
    }

    /// Writes the ledger to `file`, creating its folder when needed.
    pub(crate) fn save(&self, file: &Path) -> io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = Vec::new();
        for (path, mode) in &self.entries {
            text.push(match mode {
                Mode::Session => b'S',
                Mode::Permanent => b'P',
            });
            text.push(b' ');
            text.extend(to_hex(&path_to_bytes(path)).bytes());
            text.push(b'\n');
        }
        let mut tmp = file.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        {
            let mut out = std::fs::File::create(&tmp)?;
            out.write_all(&text)?;
            out.sync_all()?;
        }
        std::fs::rename(&tmp, file)
    }

    /// The mode `path` is registered in, if the ledger lists it.
    pub(crate) fn mode_of(&self, path: &Path) -> Option<Mode> {
        self.entries
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, mode)| *mode)
    }

    /// Lists `path` in `mode`; a path listed already keeps its entry.
    pub(crate) fn insert(&mut self, path: &Path, mode: Mode) {
        if self.mode_of(path).is_none() {
            self.entries.push((path.to_owned(), mode));
        }
    }

    /// Drops `path` from the ledger.
    pub(crate) fn remove(&mut self, path: &Path) {
        self.entries.retain(|(p, _)| p != path);
    }

    /// Every entry, oldest first.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (&Path, Mode)> {
        self.entries.iter().map(|(p, m)| (p.as_path(), *m))
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(hex: &[u8]) -> Option<Vec<u8>> {
    let (pairs, rest) = hex.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|pair| {
            let s = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(s, 16).ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn scratch_file() -> (PathBuf, PathBuf) {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fm-platform-ledger-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        (dir.join("nested").join(LEDGER_FILE), dir)
    }

    #[test]
    fn a_missing_file_is_an_empty_ledger() {
        let (file, _) = scratch_file();
        assert_eq!(Ledger::load(&file).expect("loads"), Ledger::default());
    }

    #[test]
    fn a_ledger_under_a_file_is_empty_on_every_os() {
        let (_, dir) = scratch_file();
        let _ = std::fs::create_dir_all(&dir);
        let blocker = dir.join("blocker");
        let _ = std::fs::write(&blocker, b"");
        let loaded = Ledger::load(&blocker.join(LEDGER_FILE));
        let saved = Ledger::default().save(&blocker.join(LEDGER_FILE));
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(loaded.expect("loads"), Ledger::default());
        assert!(saved.is_err());
    }

    #[test]
    fn entries_round_trip_through_the_file() {
        let (file, dir) = scratch_file();
        let mut ledger = Ledger::default();
        ledger.insert(Path::new("/fonts/a b.ttf"), Mode::Session);
        ledger.insert(Path::new("/fonts/line\nbreak.otf"), Mode::Permanent);
        ledger.insert(Path::new("/fonts/Schrift-ü.ttc"), Mode::Permanent);
        ledger.save(&file).expect("saves");
        let loaded = Ledger::load(&file).expect("loads");
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(loaded, ledger);
        assert_eq!(
            loaded.mode_of(Path::new("/fonts/line\nbreak.otf")),
            Some(Mode::Permanent)
        );
    }

    #[test]
    fn a_non_utf8_path_round_trips() {
        // On Unix: `A`, two NULs and a lone 0xD8; on Windows: `A` and an
        // unpaired surrogate. Neither is valid UTF-8.
        let path = path_from_bytes(&[0x41, 0x00, 0x00, 0xD8]).expect("a path");
        let (file, dir) = scratch_file();
        let mut ledger = Ledger::default();
        ledger.insert(&path, Mode::Session);
        ledger.save(&file).expect("saves");
        let loaded = Ledger::load(&file).expect("loads");
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(loaded.mode_of(&path), Some(Mode::Session));
    }

    #[test]
    fn unreadable_lines_are_skipped() {
        let good = to_hex(&path_to_bytes(Path::new("/fonts/ok.ttf")));
        let text = format!("X {good}\nS zz\nP 123\nS{good}\n\nP {good}\nS {good}\n");
        let ledger = Ledger::parse(text.as_bytes());
        let entries: Vec<_> = ledger.entries().collect();
        assert_eq!(entries, vec![(Path::new("/fonts/ok.ttf"), Mode::Permanent)]);
    }

    #[test]
    fn insert_keeps_the_first_mode_and_remove_drops_the_path() {
        let mut ledger = Ledger::default();
        ledger.insert(Path::new("/a.ttf"), Mode::Session);
        ledger.insert(Path::new("/a.ttf"), Mode::Permanent);
        assert_eq!(ledger.mode_of(Path::new("/a.ttf")), Some(Mode::Session));
        ledger.remove(Path::new("/a.ttf"));
        assert_eq!(ledger.mode_of(Path::new("/a.ttf")), None);
    }
}
