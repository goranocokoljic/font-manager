//! A family's specimen as one self-contained HTML page (E21.1).
//!
//! The page has a header (family, designer, foundry, version, license and
//! web use), every style in its own face (a variable face shows each named
//! instance), a waterfall of the sample text, the character set grouped by
//! Unicode block ([`crate::blocks`]), and each feature a user can switch,
//! off and on.
//!
//! Every face is embedded as a base64 `data:` URI ([`base64`]), so the page
//! loads nothing from anywhere: it has no script and no `http:`, `https:`
//! or `file:` reference. Text from the font is HTML-escaped, `:` included,
//! so a name that holds an address stays text ([`escape`]). The license is
//! summed up, never quoted: its description and URL hold addresses. The
//! CSS names the faces `f0`, `f1`…, so no font name ever enters the CSS.
//! The same family gives the same bytes: nothing depends on the time, the
//! machine or a hash order.
//!
//! A PDF specimen (E21.2) is the same page printed by the OS webview
//! ([`write_pdf`], ADR 0013).
//!
//! A collection's specimen (E21.3) is one document for a client to sign off
//! on, built from the same sections: a cover, then a page per family with
//! its facts, its styles and a short sample ([`collection_page`]).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fm_api::common::{Embedding, FeatureInfo, NamedInstance, Slant};
use fm_api::library::{LicenseType, OpenLicense};
use fm_api::{ErrorCode, FaceId};
use fm_platform::PdfPrinter;

use crate::blocks;
use crate::coverage::{self, CodePointSet};
use crate::error::{FmError, FmResult};
use crate::font_file::{self, FontFile};
use crate::license::{self, WebReason};
use crate::sample;

/// The sizes of the waterfall, in CSS pixels.
pub const WATERFALL_SIZES: [u32; 6] = [12, 16, 24, 36, 48, 72];

/// The sample text, before [`sample::dynamic_sample`] fits it to the face
/// (the layout waterfall's pangram).
const SAMPLE: &str = "The quick brown fox jumps over the lazy dog";

/// What a specimen shows about one family ([`crate::index::Index::specimen`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Specimen {
    /// The family name.
    pub family: String,
    /// Its faces, in the library's order: light to bold, narrow to wide,
    /// upright before italic.
    pub faces: Vec<SpecimenFace>,
}

/// What the specimen of a collection shows (E21.3,
/// [`crate::index::Index::collection_specimen`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionSpecimen {
    /// The collection's name.
    pub name: String,
    /// The specimen of each family with a face in the collection, in the
    /// collection's order.
    pub families: Vec<Specimen>,
}

/// One face of a specimen.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecimenFace {
    /// The face.
    pub id: FaceId,
    /// Family and style, for messages ("Inter Bold").
    pub label: String,
    /// The style name.
    pub style: String,
    /// Weight class.
    pub weight: u16,
    /// Width class.
    pub width: u16,
    /// Upright, italic or oblique.
    pub slant: Slant,
    /// Version string (name ID 5).
    pub version: Option<String>,
    /// Designer (name ID 9).
    pub designer: Option<String>,
    /// Foundry (name ID 8).
    pub foundry: Option<String>,
    /// `OS/2.fsType`.
    pub embedding: Embedding,
    /// The declared license type, the family's first.
    pub declared: Option<LicenseType>,
    /// The open license the name table states.
    pub open_license: Option<OpenLicense>,
    /// Whether the face may be used on the web, and why.
    pub web_use: WebReason,
    /// Named instances of a variable face; empty for a static one.
    pub instances: Vec<NamedInstance>,
    /// GSUB/GPOS features, with friendly names.
    pub features: Vec<FeatureInfo>,
    /// The code points the face maps.
    pub code_points: CodePointSet,
    /// The face's file.
    pub path: PathBuf,
}

/// Refuses a specimen that would embed a face its license keeps out
/// ([`license::specimen_embedding_block`]). The refusal names every such
/// face; the detail lists their ids.
pub fn check_embedding(spec: &Specimen) -> FmResult<()> {
    check_faces(spec.faces.iter())
}

/// [`check_embedding`] over any faces: every family of a collection at once.
fn check_faces<'a>(faces: impl Iterator<Item = &'a SpecimenFace>) -> FmResult<()> {
    let blocked: Vec<&SpecimenFace> = faces
        .filter(|f| license::specimen_embedding_block(f.embedding, f.declared))
        .collect();
    if blocked.is_empty() {
        return Ok(());
    }
    let labels: Vec<&str> = blocked.iter().map(|f| f.label.as_str()).collect();
    let ids: Vec<String> = blocked.iter().map(|f| f.id.0.to_string()).collect();
    Err(FmError::new(
        ErrorCode::PermissionDenied,
        format!(
            "{}: Embedding restricted and no license declared, so the font may not be embedded in a specimen. Declare a license for the family to export it.",
            labels.join(", ")
        ),
    )
    .with_detail(format!("blocked faces: {}", ids.join(", "))))
}

/// Writes the specimen of `spec` as HTML to `out`, the file the user picked,
/// and returns its absolute path. The license check comes first, so a
/// refused specimen reads no font and writes nothing. A face file that
/// cannot be read fails the export ([`font_file::read_font_file`]).
pub fn write_html(spec: &Specimen, out: &Path) -> FmResult<PathBuf> {
    save(&checked_page(spec)?, out)
}

/// Writes the specimen of the collection `c` as HTML to `out` (E21.3), as
/// [`write_html`]: the checks of [`checked_collection_page`] come first.
pub fn write_collection_html(c: &CollectionSpecimen, date: &str, out: &Path) -> FmResult<PathBuf> {
    save(&checked_collection_page(c, date)?, out)
}

/// Prints the specimen of the collection `c` to `out` through `printer`
/// (E21.3), as [`write_pdf`]; a page break comes before each family.
pub fn write_collection_pdf(
    c: &CollectionSpecimen,
    date: &str,
    out: &Path,
    printer: &dyn PdfPrinter,
) -> FmResult<PathBuf> {
    print(&checked_collection_page(c, date)?, out, printer)
}

/// Writes the checked page `page` to `out` and returns its absolute path.
fn save(page: &str, out: &Path) -> FmResult<PathBuf> {
    let out = absolute(out)?;
    std::fs::write(&out, page)
        .map_err(|e| FmError::io("The specimen could not be saved.", &out, &e))?;
    Ok(out)
}

/// Prints the specimen of `spec` as a paginated A4 PDF to `out` through
/// `printer` (E21.2) and returns its absolute path. The checks are
/// [`write_html`]'s. The page goes to a temporary HTML file, the PDF to a
/// hidden partial file next to `out`, renamed over `out` only once the print
/// succeeded and the file is a PDF; both temporary files are removed whatever
/// happens, so a failed print leaves `out` as it was. Blocks until the print
/// is done: call it off the UI thread ([`PdfPrinter::print_html`]).
pub fn write_pdf(spec: &Specimen, out: &Path, printer: &dyn PdfPrinter) -> FmResult<PathBuf> {
    print(&checked_page(spec)?, out, printer)
}

/// Prints the checked page `page` to `out`, as [`write_pdf`] describes.
fn print(page: &str, out: &Path, printer: &dyn PdfPrinter) -> FmResult<PathBuf> {
    let out = absolute(out)?;
    let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let html = TempFile(std::env::temp_dir().join(format!("fm-specimen-{pid}-{n}.html")));
    std::fs::write(&html.0, page)
        .map_err(|e| FmError::io("The specimen could not be prepared.", &html.0, &e))?;
    let name = out.file_name().map_or_else(
        || "specimen.pdf".into(),
        |n| n.to_string_lossy().into_owned(),
    );
    let partial = TempFile(out.with_file_name(format!(".{name}.{pid}-{n}.partial")));
    printer.print_html(&html.0, &partial.0).map_err(|e| {
        FmError::new(ErrorCode::Io, "The PDF specimen could not be printed.")
            .with_detail(e.to_string())
    })?;
    let head = std::fs::read(&partial.0).map(|b| b.starts_with(b"%PDF-"));
    if !matches!(head, Ok(true)) {
        return Err(
            FmError::new(ErrorCode::Io, "The PDF specimen could not be printed.")
                .with_detail(format!("{}: not a PDF file", partial.0.display())),
        );
    }
    std::fs::rename(&partial.0, &out)
        .map_err(|e| FmError::io("The specimen could not be saved.", &out, &e))?;
    Ok(out)
}

/// The page of `spec` after the checks: a family with faces whose licenses
/// allow embedding, every face file readable.
fn checked_page(spec: &Specimen) -> FmResult<String> {
    if spec.faces.is_empty() {
        return Err(FmError::not_found(format!("Family {}", spec.family)));
    }
    check_embedding(spec)?;
    let files = spec
        .faces
        .iter()
        .map(|f| font_file::read_font_file(&f.path))
        .collect::<FmResult<Vec<_>>>()?;
    html(spec, &files)
}

/// The page of the collection `c` after the checks: at least one family
/// (an empty collection is `InvalidRequest`), the licenses of all its faces
/// allow embedding (one refusal names them all, as [`check_embedding`]),
/// every face file readable.
fn checked_collection_page(c: &CollectionSpecimen, date: &str) -> FmResult<String> {
    if c.families.is_empty() {
        return Err(FmError::invalid(format!(
            "The collection {} is empty. Add fonts to it to export a specimen.",
            c.name
        )));
    }
    check_faces(c.families.iter().flat_map(|f| &f.faces))?;
    let files = c
        .families
        .iter()
        .flat_map(|f| &f.faces)
        .map(|f| font_file::read_font_file(&f.path))
        .collect::<FmResult<Vec<_>>>()?;
    collection_html(c, date, &files)
}

fn absolute(out: &Path) -> FmResult<PathBuf> {
    std::path::absolute(out).map_err(|e| FmError::io("The specimen could not be saved.", out, &e))
}

/// Tells the temporary files of concurrent PDF exports apart.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A temporary file, removed on drop when it is still there.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The page, with face `i` embedded from `files[i]`.
pub fn html(spec: &Specimen, files: &[FontFile]) -> FmResult<String> {
    page(spec, |i| {
        files.get(i).map_or_else(String::new, |f| {
            format!("data:{};base64,{}", f.mime, base64(&f.bytes))
        })
    })
}

/// The page, with `src(i)` as the URL of face `i`: a data URI in
/// [`html`], a stub in tests.
pub fn page(spec: &Specimen, src: impl Fn(usize) -> String) -> FmResult<String> {
    let table = blocks::load()?;
    let shown = shown_face(&spec.faces);
    let Some(main) = spec.faces.get(shown) else {
        return Err(FmError::not_found(format!("Family {}", spec.family)));
    };
    let sample = escape(&sample::dynamic_sample(&main.code_points, SAMPLE).text);
    let family = escape(&spec.family);
    let mut h = String::new();
    head(&mut h, &family, spec.faces.len(), &src, "");

    // Header.
    let _ = writeln!(h, "<header>\n<h1 class=\"f{shown}\">{family}</h1>");
    facts(&mut h, spec, main);
    h.push_str("</header>\n");

    // Styles.
    h.push_str("<section>\n<h2>Styles</h2>\n");
    style_rows(&mut h, spec, 0, &sample);
    h.push_str("</section>\n");

    // Waterfall.
    h.push_str("<section>\n<h2>Waterfall</h2>\n");
    size_rows(&mut h, shown, &WATERFALL_SIZES, &sample);
    h.push_str("</section>\n");

    // Character set, grouped by Unicode block.
    h.push_str("<section>\n<h2>Character set</h2>\n");
    let mut block: Option<&str> = None;
    for cp in main.code_points.iter() {
        let Some(c) = char::from_u32(cp).filter(|&c| !coverage::is_ignorable(c)) else {
            continue;
        };
        let name = table.find(cp).map_or("No block", |b| b.name.as_str());
        if block != Some(name) {
            if block.is_some() {
                h.push_str("</p>\n");
            }
            let _ = write!(h, "<h3>{}</h3>\n<p class=\"chars f{shown}\">", escape(name));
            block = Some(name);
        }
        let _ = write!(
            h,
            "<span title=\"U+{cp:04X}\">{}</span>",
            escape(c.encode_utf8(&mut [0; 4]))
        );
    }
    if block.is_some() {
        h.push_str("</p>\n");
    }
    h.push_str("</section>\n");

    // Features a user can switch, each in the first face that has it.
    let mut seen: Vec<&str> = Vec::new();
    let mut features = Vec::new();
    for (i, f) in spec.faces.iter().enumerate() {
        for feat in &f.features {
            if feat.automatic || !css_tag(&feat.tag) || seen.contains(&feat.tag.as_str()) {
                continue;
            }
            seen.push(&feat.tag);
            features.push((i, feat));
        }
    }
    if !features.is_empty() {
        h.push_str("<section>\n<h2>Features</h2>\n");
        for (i, feat) in features {
            let tag = &feat.tag;
            let name = escape(feat.name.as_deref().unwrap_or(tag.as_str()));
            let _ = writeln!(
                h,
                "<h3>{name} <code>{}</code></h3>\n<div class=\"row\"><p class=\"label\">Off</p><p class=\"sample f{i}\" style=\"font-feature-settings: &quot;{tag}&quot; 0\">{sample}</p></div>\n<div class=\"row\"><p class=\"label\">On</p><p class=\"sample f{i}\" style=\"font-feature-settings: &quot;{tag}&quot; 1\">{sample}</p></div>",
                escape(tag)
            );
        }
        h.push_str("</section>\n");
    }
    h.push_str("</body>\n</html>\n");
    Ok(h)
}

/// The sample sizes of a family's page in a collection specimen, in CSS
/// pixels: small enough that a family fits one A4 page.
pub const COLLECTION_SIZES: [u32; 3] = [16, 24, 36];

/// The collection specimen, with face `i` (counted over all families, in
/// order) embedded from `files[i]`.
pub fn collection_html(c: &CollectionSpecimen, date: &str, files: &[FontFile]) -> FmResult<String> {
    collection_page(c, date, |i| {
        files.get(i).map_or_else(String::new, |f| {
            format!("data:{};base64,{}", f.mime, base64(&f.bytes))
        })
    })
}

/// The specimen of a collection (E21.3), built from the sections of
/// [`page`]: a cover with the collection's name, `date` and the number of
/// families, then one page per family in the collection's order with its
/// name, the facts of the header (license and web use among them), every
/// style and the sample at [`COLLECTION_SIZES`]. Printed, a page break
/// comes before each family. `src(i)` is the URL of face `i`, counted over
/// all families, as [`page`]'s.
pub fn collection_page(
    c: &CollectionSpecimen,
    date: &str,
    src: impl Fn(usize) -> String,
) -> FmResult<String> {
    let total = c.families.iter().map(|f| f.faces.len()).sum();
    let name = escape(&c.name);
    let mut h = String::new();
    head(&mut h, &name, total, &src, COLLECTION_STYLE);

    // Cover.
    let count = c.families.len();
    let _ = writeln!(
        h,
        "<header class=\"cover\">\n<h1>{name}</h1>\n<p class=\"label\">Specimen for sign-off</p>\n<dl>\n<dt>Date</dt><dd>{}</dd>\n<dt>Families</dt><dd>{count}</dd>\n</dl>\n</header>",
        escape(date)
    );

    // One page per family.
    let mut base = 0;
    for spec in &c.families {
        let shown = shown_face(&spec.faces);
        let Some(main) = spec.faces.get(shown) else {
            return Err(FmError::not_found(format!("Family {}", spec.family)));
        };
        let sample = escape(&sample::dynamic_sample(&main.code_points, SAMPLE).text);
        let _ = writeln!(
            h,
            "<section class=\"family\">\n<h2 class=\"f{}\">{}</h2>",
            base + shown,
            escape(&spec.family)
        );
        facts(&mut h, spec, main);
        h.push_str("<h3>Styles</h3>\n");
        style_rows(&mut h, spec, base, &sample);
        h.push_str("<h3>Sample</h3>\n");
        size_rows(&mut h, base + shown, &COLLECTION_SIZES, &sample);
        h.push_str("</section>\n");
        base += spec.faces.len();
    }
    h.push_str("</body>\n</html>\n");
    Ok(h)
}

/// The collection specimen's look on top of [`STYLE`]: a cover, and each
/// family on a page of its own.
const COLLECTION_STYLE: &str = ".cover h1 { font-size: 3rem; }
.family { break-before: page; page-break-before: always; border-top: 1px solid #ccc; margin-top: 3rem; padding-top: 1rem; }
.family > h2 { font-size: 2.5rem; font-weight: normal; border: none; margin: 0 0 1rem; }
.family .sample { font-size: 20px; }
@media print { .family { border: none; margin: 0; padding: 0; } }
";

/// The page up to `<body>`: the title, an `@font-face` rule and a class
/// `f{i}` for each of the `faces` faces, [`STYLE`] and then `style`.
fn head(h: &mut String, title: &str, faces: usize, src: &impl Fn(usize) -> String, style: &str) {
    h.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(h, "<title>{title} specimen</title>\n<style>");
    for i in 0..faces {
        let _ = writeln!(
            h,
            "@font-face {{ font-family: \"f{i}\"; src: url(\"{}\"); font-display: block; }}\n.f{i} {{ font-family: \"f{i}\"; }}",
            src(i)
        );
    }
    h.push_str(STYLE);
    h.push_str(style);
    h.push_str("</style>\n</head>\n<body>\n");
}

/// The facts of the header as a list: designer, foundry, version, license
/// and web use. The web use is the family's: the first face not OK for the
/// web decides, else `main`'s.
fn facts(h: &mut String, spec: &Specimen, main: &SpecimenFace) {
    let first = |get: fn(&SpecimenFace) -> &Option<String>| {
        spec.faces
            .iter()
            .find_map(|f| get(f).as_deref().filter(|s| !s.trim().is_empty()))
    };
    let web = spec
        .faces
        .iter()
        .map(|f| f.web_use)
        .find(|w| !w.ok())
        .unwrap_or(main.web_use);
    let rows = [
        ("Designer", first(|f| &f.designer).map(escape)),
        ("Foundry", first(|f| &f.foundry).map(escape)),
        ("Version", main.version.as_deref().map(escape)),
        ("License", Some(escape(&license_line(main)))),
        ("Web use", Some(escape(&web_line(web)))),
    ];
    h.push_str("<dl>\n");
    for (term, value) in rows {
        let value = value.unwrap_or_else(|| "Not stated".to_owned());
        let _ = writeln!(h, "<dt>{term}</dt><dd>{value}</dd>");
    }
    h.push_str("</dl>\n");
}

/// A row per style of `spec` (a variable face: per named instance), its
/// faces classed from `f{base}` on.
fn style_rows(h: &mut String, spec: &Specimen, base: usize, sample: &str) {
    for (i, f) in spec.faces.iter().enumerate() {
        if f.instances.is_empty() {
            style_row(h, base + i, &escape(&f.style), "", sample);
        }
        for inst in &f.instances {
            let settings = variation_settings(inst);
            let style = if settings.is_empty() {
                String::new()
            } else {
                format!(" style=\"font-variation-settings: {settings}\"")
            };
            style_row(h, base + i, &escape(&inst.name), &style, sample);
        }
    }
}

/// A row per size of `sizes`, the sample in face `face`.
fn size_rows(h: &mut String, face: usize, sizes: &[u32], sample: &str) {
    for size in sizes {
        let _ = writeln!(
            h,
            "<div class=\"row\"><p class=\"label\">{size} px</p><p class=\"sample f{face}\" style=\"font-size: {size}px\">{sample}</p></div>"
        );
    }
}

/// Today's date in UTC, `YYYY-MM-DD`: the date on a collection specimen's
/// cover.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    utc_date(i64::try_from(secs).unwrap_or(0))
}

/// The UTC date, `YYYY-MM-DD`, of `secs` seconds after the Unix epoch
/// (the proleptic Gregorian calendar, by days since 0000-03-01).
pub fn utc_date(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The page's own look; no font name and nothing from outside.
const STYLE: &str = "body { margin: 2rem auto; max-width: 64rem; padding: 0 1.5rem; \
font-family: system-ui, sans-serif; color: #1a1a1a; background: #fff; }
h1 { font-size: 4rem; font-weight: normal; margin: 0 0 1rem; }
h2 { font-size: 1.25rem; border-bottom: 1px solid #ccc; margin: 2.5rem 0 1rem; }
h3 { font-size: 0.9rem; margin: 1.5rem 0 0.5rem; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; }
dt { color: #666; }
dd { margin: 0; }
.row { display: grid; grid-template-columns: 8rem 1fr; gap: 1rem; align-items: baseline; }
.label { color: #666; font-size: 0.8rem; margin: 0.25rem 0; }
.sample { font-size: 32px; margin: 0.25rem 0; overflow-wrap: anywhere; }
.chars { display: flex; flex-wrap: wrap; font-size: 28px; margin: 0; }
.chars span { width: 1.6em; text-align: center; padding: 0.15em 0; }
@media print { body { margin: 0; max-width: none; } section { break-inside: avoid-page; } }
";

/// One row of the styles section.
fn style_row(h: &mut String, face: usize, label: &str, style: &str, sample: &str) {
    let _ = writeln!(
        h,
        "<div class=\"row\"><p class=\"label\">{label}</p><p class=\"sample f{face}\"{style}>{sample}</p></div>"
    );
}

/// The face the header, waterfall and character set show: the one nearest
/// Regular (upright first, then weight nearest 400, then width nearest 5);
/// the first of equals in the library's order.
fn shown_face(faces: &[SpecimenFace]) -> usize {
    (0..faces.len())
        .min_by_key(|&i| {
            let f = &faces[i];
            (
                f.slant != Slant::Upright,
                f.weight.abs_diff(400),
                f.width.abs_diff(5),
            )
        })
        .unwrap_or(0)
}

/// The license in a few words: the declared type, else the open license
/// the name table states.
fn license_line(face: &SpecimenFace) -> String {
    let declared = match face.declared {
        Some(LicenseType::Desktop) => Some("Declared desktop license"),
        Some(LicenseType::Web) => Some("Declared web license"),
        Some(LicenseType::App) => Some("Declared app license"),
        Some(LicenseType::Trial) => Some("Declared trial license"),
        Some(LicenseType::PersonalOnly) => Some("Declared for personal use only"),
        Some(LicenseType::Unknown) | None => None,
    };
    match (declared, face.open_license) {
        (Some(d), _) => d.to_owned(),
        (None, Some(l)) => WebReason::Open(l).text(),
        (None, None) => "No license declared".to_owned(),
    }
}

/// Whether the family may be used on the web, and why.
fn web_line(web: WebReason) -> String {
    let verdict = if web.ok() {
        "OK for web"
    } else {
        "Not OK for web"
    };
    format!("{verdict} ({})", web.text())
}

/// `font-variation-settings` for a named instance; axes whose tag or value
/// CSS cannot carry are left out.
fn variation_settings(inst: &NamedInstance) -> String {
    inst.coordinates
        .iter()
        .filter(|a| css_tag(&a.tag) && a.value.is_finite())
        .map(|a| format!("&quot;{}&quot; {}", a.tag, a.value))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A four-letter OpenType tag CSS can quote as is: printable ASCII, no
/// quote, no backslash, no character HTML escapes.
fn css_tag(tag: &str) -> bool {
    tag.len() == 4
        && tag
            .bytes()
            .all(|b| (b' '..=b'~').contains(&b) && !b"\"'\\<>&:".contains(&b))
}

/// `text` safe in HTML text and attribute values: `&`, `<`, `>`, `"` and
/// `'` are escaped, and so is `:`, so text from a font never forms an
/// `http:` or `file:` address in the page.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            ':' => out.push_str("&#58;"),
            c => out.push(c),
        }
    }
    out
}

/// Standard base64 (RFC 4648, with padding).
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> shift & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
