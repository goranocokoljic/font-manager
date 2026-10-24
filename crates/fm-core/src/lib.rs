//! Font Manager core.
//!
//! All logic lives here: scanning, parsing (fontations), the SQLite/FTS5 index,
//! search and filters, coverage, previews, activation, export and catalog
//! adapters. Nothing outside this crate computes anything about fonts; every
//! consumer reaches it through `fm-api`.

pub mod activation;
pub mod api;
pub mod blocks;
pub mod catalog;
pub mod charsets;
pub mod classify;
pub mod config;
pub mod coverage;
pub mod crash;
pub mod css;
pub mod error;
pub mod features;
mod filter;
pub mod font_file;
pub mod framework_snippet;
pub mod glyph_features;
pub mod glyph_svg;
pub mod glyphs;
pub mod grouping;
pub mod index;
pub mod inventory;
pub mod layout;
pub mod license;
pub mod lifecycle;
pub mod logging;
pub mod metrics;
pub mod palettes;
pub mod pangrams;
mod parallel;
pub mod parser;
pub mod preview;
pub mod receipt;
pub mod resolve;
pub mod sample;
pub mod scanner;
mod search;
pub mod settings;
pub mod sidecar;
pub mod specimen;
pub mod startup;
pub mod subset;
pub mod telemetry;
#[cfg(test)]
mod test_support;
pub mod update;
pub mod web_css;
pub mod web_font;

pub use activation::ActivationManager;
pub use api::CoreApi;
pub use config::{Config, LogLevel};
pub use error::{FmError, FmResult};
pub use index::Index;
pub use resolve::DuplicateResolver;
pub use settings::SettingsStore;
pub use telemetry::{CrashReport, Report, Telemetry, TelemetrySettings, TelemetrySink};
