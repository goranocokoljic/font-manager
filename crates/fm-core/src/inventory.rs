//! The license inventory (E15.4): one CSV row per family with what the
//! library knows about its license, for a spreadsheet.
//!
//! The rows come from the index ([`crate::index::Index::license_inventory`]);
//! this module writes them as CSV. Cells follow RFC 4180: a cell with a
//! comma, a quote or a line break is quoted, its quotes doubled; lines end in
//! CRLF. A cell a spreadsheet would read as a formula (one starting with `=`,
//! `+`, `-`, `@`, a tab or a carriage return) gets a leading `'`, so opening
//! the file never runs it (CSV injection).
//!
//! The CSV is returned as text, never written here: the shell saves it to a
//! file the user picks ([`save`]).

use std::borrow::Cow;
use std::path::Path;

use fm_api::common::{Embedding, FontFormat};
use fm_api::export::LicenseInventory;
use fm_api::library::{LicenseType, OpenLicense};

use crate::error::{FmError, FmResult};

/// The header row, in column order.
pub const HEADER: [&str; 12] = [
    "Family",
    "Foundry",
    "Styles",
    "Formats",
    "Declared license",
    "Seats",
    "Purchase date",
    "Receipt",
    "Open license",
    "Embedding",
    "License URL",
    "Source folder",
];

/// What one family's row says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryRow {
    /// Family name.
    pub family: String,
    /// Foundry or manufacturer.
    pub foundry: Option<String>,
    /// Number of styles, named instances included.
    pub styles: u32,
    /// Formats present, in the family's order.
    pub formats: Vec<FontFormat>,
    /// The license type declared for the family (or, before E15.2, a face).
    pub declared: Option<LicenseType>,
    /// Seats declared.
    pub seats: Option<u32>,
    /// Purchase date declared, `YYYY-MM-DD`.
    pub purchase_date: Option<String>,
    /// The receipt copy the app keeps.
    pub receipt: Option<String>,
    /// The open license a face's name table states.
    pub open_license: Option<OpenLicense>,
    /// The embedding permissions of its faces, each once, most permissive first.
    pub embedding: Vec<Embedding>,
    /// The license URL (name ID 14) of the first face that has one.
    pub license_url: Option<String>,
    /// The source folder the family was found in.
    pub source_folder: Option<String>,
}

/// The inventory of `rows` as CSV with a header row; `rows` is the number
/// of families written.
pub fn to_csv(rows: &[InventoryRow]) -> LicenseInventory {
    let mut csv = String::new();
    push_line(&mut csv, HEADER.iter().map(|h| Cow::Borrowed(*h)));
    for row in rows {
        push_line(&mut csv, cells(row).into_iter());
    }
    LicenseInventory {
        csv,
        rows: u32::try_from(rows.len()).unwrap_or(u32::MAX),
    }
}

/// The cells of one row, in [`HEADER`] order.
fn cells(row: &InventoryRow) -> [Cow<'_, str>; 12] {
    [
        Cow::Borrowed(row.family.as_str()),
        text(&row.foundry),
        Cow::Owned(row.styles.to_string()),
        Cow::Owned(join(row.formats.iter().map(|f| format_label(*f)))),
        Cow::Borrowed(row.declared.map_or("", license_type_label)),
        Cow::Owned(row.seats.map(|s| s.to_string()).unwrap_or_default()),
        text(&row.purchase_date),
        text(&row.receipt),
        Cow::Borrowed(row.open_license.map_or("", open_license_label)),
        Cow::Owned(join(row.embedding.iter().map(|e| embedding_label(*e)))),
        text(&row.license_url),
        text(&row.source_folder),
    ]
}

fn text(value: &Option<String>) -> Cow<'_, str> {
    Cow::Borrowed(value.as_deref().unwrap_or_default())
}

fn join<'a>(labels: impl Iterator<Item = &'a str>) -> String {
    labels.collect::<Vec<_>>().join("; ")
}

/// Appends one CSV line (CRLF, RFC 4180) of `cells`.
fn push_line<'a>(csv: &mut String, cells: impl Iterator<Item = Cow<'a, str>>) {
    for (i, value) in cells.enumerate() {
        if i > 0 {
            csv.push(',');
        }
        csv.push_str(&cell(&value));
    }
    csv.push_str("\r\n");
}

/// One cell: defused when a spreadsheet would read it as a formula, then
/// quoted when it holds a comma, a quote or a line break.
pub fn cell(value: &str) -> Cow<'_, str> {
    let value: Cow<'_, str> = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        Cow::Owned(format!("'{value}"))
    } else {
        Cow::Borrowed(value)
    };
    if value.contains([',', '"', '\n', '\r']) {
        Cow::Owned(format!("\"{}\"", value.replace('"', "\"\"")))
    } else {
        value
    }
}

/// Writes `csv` to `path`, the file the user picked, replacing it. A file
/// that cannot be written is an error with the path in its detail.
pub fn save(path: &Path, csv: &str) -> FmResult<()> {
    std::fs::write(path, csv)
        .map_err(|e| FmError::io("The license inventory could not be saved there.", path, &e))
}

fn format_label(f: FontFormat) -> &'static str {
    match f {
        FontFormat::Ttf => "TTF",
        FontFormat::Otf => "OTF",
        FontFormat::Ttc => "TTC",
        FontFormat::Otc => "OTC",
        FontFormat::Woff => "WOFF",
        FontFormat::Woff2 => "WOFF2",
        FontFormat::Type1 => "Type 1",
        FontFormat::Dfont => "dfont",
        FontFormat::Bitmap => "Bitmap",
    }
}

fn license_type_label(l: LicenseType) -> &'static str {
    match l {
        LicenseType::Desktop => "Desktop",
        LicenseType::Web => "Web",
        LicenseType::App => "App",
        LicenseType::Trial => "Trial",
        LicenseType::PersonalOnly => "Personal use only",
        LicenseType::Unknown => "Unknown",
    }
}

/// The name of an open license as the user reads it.
pub(crate) fn open_license_label(l: OpenLicense) -> &'static str {
    match l {
        OpenLicense::Ofl => "SIL Open Font License",
        OpenLicense::Apache => "Apache License 2.0",
        OpenLicense::Ufl => "Ubuntu Font Licence",
        OpenLicense::Mit => "MIT",
    }
}

fn embedding_label(e: Embedding) -> &'static str {
    match e {
        Embedding::Installable => "Installable",
        Embedding::Editable => "Editable",
        Embedding::PreviewAndPrint => "Preview and print",
        Embedding::Restricted => "Restricted",
    }
}

#[cfg(test)]
pub(crate) mod tests;
