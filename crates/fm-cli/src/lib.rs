//! The `fm` command line: the clap command tree, the output (`--json` or
//! human text) and the exit codes. A thin adapter over `fm-api` (rule 1):
//! every command is one or a few [`Api`] calls and prints their answer, so
//! the same commands run on [`fm_api::mock::MockApi`] in the tests.

mod commands;
mod exit;
mod output;

use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use fm_api::Api;
use fm_api::library::{FontMatch, ResolveFontRequest};
use fm_api::search::Supports;

pub use commands::{Scanned, SearchFlags};
pub use exit::{EXIT_CODES, Exit};
pub use output::Output;

/// Font Manager on the command line.
#[derive(Debug, Parser)]
#[command(name = "fm", version, about, after_help = EXIT_CODES)]
pub struct Cli {
    /// Print one JSON value (the API's answer, or the error) and nothing else.
    #[arg(long, global = true)]
    pub json: bool,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Print the app and API versions.
    Version,
    /// Show which face a font argument names: a face id, a PostScript name,
    /// a full name, or a family with or without its style.
    Resolve {
        /// A face id or a font name.
        font: String,
    },
    /// Add a folder to the library and scan it, or scan every folder.
    /// Progress goes to stderr; the counts of each scan to stdout.
    Scan {
        /// A folder to add (when it is not in the library yet) and scan.
        path: Option<PathBuf>,
    },
    /// List the families, or the faces of one family.
    List {
        /// List the faces of this family (a family name or any font argument).
        #[arg(long, value_name = "FONT")]
        family: Option<String>,
    },
    /// Search the library; prints the families that match.
    Search {
        /// Words to look for; empty lists everything that passes the filters.
        #[arg(default_value = "")]
        query: String,
        /// Only fonts that cover every one of these languages; the
        /// characters each family misses are printed.
        #[arg(long, value_name = "LANGUAGES", value_delimiter = ',')]
        supports: Vec<String>,
        /// Only fonts that have this OpenType feature (repeat or comma list).
        #[arg(long, value_name = "TAG", value_delimiter = ',')]
        feature: Vec<String>,
        /// Only variable fonts.
        #[arg(long)]
        variable: bool,
    },
    /// Show a face: names, styles, axes, features, scripts and license.
    Info {
        /// A face id or a font name.
        font: String,
    },
    /// Check that a face covers a text or languages; exits 5 when a
    /// character is missing.
    Coverage {
        /// A face id or a font name.
        font: String,
        /// The text to cover.
        #[arg(
            long,
            required_unless_present = "supports",
            conflicts_with = "supports"
        )]
        text: Option<String>,
        /// The languages to cover (comma list of codes, `sr-Cyrl,pl`).
        #[arg(long, value_name = "LANGUAGES", value_delimiter = ',')]
        supports: Vec<String>,
    },
    /// List the faces that look most like a face.
    Similar {
        /// A face id or a font name.
        font: String,
        /// How many faces to list.
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },
}

/// Runs `cli` on `api`, printing to `out` and errors to `err`; the exit code.
pub fn run(api: &dyn Api, cli: &Cli, out: &mut dyn Write, err: &mut dyn Write) -> Exit {
    let mut output = Output::new(cli.json, out, err);
    match &cli.command {
        Command::Version => output.print(api.app_info(), output::version),
        Command::Resolve { font } => output.print(resolve(api, font), output::font),
        Command::Scan { path } => {
            let scanned = commands::scan(api, path.as_deref(), output.progress());
            output.print(scanned, |s, o| output::scanned(s, o))
        }
        Command::List { family: None } => {
            output.print(commands::families(api), |f, o| output::families(f, o))
        }
        Command::List { family: Some(font) } => {
            output.print(commands::faces(api, font), |f, o| output::face_rows(f, o))
        }
        Command::Search {
            query,
            supports,
            feature,
            variable,
        } => {
            let flags = SearchFlags {
                supports: supports.clone(),
                features: feature.clone(),
                variable: *variable,
            };
            output.print(commands::search(api, query, &flags), |h, o| {
                output::hits(h, o)
            })
        }
        Command::Info { font } => output.print(commands::info(api, font), output::info),
        Command::Coverage {
            font,
            text,
            supports,
        } => {
            let target = match text {
                Some(text) => Supports::Text { text: text.clone() },
                None => Supports::Languages {
                    codes: supports.clone(),
                },
            };
            output.check(
                commands::coverage(api, font, target),
                output::coverage,
                |r| r.complete,
            )
        }
        Command::Similar { font, limit } => output
            .print(commands::similar(api, font, *limit), |s, o| {
                output::similar(s, o)
            }),
    }
}

/// The one face a `<font>` argument names; an ambiguous name is
/// `InvalidRequest` with the candidates (exit 2).
pub fn resolve(api: &dyn Api, font: &str) -> Result<FontMatch, output::Failure> {
    let matches = api.resolve_font(ResolveFontRequest { font: font.into() })?;
    let candidates = matches.faces.clone();
    matches
        .single(font)
        .map_err(|error| output::Failure::Ambiguous { error, candidates })
}
