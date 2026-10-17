//! The scanner: finds font files in source folders and tells a rescan what
//! changed, runs a scan as a job that reports progress, and watches folders
//! for changes.

pub mod changes;
pub mod discovery;
pub mod job;
pub mod watch;

pub use changes::{FileHash, FileIndex, IndexedFile, Rescan, hash_and_parse};
pub use discovery::{Discovery, FONT_EXTENSIONS, FoundFile, Unreadable, discover, is_font_file};
pub use job::{PROGRESS_INTERVAL, ScanJob, Throttle};
pub use watch::{Debounce, Trigger, WatchSettings, WatchUpdate, Watcher};
