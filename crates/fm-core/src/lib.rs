//! Font Manager core.
//!
//! All logic lives here: scanning, parsing (fontations), the SQLite/FTS5 index,
//! search and filters, coverage, previews, activation, export and catalog
//! adapters. Nothing outside this crate computes anything about fonts; every
//! consumer reaches it through `fm-api`.

pub mod charsets;
pub mod config;
pub mod error;
pub mod font_file;
pub mod grouping;
pub mod index;
pub mod logging;
pub mod parser;
pub mod scanner;
pub mod settings;
pub mod startup;
pub mod telemetry;
#[cfg(test)]
mod test_support;

pub use config::{Config, LogLevel};
pub use error::{FmError, FmResult};
pub use index::Index;
pub use settings::SettingsStore;
pub use telemetry::{CrashReport, Report, Telemetry, TelemetrySettings, TelemetrySink};
