//! What `fm` prints. With `--json`: the API's answer as one JSON value
//! (camelCase, as `fm-api` serializes it) on stdout, or the whole error as
//! JSON on stderr, and nothing else. Without: human text, tables through
//! comfy-table, and the error's message on stderr.

use std::io::{self, Write};

use comfy_table::{Table, presets};
use fm_api::ApiError;
use fm_api::common::Health;
use fm_api::library::{AppInfo, FaceDetail, FaceSummary, FamilySummary, FontMatch};
use fm_api::search::{CoverageReport, FamilyHit, SimilarFace};
use serde::Serialize;

use crate::Exit;
use crate::commands::Scanned;

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
        human: impl FnOnce(&T, &mut dyn Write) -> io::Result<()>,
    ) -> Exit {
        self.check(answer, human, |_| Exit::Success)
    }

    /// As [`Output::print`] for an answer that can report a failure of its
    /// own (a check that found problems, a scan of one folder that failed):
    /// it is printed the same way and exits with what `exit` says of it.
    pub fn check<T: Serialize, E: Into<Failure>>(
        self,
        answer: Result<T, E>,
        human: impl FnOnce(&T, &mut dyn Write) -> io::Result<()>,
        exit: impl FnOnce(&T) -> Exit,
    ) -> Exit {
        let written = match answer.map_err(Into::into) {
            Ok(value) => {
                let exit = exit(&value);
                self.answer(&value, human).map(|()| exit)
            }
            Err(failure) => {
                let exit = Exit::of(failure.error().code);
                self.failure(&failure).map(|()| exit)
            }
        };
        written.unwrap_or(Exit::Unexpected)
    }

    /// Where a long command reports progress: stderr, or nowhere with
    /// `--json` (stderr then carries only an error).
    pub fn progress(&mut self) -> Option<&mut dyn Write> {
        if self.json {
            None
        } else {
            Some(&mut *self.err)
        }
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
        human: impl FnOnce(&T, &mut dyn Write) -> io::Result<()>,
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

/// `fm scan`: per folder, what its scan found.
pub fn scanned(scans: &[Scanned], out: &mut dyn Write) -> io::Result<()> {
    if scans.is_empty() {
        return writeln!(out, "No folder to scan. Add one with fm scan <folder>.");
    }
    let failed = scans.iter().any(|s| s.error.is_some());
    let mut header = vec!["FOLDER", "ADDED", "CHANGED", "REMOVED", "PROBLEMS"];
    if failed {
        header.push("ERROR");
    }
    let mut table = Table::new();
    table.load_preset(presets::NOTHING).set_header(header);
    for s in scans {
        let mut row = match &s.error {
            Some(_) => vec![
                s.source.path.clone(),
                "-".into(),
                "-".into(),
                "-".into(),
                "-".into(),
            ],
            None => vec![
                s.source.path.clone(),
                s.added.to_string(),
                s.changed.to_string(),
                s.removed.to_string(),
                s.problems.to_string(),
            ],
        };
        if failed {
            row.push(
                s.error
                    .as_ref()
                    .map_or_else(String::new, |e| e.message.clone()),
            );
        }
        table.add_row(row);
    }
    writeln!(out, "{table}")
}

/// `fm list`: the families.
pub fn families(families: &[FamilySummary], out: &mut dyn Write) -> io::Result<()> {
    let mut table = Table::new();
    table
        .load_preset(presets::NOTHING)
        .set_header(["ID", "FAMILY", "STYLES", "VARIABLE", "FOUNDRY"]);
    for f in families {
        table.add_row([
            f.id.to_string(),
            f.name.clone(),
            f.style_count.to_string(),
            yes(f.is_variable).into(),
            f.foundry.clone().unwrap_or_default(),
        ]);
    }
    writeln!(out, "{table}")?;
    writeln!(out, "{} families", families.len())
}

/// `fm list --family`: the faces.
pub fn face_rows(faces: &[FaceSummary], out: &mut dyn Write) -> io::Result<()> {
    let mut table = Table::new();
    table.load_preset(presets::NOTHING).set_header([
        "ID",
        "POSTSCRIPT NAME",
        "FAMILY",
        "STYLE",
        "WEIGHT",
        "VARIABLE",
    ]);
    for f in faces {
        table.add_row([
            f.id.to_string(),
            f.postscript_name.clone(),
            f.family_name.clone(),
            f.style.clone(),
            f.weight.to_string(),
            yes(f.is_variable).into(),
        ]);
    }
    writeln!(out, "{table}")
}

/// `fm search`: the family hits; with `--supports`, what each misses.
pub fn hits(hits: &[FamilyHit], out: &mut dyn Write) -> io::Result<()> {
    let coverage = hits.iter().any(|h| h.missing.is_some());
    let mut header = vec!["ID", "FAMILY", "STYLES", "MATCHING FACES"];
    if coverage {
        header.push("MISSING");
    }
    let mut table = Table::new();
    table.load_preset(presets::NOTHING).set_header(header);
    for h in hits {
        let mut row = vec![
            h.family.id.to_string(),
            h.family.name.clone(),
            h.family.style_count.to_string(),
            h.faces.len().to_string(),
        ];
        if coverage {
            row.push(
                h.missing
                    .as_ref()
                    .map_or_else(String::new, |m| m.chars.join(" ")),
            );
        }
        table.add_row(row);
    }
    writeln!(out, "{table}")?;
    writeln!(out, "{} families", hits.len())
}

/// `fm info`: names, styles, axes, features, scripts and license bits.
pub fn info(d: &FaceDetail, out: &mut dyn Write) -> io::Result<()> {
    let f = &d.face;
    let text = |v: &Option<String>| v.clone().unwrap_or_default();
    let axes: Vec<String> = d
        .axes
        .iter()
        .map(|a| format!("{} {}–{} (default {})", a.tag, a.min, a.max, a.default))
        .collect();
    let features: Vec<&str> = d.features.iter().map(|f| f.tag.as_str()).collect();
    let l = &d.license;
    let fields = [
        ("ID", f.id.to_string()),
        ("Family", f.family_name.clone()),
        ("Style", f.style.clone()),
        ("PostScript name", f.postscript_name.clone()),
        ("Legacy family", text(&d.legacy_family)),
        ("Legacy style", text(&d.legacy_style)),
        ("Version", text(&d.version)),
        ("Designer", text(&d.designer)),
        ("Foundry", text(&d.foundry)),
        ("Weight", f.weight.to_string()),
        ("Width", f.width.to_string()),
        ("Slant", json_name(&f.slant)),
        ("Format", json_name(&f.format)),
        ("Variable", yes(f.is_variable).into()),
        (
            "Health",
            match &f.health {
                Health::Ok => "ok".into(),
                Health::Broken(why) => format!("broken: {why}"),
                Health::Unsupported(why) => format!("unsupported: {why}"),
            },
        ),
        ("File", d.path.clone()),
        ("Axes", axes.join(", ")),
        ("Features", features.join(" ")),
        ("Scripts", d.scripts.join(" ")),
        (
            "Open license",
            l.open_license.as_ref().map(json_name).unwrap_or_default(),
        ),
        ("Embedding", json_name(&l.embedding)),
        (
            "Declared license",
            l.declared.as_ref().map(json_name).unwrap_or_default(),
        ),
        ("License", text(&l.description)),
        ("License URL", text(&l.url)),
    ];
    for (label, value) in fields {
        if !value.is_empty() {
            writeln!(out, "{label:<18}{value}")?;
        }
    }
    Ok(())
}

/// `fm coverage`: covered, or the characters missing, overall and per
/// language.
pub fn coverage(report: &CoverageReport, out: &mut dyn Write) -> io::Result<()> {
    if report.complete {
        writeln!(out, "Every character is covered.")?;
    } else {
        writeln!(
            out,
            "{} characters are missing: {}",
            report.missing.len(),
            report.missing.join(" ")
        )?;
    }
    for l in &report.languages {
        write!(out, "{}: {} of {}", l.code, l.covered, l.total)?;
        if !l.missing.is_empty() {
            write!(out, ", missing {}", l.missing.join(" "))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// `fm similar`: the faces, closest first, with the reasons.
pub fn similar(faces: &[SimilarFace], out: &mut dyn Write) -> io::Result<()> {
    if faces.is_empty() {
        return writeln!(out, "No similar fonts found.");
    }
    let mut table = Table::new();
    table.load_preset(presets::NOTHING).set_header([
        "SCORE",
        "ID",
        "POSTSCRIPT NAME",
        "FAMILY",
        "STYLE",
        "REASONS",
    ]);
    for s in faces {
        table.add_row([
            format!("{:.2}", s.score),
            s.face.id.to_string(),
            s.face.postscript_name.clone(),
            s.face.family_name.clone(),
            s.face.style.clone(),
            s.reasons.join(", "),
        ]);
    }
    writeln!(out, "{table}")
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// An enum value as the API names it in JSON (`italic`, `ofl`).
fn json_name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}
