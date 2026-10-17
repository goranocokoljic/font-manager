//! The scanner: finds font files in source folders and tells a rescan what
//! changed, and watches them for changes.

pub mod changes;
pub mod discovery;
pub mod watch;

pub use changes::{FileHash, FileIndex, IndexedFile, Rescan, hash_and_parse};
pub use discovery::{Discovery, FONT_EXTENSIONS, FoundFile, Unreadable, discover, is_font_file};
pub use watch::{Debounce, Trigger, WatchSettings, WatchUpdate, Watcher};
