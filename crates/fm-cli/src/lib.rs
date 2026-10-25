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
use fm_api::export::{SpecimenFormat, WebFormat};
use fm_api::library::{FontMatch, ResolveFontRequest};
use fm_api::search::Supports;
use fm_api::{Api, ApiError, ErrorCode};

pub use commands::{Doctor, PreviewFlags, PreviewOut, Previewed, Scanned, SearchFlags, TargetKind};
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
    /// Activate a font (a family name names every style), a collection or
    /// a set; one line per face. Exits 4 when any face was not activated.
    /// Needs the app running.
    Activate {
        /// A font argument, or with --collection or --set an id or a name.
        target: String,
        /// Keep the fonts active after a restart (per user).
        #[arg(long)]
        permanent: bool,
        #[command(flatten)]
        kind: KindFlags,
    },
    /// Deactivate a font, a collection or a set. Only fonts the app
    /// activated are deactivated; the others are reported as skipped (exit
    /// 4). Needs the app running.
    Deactivate {
        /// A font argument, or with --collection or --set an id or a name.
        target: String,
        #[command(flatten)]
        kind: KindFlags,
    },
    /// Check the library: duplicates, broken files and activation
    /// conflicts, with the files involved. Changes nothing. Exits 5 when it
    /// finds anything, so a script can gate on it.
    Doctor,
    /// Export or import project sets.
    Sets {
        /// What to do with sets.
        #[command(subcommand)]
        command: SetsCommand,
    },
    /// Write a family's faces as web fonts to a folder; prints the files
    /// written. Exits 4 when a face's license does not allow it.
    Export {
        /// The family (a family name or any font argument).
        family: String,
        /// Write WOFF 2 files.
        #[arg(long, required_unless_present = "woff")]
        woff2: bool,
        /// Write WOFF 1 files.
        #[arg(long)]
        woff: bool,
        /// Also write fonts.css with one @font-face per face.
        #[arg(long)]
        css: bool,
        /// The folder to write to; it must exist.
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        /// Replace files of the same name in the folder.
        #[arg(long)]
        force: bool,
    },
    /// Write a family's specimen to a file.
    Specimen {
        /// The family (a family name or any font argument).
        family: String,
        /// An HTML page with the fonts embedded (the default).
        #[arg(long, conflicts_with = "pdf")]
        html: bool,
        /// A PDF; needs the app running.
        #[arg(long)]
        pdf: bool,
        /// The file to write.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
        /// Replace the file when it exists.
        #[arg(long)]
        force: bool,
    },
    /// Render a line of text in a face as a PNG.
    Preview {
        /// A face id or a font name.
        font: String,
        /// The text to render.
        #[arg(long)]
        text: String,
        /// Size in pixels (8–300).
        #[arg(long, default_value_t = 48.0)]
        size: f32,
        /// Text color, CSS hex (`#1a1a1a`); black by default.
        #[arg(long, value_name = "COLOR")]
        fg: Option<String>,
        /// Background color, CSS hex; transparent by default.
        #[arg(long, value_name = "COLOR")]
        bg: Option<String>,
        /// The PNG file to write, or `-` for stdout.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,
        /// Replace the file when it exists.
        #[arg(long)]
        force: bool,
    },
}

/// What the target of `fm activate` and `fm deactivate` names: a font
/// unless a flag says otherwise.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct KindFlags {
    /// The target is a collection.
    #[arg(long, conflicts_with = "set")]
    pub collection: bool,
    /// The target is a project set.
    #[arg(long)]
    pub set: bool,
}

impl KindFlags {
    /// The kind of target the flags ask for.
    pub fn kind(self) -> TargetKind {
        match (self.collection, self.set) {
            (true, _) => TargetKind::Collection,
            (_, true) => TargetKind::Set,
            _ => TargetKind::Font,
        }
    }
}

/// The `fm sets` commands.
#[derive(Debug, Subcommand)]
pub enum SetsCommand {
    /// Print a set as portable JSON (the fonts by content hash), or write it
    /// to a file.
    Export {
        /// The set: its id or its name.
        set: String,
        /// Write the JSON to this file instead of stdout.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
    /// Import a set exported by `fm sets export`; prints the fonts found in
    /// the library and the fonts missing from it.
    Import {
        /// The JSON file.
        file: PathBuf,
    },
}

/// How `fm` reaches the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// Through the index directly, with the app closed: a session
    /// activation would end with the `fm` process, so activation refuses.
    Direct,
    /// Through the running app's endpoint.
    Endpoint,
}

/// Runs `cli` on `api`, reached over `link`, printing to `out` and errors
/// to `err`; the exit code.
pub fn run(api: &dyn Api, link: Link, cli: &Cli, out: &mut dyn Write, err: &mut dyn Write) -> Exit {
    let mut output = Output::new(cli.json, out, err);
    match &cli.command {
        Command::Version => output.print(api.app_info(), output::version),
        Command::Resolve { font } => output.print(resolve(api, font), output::font),
        Command::Scan { path } => {
            let scanned = commands::scan(api, path.as_deref(), output.progress());
            output.check(scanned, |s, o| output::scanned(s, o), |s| Scanned::exit(s))
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
                |r| {
                    if r.complete {
                        Exit::Success
                    } else {
                        Exit::Problems
                    }
                },
            )
        }
        Command::Similar { font, limit } => output
            .print(commands::similar(api, font, *limit), |s, o| {
                output::similar(s, o)
            }),
        Command::Activate { .. } | Command::Deactivate { .. } if link == Link::Direct => {
            let refused: Result<(), _> = Err(commands::needs_the_app());
            output.print(refused, |(), _| Ok(()))
        }
        Command::Activate {
            target,
            permanent,
            kind,
        } => output.check(
            commands::activate(api, kind.kind(), target, *permanent),
            |r, o| output::outcomes(api, r, o),
            |r| commands::activation_exit(r),
        ),
        Command::Deactivate { target, kind } => output.check(
            commands::deactivate(api, kind.kind(), target),
            |r, o| output::outcomes(api, r, o),
            |r| commands::activation_exit(r),
        ),
        Command::Doctor => output.check(
            commands::doctor(api),
            |d, o| output::doctor(api, d, o),
            Doctor::exit,
        ),
        Command::Sets {
            command: SetsCommand::Export { set, out },
        } => {
            let exported = commands::export_set(api, set, out.as_deref());
            match out {
                Some(path) => output.print(exported, |e, o| output::set_written(e, path, o)),
                None => output.print(exported, output::set_export),
            }
        }
        Command::Sets {
            command: SetsCommand::Import { file },
        } => output.print(commands::import_set(api, file), |r, o| {
            output::imported(api, r, o)
        }),
        Command::Export {
            family,
            woff2,
            woff,
            css,
            out,
            force,
        } => {
            let formats = [(*woff2, WebFormat::Woff2), (*woff, WebFormat::Woff)]
                .into_iter()
                .filter_map(|(on, format)| on.then_some(format))
                .collect();
            output.print(
                commands::export_web(api, family, formats, *css, out, *force),
                output::exported,
            )
        }
        Command::Specimen {
            family,
            pdf,
            out,
            force,
            ..
        } => {
            let format = if *pdf {
                SpecimenFormat::Pdf
            } else {
                SpecimenFormat::Html
            };
            output.print(
                commands::specimen(api, family, format, out, *force),
                output::specimen,
            )
        }
        Command::Preview {
            font,
            text,
            size,
            fg,
            bg,
            out,
            force,
        } => {
            let out = if out.as_os_str() == "-" {
                PreviewOut::Stdout
            } else {
                PreviewOut::File(out.clone())
            };
            if cli.json && out == PreviewOut::Stdout {
                let refused: Result<(), _> = Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    "--out - writes the PNG to stdout and cannot be used with --json. Name a file.",
                ));
                return output.print(refused, |(), _| Ok(()));
            }
            let flags = PreviewFlags {
                text: text.clone(),
                size: *size,
                fg: fg.clone(),
                bg: bg.clone(),
            };
            output.print(
                commands::preview(api, font, &flags, &out, *force),
                output::previewed,
            )
        }
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
