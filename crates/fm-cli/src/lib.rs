//! The `fm` command line: the clap command tree, the output (`--json` or
//! human text) and the exit codes. A thin adapter over `fm-api` (rule 1):
//! every command is one or a few [`Api`] calls and prints their answer, so
//! the same commands run on [`fm_api::mock::MockApi`] in the tests.

mod exit;
mod output;

use std::io::Write;

use clap::{Parser, Subcommand};
use fm_api::Api;
use fm_api::library::{FontMatch, ResolveFontRequest};

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
}

/// Runs `cli` on `api`, printing to `out` and errors to `err`; the exit code.
pub fn run(api: &dyn Api, cli: &Cli, out: &mut dyn Write, err: &mut dyn Write) -> Exit {
    let output = Output::new(cli.json, out, err);
    match &cli.command {
        Command::Version => output.print(api.app_info(), output::version),
        Command::Resolve { font } => output.print(resolve(api, font), output::font),
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
