//! Font Manager core.
//!
//! All logic lives here: scanning, parsing (fontations), the SQLite/FTS5 index,
//! search and filters, coverage, previews, activation, export and catalog
//! adapters. Nothing outside this crate computes anything about fonts; every
//! consumer reaches it through `fm-api`.
