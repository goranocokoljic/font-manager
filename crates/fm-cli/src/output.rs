//! What `fm` prints. With `--json`: the API's answer as one JSON value
//! (camelCase, as `fm-api` serializes it) on stdout, or the whole error as
//! JSON on stderr, and nothing else. Without: human text, tables through
//! comfy-table, and the error's message on stderr.

use std::io::{self, Write};

use comfy_table::{Table, presets};
use fm_api::ApiError;
use fm_api::library::{AppInfo, FontMatch};
use serde::Serialize;

use crate::Exit;

/// Why a command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// An API call failed.
    Api(ApiError),
    /// A font argument names several faces: the error and the faces.
    Ambiguous {
        /// `InvalidRequest`, with the candidates in its detail.
        error: ApiError,
        /// The faces it names.
        candidates: Vec<FontMatch>,
    },
}

impl From<ApiError> for Failure {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

impl Failure {
    fn error(&self) -> &ApiError {
        match self {
            Self::Api(error) | Self::Ambiguous { error, .. } => error,
        }
    }
}

/// Where and how a command prints.
pub struct Output<'a> {
    json: bool,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl std::fmt::Debug for Output<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output").field("json", &self.json).finish()
    }
}

impl<'a> Output<'a> {
    /// JSON when `json`, human text otherwise.
    pub fn new(json: bool, out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Self { json, out, err }
    }

    /// Prints a command's answer (`human` writes the text form) or its
    /// failure; the exit code. Output that cannot be written exits 1.
    pub fn print<T: Serialize, E: Into<Failure>>(
        self,
        answer: Result<T, E>,
        human: fn(&T, &mut dyn Write) -> io::Result<()>,
    ) -> Exit {
        let written = match answer.map_err(Into::into) {
            Ok(value) => self.answer(&value, human).map(|()| Exit::Success),
            Err(failure) => {
                let exit = Exit::of(failure.error().code);
                self.failure(&failure).map(|()| exit)
            }
        };
        written.unwrap_or(Exit::Unexpected)
    }

    /// Prints a usage error clap found (`fm` with a bad argument); with
    /// `--json` as an `InvalidRequest` error, with clap's text as detail.
    pub fn usage(self, clap_text: &str) -> Exit {
        let failure = Failure::Api(
            ApiError::new(
                fm_api::ErrorCode::InvalidRequest,
                "The command line is not valid. See fm --help.",
            )
            .with_detail(clap_text.trim_end()),
        );
        let written = if self.json {
            self.failure(&failure)
        } else {
            write!(self.err, "{clap_text}")
        };
        written.map(|()| Exit::Usage).unwrap_or(Exit::Unexpected)
    }

    fn answer<T: Serialize>(
        self,
        value: &T,
        human: fn(&T, &mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.json {
            serde_json::to_writer(&mut *self.out, value)?;
            writeln!(self.out)
        } else {
            human(value, self.out)
        }
    }

    fn failure(self, failure: &Failure) -> io::Result<()> {
        if self.json {
            serde_json::to_writer(&mut *self.err, failure.error())?;
            return writeln!(self.err);
        }
        writeln!(self.err, "fm: {}", failure.error().message)?;
        if let Failure::Ambiguous { candidates, .. } = failure {
            writeln!(self.err, "{}", faces(candidates))?;
        }
        Ok(())
    }
}

/// `fm version`: the app and the API it serves.
pub fn version(info: &AppInfo, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "{} {}", info.name, info.version)?;
    writeln!(out, "API {}", info.api_version)
}

/// `fm resolve`: the face, as one row.
pub fn font(face: &FontMatch, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "{}", faces(std::slice::from_ref(face)))
}

/// Faces as a table: id, PostScript name, family, style, full name.
fn faces(faces: &[FontMatch]) -> Table {
    let mut table = Table::new();
    table.load_preset(presets::NOTHING).set_header([
        "ID",
        "POSTSCRIPT NAME",
        "FAMILY",
        "STYLE",
        "FULL NAME",
    ]);
    for f in faces {
        table.add_row([
            f.face.to_string(),
            f.postscript_name.clone(),
            f.family.clone(),
            f.style.clone(),
            f.full_name.clone().unwrap_or_default(),
        ]);
    }
    table
}
