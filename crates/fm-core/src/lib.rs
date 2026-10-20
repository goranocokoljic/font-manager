//! Font Manager core.
//!
//! All logic lives here: scanning, parsing (fontations), the SQLite/FTS5 index,
//! search and filters, coverage, previews, activation, export and catalog
//! adapters. Nothing outside this crate computes anything about fonts; every
//! consumer reaches it through `fm-api`.

pub mod activation;
pub mod api;
pub mod blocks;
pub mod charsets;
pub mod classify;
pub mod config;
pub mod coverage;
pub mod crash;
pub mod error;
pub mod features;
mod filter;
pub mod font_file;
pub mod glyphs;
pub mod grouping;
pub mod index;
pub mod lifecycle;
pub mod logging;
mod parallel;
pub mod parser;
pub mod preview;
pub mod resolve;
pub mod sample;
pub mod scanner;
mod search;
pub mod settings;
pub mod sidecar;
pub mod startup;
pub mod telemetry;
#[cfg(test)]
mod test_support;

pub use activation::ActivationManager;
pub use api::CoreApi;
pub use config::{Config, LogLevel};
pub use error::{FmError, FmResult};
pub use index::Index;
pub use resolve::DuplicateResolver;
pub use settings::SettingsStore;
pub use telemetry::{CrashReport, Report, Telemetry, TelemetrySettings, TelemetrySink};
