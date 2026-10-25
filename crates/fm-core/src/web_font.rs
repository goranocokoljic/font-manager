//! Web fonts (E20.1): WOFF 2 and WOFF 1, written by the core.
//!
//! - **Source**: the face's file is only read. A WOFF or WOFF2 file is
//!   decoded first; a face of a TTC/OTC is cut out as a standalone font
//!   with only its own tables. Table bytes are kept as they are, except the
//!   `head` table's `checkSumAdjustment`, which is set for the standalone
//!   font a decoder rebuilds (tables sorted by tag, 4-byte aligned).
//! - **Subset** (E20.2): when the request keeps only some characters, the
//!   standalone font is cut with [`subset::cut`] before it is encoded.
//! - **WOFF 2**: every table is stored untransformed (`glyf`/`loca` with the
//!   null transform, version 3), and the table data is one `brotli` stream.
//! - **WOFF 1**: each table is compressed with zlib, or stored as it is when
//!   that is not smaller.
//! - **Export**: all or nothing. Every target name is checked before anything
//!   is written; an existing file is replaced only when the request says so
//!   ([`Existing::Replace`], `fm export --force`). Each file is written to
//!   a temporary name in the output folder and renamed only when every file
//!   of the request was written, so a failed export leaves no files behind.
//! - **CSS** (E20.3): [`export_with_css`] also writes `fonts.css` beside the
//!   fonts ([`web_css`]), in the same all-or-nothing way; with a subset each
//!   block's `unicode-range` is what the written file's `cmap` maps.
//! - **Errors**: a broken, Type 1 or unreadable face is an [`FmError`] with a
//!   message for the user, never a panic.

use std::collections::HashSet;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use fm_api::export::WebFormat;
use read_fonts::{FileRef, FontRef, TableProvider as _};

use crate::error::{FmError, FmResult};
use crate::font_file::font_mime;
use crate::glyphs;
use crate::subset::{self, Keep};
use crate::web_css::{self, FaceStyle};

/// The name of the stylesheet [`export_with_css`] writes.
pub const CSS_FILE: &str = "fonts.css";

/// What an export does with a target file that already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Existing {
    /// Refuse the whole export with `InvalidRequest` naming the file.
    #[default]
    Refuse,
    /// Replace it: the user asked to (`ExportWebRequest::overwrite`).
    Replace,
}

/// A face to export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFace {
    /// The file, where the user keeps it.
    pub path: PathBuf,
    /// The face's index in a collection file; 0 otherwise.
    pub collection_index: u32,
    /// The output file name without extension ([`file_stem`]).
    pub file_stem: String,
    /// The face as the user knows it ("Inter Bold"), for error messages.
    pub label: String,
}

/// One table of a standalone font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The tag.
    pub tag: [u8; 4],
    /// The table's bytes, unpadded.
    pub data: Vec<u8>,
}

/// A standalone TrueType or OpenType font as its tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sfnt {
    /// The sfnt version (`0x00010000`, `OTTO`, `true`).
    pub flavor: u32,
    /// The tables, sorted by tag.
    pub tables: Vec<Table>,
}

/// The file extension of `format`.
pub fn extension(format: WebFormat) -> &'static str {
    match format {
        WebFormat::Woff2 => "woff2",
        WebFormat::Woff => "woff",
    }
}

/// The output file name of a face without extension: its PostScript name,
/// with every character a file system may refuse replaced by `_`. A face
/// without a PostScript name is `face-<id>`.
pub fn file_stem(postscript_name: &str, face_id: u64) -> String {
    if postscript_name.is_empty() {
        return format!("face-{face_id}");
    }
    postscript_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && !r#"<>:"/\|?*"#.contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Writes every face of `faces` in every format of `formats` to `out_dir`
/// as `<file_stem>.<extension>`, and answers the absolute paths written, face
/// by face in request order. With `keep`, each face is first cut down to
/// those characters ([`subset::cut`]).
///
/// A target that already exists, or two faces with the same file name, is
/// `InvalidRequest` naming the file, and nothing is written. A face that
/// cannot be read or converted fails the whole export, and the files
/// written for it so far are removed.
pub fn export(
    faces: &[ExportFace],
    formats: &[WebFormat],
    keep: Option<&Keep>,
    out_dir: &Path,
) -> FmResult<Vec<PathBuf>> {
    Ok(write(faces, None, formats, keep, out_dir, Existing::Refuse)?.files)
}

/// What [`export_with_css`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exported {
    /// The absolute paths written: the fonts as [`export`] lists them, then
    /// `fonts.css`.
    pub files: Vec<PathBuf>,
    /// The text of `fonts.css`.
    pub css: String,
}

/// As [`export`], and also writes [`CSS_FILE`] with one `@font-face` block
/// per face, `styles[i]` describing `faces[i]`. An existing `fonts.css`
/// refuses the export as an existing font file does.
pub fn export_with_css(
    faces: &[ExportFace],
    styles: &[FaceStyle],
    formats: &[WebFormat],
    keep: Option<&Keep>,
    out_dir: &Path,
) -> FmResult<Exported> {
    if styles.len() != faces.len() {
        return Err(FmError::internal("one CSS style per exported face"));
    }
    let out = write(
        faces,
        Some(styles),
        formats,
        keep,
        out_dir,
        Existing::Refuse,
    )?;
    Ok(Exported {
        files: out.files,
        css: out.css.unwrap_or_default(),
    })
}

/// The files [`write`] wrote, and the stylesheet when it wrote one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The absolute paths written, as [`export_with_css`] lists them.
    pub files: Vec<PathBuf>,
    /// The text of `fonts.css`, when `styles` was given.
    pub css: Option<String>,
}

/// As [`export`], or [`export_with_css`] when `styles` is given, with
/// `existing` saying what to do with a target that exists. A replaced file
/// is replaced in one rename, and is left as written when a later file of
/// the export fails: removing it would lose the user's file and the new one.
pub fn write(
    faces: &[ExportFace],
    styles: Option<&[FaceStyle]>,
    formats: &[WebFormat],
    keep: Option<&Keep>,
    out_dir: &Path,
    existing: Existing,
) -> FmResult<Output> {
    let out_dir = std::path::absolute(out_dir)
        .map_err(|e| FmError::io("The output folder could not be opened.", out_dir, &e))?;
    match fs::metadata(&out_dir) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            return Err(
                FmError::invalid("The output folder is a file. Choose a folder.")
                    .with_detail(out_dir.display().to_string()),
            );
        }
        Err(e) => {
            return Err(FmError::io(
                "The output folder could not be opened.",
                &out_dir,
                &e,
            ));
        }
    }

    let targets = plan(faces, formats, &out_dir, existing)?;
    let css_target = match styles {
        Some(_) => Some(free_target(&out_dir, CSS_FILE, existing)?),
        None => None,
    };
    let mut written = Written::default();
    written.existing = existing;
    let mut next = targets.iter();
    let mut blocks = Vec::new();
    for (i, face) in faces.iter().enumerate() {
        let data = fs::read(&face.path).map_err(|e| {
            FmError::io(format!("{} could not be read.", face.label), &face.path, &e)
        })?;
        let mut sfnt = standalone(&data, face.collection_index).map_err(|e| about(face, e))?;
        if let Some(keep) = keep {
            sfnt = subset::cut(&sfnt, keep).map_err(|e| about(face, e))?;
        }
        for &format in formats {
            let Some(target) = next.next() else {
                return Err(FmError::internal("export plan is shorter than the request"));
            };
            let bytes = encode(&sfnt, format).map_err(|e| about(face, e))?;
            written.temp(target, &bytes)?;
        }
        if let Some(style) = styles.and_then(|s| s.get(i)) {
            let mapped = match keep {
                Some(_) => Some(mapped_code_points(&sfnt).map_err(|e| about(face, e))?),
                None => None,
            };
            blocks.push(web_css::font_face(
                style,
                &face.file_stem,
                formats,
                mapped.as_deref(),
            ));
        }
    }
    let css = match css_target {
        Some(target) => {
            let css = web_css::stylesheet(&blocks);
            written.temp(&target, css.as_bytes())?;
            Some(css)
        }
        None => None,
    };
    Ok(Output {
        files: written.commit()?,
        css,
    })
}

/// The code points the `cmap` of `sfnt` maps, ascending.
pub fn mapped_code_points(sfnt: &Sfnt) -> FmResult<Vec<u32>> {
    use skrifa::MetadataProvider as _;
    let bytes = sfnt_bytes(sfnt)?;
    let font =
        FontRef::new(&bytes).map_err(|e| broken(format!("The font header is malformed: {e}.")))?;
    Ok(font.charmap().mappings().map(|(cp, _)| cp).collect())
}

/// `out_dir/name`, or `InvalidRequest` naming the file when it exists and
/// `existing` refuses it.
fn free_target(out_dir: &Path, name: &str, existing: Existing) -> FmResult<PathBuf> {
    let target = out_dir.join(name);
    if existing == Existing::Refuse && fs::symlink_metadata(&target).is_ok() {
        return Err(FmError::invalid(format!(
            "{name} already exists in the output folder. Choose another folder, or move the file first."
        ))
        .with_detail(target.display().to_string()));
    }
    Ok(target)
}

/// The target of every (face, format) in request order, after checking that
/// none exists (unless `existing` replaces it) and no two share a name. File systems that ignore case would
/// put `A.woff2` and `a.woff2` in one file, so names are compared without it.
fn plan(
    faces: &[ExportFace],
    formats: &[WebFormat],
    out_dir: &Path,
    existing: Existing,
) -> FmResult<Vec<PathBuf>> {
    let mut seen = HashSet::new();
    let mut targets = Vec::with_capacity(faces.len() * formats.len());
    for face in faces {
        for &format in formats {
            let name = format!("{}.{}", face.file_stem, extension(format));
            if !seen.insert(name.to_lowercase()) {
                return Err(FmError::invalid(format!(
                    "Two of the fonts would both be written as {name}. Export them separately."
                )));
            }
            targets.push(free_target(out_dir, &name, existing)?);
        }
    }
    Ok(targets)
}

/// `e` with the face named in its message.
fn about(face: &ExportFace, e: FmError) -> FmError {
    let named = FmError::new(e.code(), format!("{}: {}", face.label, e.message()));
    let detail = e.detail().map_or_else(
        || face.path.display().to_string(),
        |d| format!("{}: {d}", face.path.display()),
    );
    named.with_detail(detail)
}

/// The files of one export: temporary ones until [`Written::commit`], and
/// the targets renamed so far. Dropped before the commit ends, it removes
/// every file it created, so a failed export leaves nothing behind. These are
/// files this export made, never one of the user's: a target that replaced
/// a file ([`Existing::Replace`]) is listed in `files` but not in `done`.
#[derive(Debug, Default)]
struct Written {
    temps: Vec<(PathBuf, PathBuf)>,
    files: Vec<PathBuf>,
    done: Vec<PathBuf>,
    committed: bool,
    existing: Existing,
}

impl Written {
    /// Writes `bytes` to a temporary file beside `target`.
    fn temp(&mut self, target: &Path, bytes: &[u8]) -> FmResult<()> {
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temp = target.with_file_name(format!(".{name}.{}.fm-part", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| FmError::io("The web font could not be written.", &temp, &e))?;
        self.temps.push((temp.clone(), target.to_path_buf()));
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| FmError::io("The web font could not be written.", &temp, &e))
    }

    /// Renames every temporary file to its target. A target that appeared
    /// since the plan is not replaced unless the export replaces files: the
    /// export fails and is undone.
    fn commit(mut self) -> FmResult<Vec<PathBuf>> {
        let temps = std::mem::take(&mut self.temps);
        let mut left = temps.iter();
        for (temp, target) in left.by_ref() {
            let exists = fs::symlink_metadata(target).is_ok();
            let moved = if exists && self.existing == Existing::Refuse {
                Err(FmError::invalid(format!(
                    "{} already exists in the output folder. Choose another folder, or move the file first.",
                    target.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
                ))
                .with_detail(target.display().to_string()))
            } else {
                fs::rename(temp, target)
                    .map_err(|e| FmError::io("The web font could not be written.", target, &e))
            };
            match moved {
                Ok(()) => {
                    self.files.push(target.clone());
                    if !exists {
                        self.done.push(target.clone());
                    }
                }
                Err(e) => {
                    let _ = fs::remove_file(temp);
                    self.temps = left.cloned().collect();
                    return Err(e);
                }
            }
        }
        self.committed = true;
        Ok(std::mem::take(&mut self.files))
    }
}

impl Drop for Written {
    fn drop(&mut self) {
        for (temp, _) in &self.temps {
            let _ = fs::remove_file(temp);
        }
        if !self.committed {
            for target in &self.done {
                let _ = fs::remove_file(target);
            }
        }
    }
}

/// The face `collection_index` of the font file `data` (TrueType, OpenType,
/// a collection, WOFF or WOFF2) as a standalone font.
pub fn standalone(data: &[u8], collection_index: u32) -> FmResult<Sfnt> {
    if font_mime(data).is_none() {
        return Err(if is_type1(data) {
            FmError::unsupported(
                "This is a Type 1 font. Only TrueType and OpenType fonts can be converted to web fonts.",
            )
        } else {
            FmError::unsupported("This file is not a TrueType or OpenType font.")
        });
    }
    let sfnt = glyphs::sfnt(data).map_err(broken)?;
    let font = match FileRef::new(&sfnt) {
        Ok(FileRef::Font(font)) if collection_index == 0 => font,
        Ok(FileRef::Font(_)) => {
            return Err(broken(format!("The file has no face {collection_index}.")));
        }
        Ok(FileRef::Collection(c)) => c.get(collection_index).map_err(|e| {
            broken(format!(
                "Face {collection_index} of the collection is malformed: {e}."
            ))
        })?,
        Err(e) => return Err(broken(format!("The font header is malformed: {e}."))),
    };
    let mut out = Sfnt {
        flavor: font.table_directory().sfnt_version(),
        tables: tables(&font)?,
    };
    out.tables.sort_by_key(|t| t.tag);
    set_checksum_adjustment(&mut out);
    Ok(out)
}

fn tables(font: &FontRef<'_>) -> FmResult<Vec<Table>> {
    let records = font.table_directory().table_records();
    if records.is_empty() {
        return Err(broken("The font has no tables."));
    }
    let mut seen = HashSet::new();
    let mut tables = Vec::with_capacity(records.len());
    for record in records {
        let tag = record.tag().to_be_bytes();
        if !seen.insert(tag) {
            return Err(broken(format!(
                "The {} table is listed twice.",
                record.tag()
            )));
        }
        let data = font
            .table_data(record.tag())
            .ok_or_else(|| broken(format!("The {} table is cut off.", record.tag())))?;
        tables.push(Table {
            tag,
            data: data.as_bytes().to_vec(),
        });
    }
    // A decoder needs these to rebuild the font.
    font.head()
        .map_err(|e| broken(format!("The head table is malformed: {e}.")))?;
    font.maxp()
        .map_err(|e| broken(format!("The maxp table is malformed: {e}.")))?;
    Ok(tables)
}

/// Whether `data` starts like a Type 1 font: PFB (`0x80 0x01`) or PFA text.
fn is_type1(data: &[u8]) -> bool {
    data.starts_with(&[0x80, 0x01])
        || data.starts_with(b"%!PS-AdobeFont")
        || data.starts_with(b"%!FontType1")
}

fn broken(why: impl Into<String>) -> FmError {
    FmError::new(
        ErrorCode::Unsupported,
        "The font is damaged and cannot be converted.",
    )
    .with_detail(why.into())
}

/// The font in `format`.
pub fn encode(sfnt: &Sfnt, format: WebFormat) -> FmResult<Vec<u8>> {
    match format {
        WebFormat::Woff2 => woff2(sfnt),
        WebFormat::Woff => woff1(sfnt),
    }
}

/// The font as the sfnt a decoder rebuilds: tables sorted by tag, each at a
/// 4-byte boundary and padded with zeros.
pub fn sfnt_bytes(sfnt: &Sfnt) -> FmResult<Vec<u8>> {
    let n = u16_of(sfnt.tables.len())?;
    let mut out = Vec::with_capacity(sfnt_size(sfnt));
    out.extend_from_slice(&sfnt.flavor.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    let selector = if n == 0 {
        0
    } else {
        15 - n.leading_zeros() as u16
    };
    let range = (1u16 << selector).saturating_mul(16);
    out.extend_from_slice(&range.to_be_bytes());
    out.extend_from_slice(&selector.to_be_bytes());
    out.extend_from_slice(&n.saturating_mul(16).saturating_sub(range).to_be_bytes());
    let mut offset = 12 + 16 * sfnt.tables.len();
    for t in &sfnt.tables {
        out.extend_from_slice(&t.tag);
        out.extend_from_slice(&table_checksum(t).to_be_bytes());
        out.extend_from_slice(&u32_of(offset)?.to_be_bytes());
        out.extend_from_slice(&u32_of(t.data.len())?.to_be_bytes());
        offset += padded(t.data.len());
    }
    for t in &sfnt.tables {
        out.extend_from_slice(&t.data);
        pad(&mut out);
    }
    Ok(out)
}

/// The size of [`sfnt_bytes`].
fn sfnt_size(sfnt: &Sfnt) -> usize {
    12 + 16 * sfnt.tables.len()
        + sfnt
            .tables
            .iter()
            .map(|t| padded(t.data.len()))
            .sum::<usize>()
}

/// Sets `head.checkSumAdjustment` for [`sfnt_bytes`]: zero, then
/// `0xB1B0AFBA` minus the checksum of the whole font. A font whose `head`
/// is too short to hold the field keeps it as it is.
fn set_checksum_adjustment(sfnt: &mut Sfnt) {
    let Some(i) = sfnt.tables.iter().position(|t| &t.tag == b"head") else {
        return;
    };
    let Some(field) = sfnt.tables[i].data.get_mut(8..12) else {
        return;
    };
    field.fill(0);
    let Ok(bytes) = sfnt_bytes(sfnt) else {
        return;
    };
    let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(checksum(&bytes));
    if let Some(field) = sfnt.tables[i].data.get_mut(8..12) {
        field.copy_from_slice(&adjustment.to_be_bytes());
    }
}

/// The checksum of a table's directory entry. The `head` table's is taken
/// with `checkSumAdjustment` as zero, so the whole font sums to `0xB1B0AFBA`.
fn table_checksum(t: &Table) -> u32 {
    let sum = checksum(&t.data);
    match t.data.get(8..12) {
        Some(field) if &t.tag == b"head" => sum.wrapping_sub(checksum(field)),
        _ => sum,
    }
}

/// The OpenType checksum: the sum of the big-endian `u32` words, the last
/// one padded with zeros.
pub fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

/// WOFF 1: a 44-byte header, a 20-byte entry per table, then each table
/// zlib-compressed (or as it is when that is not smaller) at a 4-byte
/// boundary.
fn woff1(sfnt: &Sfnt) -> FmResult<Vec<u8>> {
    let n = sfnt.tables.len();
    let mut directory = Vec::with_capacity(20 * n);
    let mut body = Vec::new();
    let mut offset = 44 + 20 * n;
    for t in &sfnt.tables {
        let compressed = zlib(&t.data)?;
        let stored = if compressed.len() < t.data.len() {
            &compressed
        } else {
            &t.data
        };
        directory.extend_from_slice(&t.tag);
        directory.extend_from_slice(&u32_of(offset)?.to_be_bytes());
        directory.extend_from_slice(&u32_of(stored.len())?.to_be_bytes());
        directory.extend_from_slice(&u32_of(t.data.len())?.to_be_bytes());
        directory.extend_from_slice(&table_checksum(t).to_be_bytes());
        body.extend_from_slice(stored);
        pad(&mut body);
        offset = 44 + 20 * n + body.len();
    }
    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(b"wOFF");
    out.extend_from_slice(&sfnt.flavor.to_be_bytes());
    out.extend_from_slice(&u32_of(offset)?.to_be_bytes());
    out.extend_from_slice(&u16_of(n)?.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&u32_of(sfnt_size(sfnt))?.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    // No metadata, no private data.
    out.extend_from_slice(&[0u8; 20]);
    out.extend_from_slice(&directory);
    out.extend_from_slice(&body);
    Ok(out)
}

fn zlib(data: &[u8]) -> FmResult<Vec<u8>> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(data)
        .and_then(|()| z.finish())
        .map_err(|e| FmError::internal(format!("zlib: {e}")))
}

/// The tags WOFF 2 stores as a one-byte index, in index order.
const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ", b"fpgm",
    b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp", b"hdmx", b"kern",
    b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC",
    b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar", b"gvar", b"hsty",
    b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat",
    b"Gloc", b"Feat", b"Sill",
];

/// WOFF 2: a 48-byte header, the table directory, then the tables'
/// bytes, unpadded and in directory order, as one brotli stream. No table
/// is transformed: `glyf` and `loca` carry transform version 3 (their null
/// transform), every other table version 0.
fn woff2(sfnt: &Sfnt) -> FmResult<Vec<u8>> {
    let mut directory = Vec::new();
    let mut stream = Vec::with_capacity(sfnt.tables.iter().map(|t| t.data.len()).sum());
    for t in &sfnt.tables {
        let known = KNOWN_TAGS.iter().position(|k| **k == t.tag);
        let version: u8 = if &t.tag == b"glyf" || &t.tag == b"loca" {
            3
        } else {
            0
        };
        let index = known.and_then(|i| u8::try_from(i).ok()).unwrap_or(63);
        directory.push(index | (version << 6));
        if index == 63 {
            directory.extend_from_slice(&t.tag);
        }
        base128(&mut directory, u32_of(t.data.len())?);
        stream.extend_from_slice(&t.data);
    }
    let compressed = brotli_font(&stream)?;
    let mut out = Vec::with_capacity(48 + directory.len() + compressed.len() + 3);
    out.extend_from_slice(b"wOF2");
    out.extend_from_slice(&sfnt.flavor.to_be_bytes());
    let length = padded(48 + directory.len() + compressed.len());
    out.extend_from_slice(&u32_of(length)?.to_be_bytes());
    out.extend_from_slice(&u16_of(sfnt.tables.len())?.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&u32_of(sfnt_size(sfnt))?.to_be_bytes());
    out.extend_from_slice(&u32_of(compressed.len())?.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    // No metadata, no private data.
    out.extend_from_slice(&[0u8; 20]);
    out.extend_from_slice(&directory);
    out.extend_from_slice(&compressed);
    pad(&mut out);
    Ok(out)
}

fn brotli_font(data: &[u8]) -> FmResult<Vec<u8>> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        lgwin: 22,
        mode: brotli::enc::backward_references::BrotliEncoderMode::BROTLI_MODE_FONT,
        size_hint: data.len(),
        ..Default::default()
    };
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut out, &params)
        .map_err(|e| FmError::internal(format!("brotli: {e}")))?;
    Ok(out)
}

/// WOFF 2's `UIntBase128`: big-endian groups of 7 bits, the high bit set on
/// every byte but the last, no leading zero groups.
fn base128(out: &mut Vec<u8>, value: u32) {
    let groups = (32 - value.leading_zeros()).div_ceil(7).max(1);
    for g in (0..groups).rev() {
        // `& 0x7f` keeps the value in a byte.
        let byte = ((value >> (7 * g)) & 0x7f) as u8;
        out.push(if g == 0 { byte } else { byte | 0x80 });
    }
}

fn padded(len: usize) -> usize {
    len.next_multiple_of(4)
}

fn pad(out: &mut Vec<u8>) {
    out.resize(padded(out.len()), 0);
}

fn u32_of(n: usize) -> FmResult<u32> {
    u32::try_from(n).map_err(|_| FmError::unsupported("The font is too large for a web font."))
}

fn u16_of(n: usize) -> FmResult<u16> {
    u16::try_from(n)
        .map_err(|_| FmError::unsupported("The font has too many tables for a web font."))
}

#[cfg(test)]
mod tests;
