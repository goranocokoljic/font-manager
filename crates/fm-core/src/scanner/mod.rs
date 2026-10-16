//! The scanner: finds font files in source folders.

pub mod discovery;

pub use discovery::{Discovery, FONT_EXTENSIONS, FoundFile, Unreadable, discover, is_font_file};
