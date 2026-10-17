//! Font parsing with fontations (ADR 0002): the second step of a scan.
//!
//! [`parse`] reads one font file from memory and returns every face in it with
//! the metadata the index needs: names, OS/2 classes, the italic angle,
//! variation axes and named instances, layout feature tags and cmap code points.
//!
//! Malformed data is never a panic. A file whose header cannot be read is
//! [`Health::Broken`] with no faces; a face whose tables are malformed or cut
//! off is listed with its own `Broken` health and whatever could still be
//! read, and a file whose faces are all broken is broken as a whole. A
//! table that is simply absent is not an error: its fields stay empty. Formats
//! fontations does not read (WOFF, WOFF2, Type 1, dfont) are recognized by
//! their magic bytes and returned as [`Health::Unsupported`] without faces.

use std::fs;
use std::path::Path;

use fm_api::common::{Axis, AxisValue, FontFormat, Health, NamedInstance};
use read_fonts::types::Tag;
use read_fonts::{CollectionRef, FontRef, ReadError, TableProvider};
use skrifa::MetadataProvider;
use skrifa::string::StringId;

use crate::{FmError, FmResult};

/// What [`parse`] found in one font file.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    /// The format read from the magic bytes; `None` when the data is not a
    /// font format the app knows.
    pub format: Option<FontFormat>,
    /// Whether the file as a whole could be read. `Ok` does not mean every
    /// face is; see [`ParsedFace::health`].
    pub health: Health,
    /// Every face, in file order (one for a single font, all of a collection).
    pub faces: Vec<ParsedFace>,
}

/// One face of a font file.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFace {
    /// The face's index in its collection; 0 for a single font.
    pub index: u32,
    /// Family name (name ID 1).
    pub family: Option<String>,
    /// Subfamily name (name ID 2).
    pub subfamily: Option<String>,
    /// Typographic family name (name ID 16).
    pub typographic_family: Option<String>,
    /// Typographic subfamily name (name ID 17).
    pub typographic_subfamily: Option<String>,
    /// PostScript name (name ID 6).
    pub postscript_name: Option<String>,
    /// Version string (name ID 5).
    pub version: Option<String>,
    /// Foundry: the manufacturer name (name ID 8).
    pub foundry: Option<String>,
    /// `OS/2.achVendID`, trimmed; `None` when blank.
    pub vendor_id: Option<String>,
    /// Designer (name ID 9).
    pub designer: Option<String>,
    /// Copyright notice (name ID 0).
    pub copyright: Option<String>,
    /// License description (name ID 13).
    pub license: Option<String>,
    /// License URL (name ID 14).
    pub license_url: Option<String>,
    /// `OS/2.usWeightClass`; `None` without an OS/2 table.
    pub weight: Option<u16>,
    /// `OS/2.usWidthClass`; `None` without an OS/2 table.
    pub width: Option<u16>,
    /// `OS/2.fsType` (embedding permissions); `None` without an OS/2 table.
    pub fs_type: Option<u16>,
    /// `post.italicAngle` in degrees; `None` without a post table.
    pub italic_angle: Option<f32>,
    /// Variation axes from `fvar`, in table order; empty for a static font.
    pub axes: Vec<Axis>,
    /// Named instances from `fvar`, in table order.
    pub named_instances: Vec<NamedInstance>,
    /// Feature tags from `GSUB` and `GPOS`, sorted, without duplicates.
    pub features: Vec<String>,
    /// Code points mapped by `cmap`, sorted, without duplicates.
    pub code_points: Vec<u32>,
    /// Whether this face could be read in full.
    pub health: Health,
}

impl ParsedFace {
    fn empty(index: u32, health: Health) -> Self {
        Self {
            index,
            family: None,
            subfamily: None,
            typographic_family: None,
            typographic_subfamily: None,
            postscript_name: None,
            version: None,
            foundry: None,
            vendor_id: None,
            designer: None,
            copyright: None,
            license: None,
            license_url: None,
            weight: None,
            width: None,
            fs_type: None,
            italic_angle: None,
            axes: Vec::new(),
            named_instances: Vec::new(),
            features: Vec::new(),
            code_points: Vec::new(),
            health,
        }
    }

    /// Whether the face has variation axes.
    pub fn is_variable(&self) -> bool {
        !self.axes.is_empty()
    }
}

/// Reads the font file at `path` and parses it.
///
/// Fails only when the file cannot be read from disk; malformed font data is
/// a [`ParsedFile`] with [`Health::Broken`].
pub fn parse_file(path: &Path) -> FmResult<ParsedFile> {
    let data =
        fs::read(path).map_err(|e| FmError::io("The font file could not be read.", path, &e))?;
    Ok(parse(&data))
}

/// Parses a font file held in memory. Never panics, whatever `data` holds.
pub fn parse(data: &[u8]) -> ParsedFile {
    let magic: [u8; 4] = match data.get(..4).and_then(|m| m.try_into().ok()) {
        Some(magic) => magic,
        None => return broken_file(None, "The file is too short to be a font."),
    };
    match &magic {
        b"ttcf" => parse_collection(data),
        [0, 1, 0, 0] | b"true" | b"OTTO" => parse_single(data, &magic),
        b"wOFF" => unsupported(FontFormat::Woff, "WOFF files are not parsed yet."),
        b"wOF2" => unsupported(FontFormat::Woff2, "WOFF2 files are not parsed yet."),
        _ if is_type1(data) => unsupported(
            FontFormat::Type1,
            "Type 1 fonts are detected but not parsed.",
        ),
        [0, 0, 1, 0] => unsupported(
            FontFormat::Dfont,
            "Mac resource-fork fonts are detected but not parsed.",
        ),
        _ => broken_file(None, "The file is not a font format Font Manager can read."),
    }
}

fn parse_single(data: &[u8], magic: &[u8; 4]) -> ParsedFile {
    let format = if magic == b"OTTO" {
        FontFormat::Otf
    } else {
        FontFormat::Ttf
    };
    match FontRef::new(data) {
        Ok(font) => with_faces(format, vec![parse_face(&font, 0)]),
        Err(e) => broken_file(Some(format), format!("The font header is malformed: {e}.")),
    }
}

fn parse_collection(data: &[u8]) -> ParsedFile {
    let collection = match CollectionRef::new(data) {
        Ok(collection) => collection,
        Err(e) => {
            return broken_file(
                Some(FontFormat::Ttc),
                format!("The collection header is malformed: {e}."),
            );
        }
    };
    if collection.is_empty() {
        return broken_file(Some(FontFormat::Ttc), "The collection holds no fonts.");
    }
    let mut format = FontFormat::Ttc;
    let mut faces = Vec::new();
    for (index, font) in (0..collection.len()).zip(collection.iter()) {
        match font {
            Ok(font) => {
                if index == 0 && font.table_data(Tag::new(b"CFF ")).is_some() {
                    format = FontFormat::Otc;
                }
                faces.push(parse_face(&font, index));
            }
            Err(e) => faces.push(ParsedFace::empty(
                index,
                Health::Broken(format!("Face {index} of the collection is malformed: {e}.")),
            )),
        }
    }
    with_faces(format, faces)
}

/// A file whose faces are all broken is broken as a whole, with the first
/// face's reason; otherwise it is `Ok` and each face carries its own health.
fn with_faces(format: FontFormat, faces: Vec<ParsedFace>) -> ParsedFile {
    let health = match faces.first() {
        Some(ParsedFace {
            health: Health::Broken(reason),
            ..
        }) if faces.iter().all(|f| matches!(f.health, Health::Broken(_))) => {
            Health::Broken(reason.clone())
        }
        _ => Health::Ok,
    };
    ParsedFile {
        format: Some(format),
        health,
        faces,
    }
}

/// Reads one face. Every table is read on its own, so one malformed table
/// marks the face broken but does not hide the others.
fn parse_face(font: &FontRef<'_>, index: u32) -> ParsedFace {
    let mut face = ParsedFace::empty(index, Health::Ok);
    let mut problems: Vec<String> = Vec::new();

    // read-fonts treats a table that points past the end of the data as
    // absent; for a truncated file that is damage, not absence.
    let len = font.data().len() as u64;
    for record in font.table_directory().table_records() {
        if u64::from(record.offset()) + u64::from(record.length()) > len {
            problems.push(format!("The {} table is cut off.", record.tag()));
        }
    }

    if present(font.name(), "name", &mut problems).is_some() {
        let string = |id| {
            font.localized_strings(id)
                .english_or_first()
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty())
        };
        face.family = string(StringId::FAMILY_NAME);
        face.subfamily = string(StringId::SUBFAMILY_NAME);
        face.typographic_family = string(StringId::TYPOGRAPHIC_FAMILY_NAME);
        face.typographic_subfamily = string(StringId::TYPOGRAPHIC_SUBFAMILY_NAME);
        face.postscript_name = string(StringId::POSTSCRIPT_NAME);
        face.version = string(StringId::VERSION_STRING);
        face.foundry = string(StringId::MANUFACTURER);
        face.designer = string(StringId::DESIGNER);
        face.copyright = string(StringId::COPYRIGHT_NOTICE);
        face.license = string(StringId::LICENSE_DESCRIPTION);
        face.license_url = string(StringId::LICENSE_URL);
    }

    if let Some(os2) = present(font.os2(), "OS/2", &mut problems) {
        face.weight = Some(os2.us_weight_class());
        face.width = Some(os2.us_width_class());
        face.fs_type = Some(os2.fs_type());
        let vendor = os2.ach_vend_id().to_string();
        let vendor = vendor.trim_matches(|c: char| c == ' ' || c == '\0');
        face.vendor_id = (!vendor.is_empty()).then(|| vendor.to_owned());
    }

    if let Some(post) = present(font.post(), "post", &mut problems) {
        face.italic_angle = Some(post.italic_angle().to_f32());
    }

    if present(font.fvar(), "fvar", &mut problems).is_some() {
        read_variations(font, &mut face);
    }

    let mut features = Vec::new();
    if let Some(gsub) = present(font.gsub(), "GSUB", &mut problems) {
        collect_features(gsub.feature_list(), "GSUB", &mut features, &mut problems);
    }
    if let Some(gpos) = present(font.gpos(), "GPOS", &mut problems) {
        collect_features(gpos.feature_list(), "GPOS", &mut features, &mut problems);
    }
    features.sort_unstable();
    features.dedup();
    face.features = features;

    if present(font.cmap(), "cmap", &mut problems).is_some() {
        let mut code_points: Vec<u32> = font.charmap().mappings().map(|(cp, _)| cp).collect();
        code_points.sort_unstable();
        code_points.dedup();
        face.code_points = code_points;
    }

    if !problems.is_empty() {
        face.health = Health::Broken(problems.join(" "));
    }
    face
}

fn read_variations(font: &FontRef<'_>, face: &mut ParsedFace) {
    let name = |id: StringId| {
        font.localized_strings(id)
            .english_or_first()
            .map(|s| s.to_string())
    };
    let axes = font.axes();
    face.axes = axes
        .iter()
        .map(|axis| {
            let tag = axis.tag().to_string();
            Axis {
                name: name(axis.name_id()).unwrap_or_else(|| tag.clone()),
                tag,
                min: axis.min_value(),
                default: axis.default_value(),
                max: axis.max_value(),
            }
        })
        .collect();
    face.named_instances = font
        .named_instances()
        .iter()
        .map(|instance| NamedInstance {
            name: name(instance.subfamily_name_id()).unwrap_or_default(),
            coordinates: face
                .axes
                .iter()
                .zip(instance.user_coords())
                .map(|(axis, value)| AxisValue {
                    tag: axis.tag.clone(),
                    value,
                })
                .collect(),
        })
        .collect();
}

fn collect_features(
    list: Result<read_fonts::tables::layout::FeatureList<'_>, ReadError>,
    table: &str,
    features: &mut Vec<String>,
    problems: &mut Vec<String>,
) {
    match list {
        Ok(list) => features.extend(
            list.feature_records()
                .iter()
                .map(|record| record.feature_tag().to_string()),
        ),
        // A null FeatureList offset means the table lists no features; shipped
        // fonts (Londrina Solid, Long Cang) do this and render fine.
        Err(ReadError::NullOffset) => {}
        Err(e) => problems.push(format!("The {table} feature list is malformed: {e}.")),
    }
}

/// The table when it is present and readable. An absent table is `None`
/// without a problem; a malformed one is `None` and adds a problem.
fn present<T>(table: Result<T, ReadError>, name: &str, problems: &mut Vec<String>) -> Option<T> {
    match table {
        Ok(table) => Some(table),
        Err(ReadError::TableIsMissing(_)) => None,
        Err(e) => {
            problems.push(format!("The {name} table is malformed: {e}."));
            None
        }
    }
}

fn is_type1(data: &[u8]) -> bool {
    data.starts_with(&[0x80, 0x01])
        || data.starts_with(b"%!PS-AdobeFont")
        || data.starts_with(b"%!FontType1")
}

fn broken_file(format: Option<FontFormat>, reason: impl Into<String>) -> ParsedFile {
    ParsedFile {
        format,
        health: Health::Broken(reason.into()),
        faces: Vec::new(),
    }
}

fn unsupported(format: FontFormat, reason: &str) -> ParsedFile {
    ParsedFile {
        format: Some(format),
        health: Health::Unsupported(reason.to_owned()),
        faces: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use fm_api::ErrorCode;

    use super::*;
    use crate::test_support::TempDir;

    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/fonts")
            .join(rel)
    }

    fn read(rel: &str) -> Vec<u8> {
        fs::read(fixture(rel)).expect("read fixture")
    }

    fn single(rel: &str) -> ParsedFace {
        let file = parse(&read(rel));
        assert_eq!(file.health, Health::Ok, "{rel}");
        assert_eq!(file.faces.len(), 1, "{rel}");
        file.faces.into_iter().next().expect("one face")
    }

    #[test]
    fn a_collection_yields_every_face() {
        let file = parse(&read("derived/agdasima/agdasima.ttc"));
        assert_eq!(file.format, Some(FontFormat::Ttc));
        assert_eq!(file.health, Health::Ok);
        let faces: Vec<_> = file
            .faces
            .iter()
            .map(|f| (f.index, f.postscript_name.as_deref(), f.weight, &f.health))
            .collect();
        assert_eq!(
            faces,
            [
                (0, Some("Agdasima-Bold"), Some(700), &Health::Ok),
                (1, Some("Agdasima-Regular"), Some(400), &Health::Ok),
            ]
        );
        assert_eq!(file.faces[0].subfamily.as_deref(), Some("Bold"));
        assert_eq!(file.faces[1].subfamily.as_deref(), Some("Regular"));
    }

    #[test]
    fn names_and_os2_fields_are_read() {
        let face = single("fontshare/asap/Asap-SemiBold.otf");
        assert_eq!(face.family.as_deref(), Some("Asap SemiBold"));
        assert_eq!(face.subfamily.as_deref(), Some("Regular"));
        assert_eq!(face.typographic_family.as_deref(), Some("Asap"));
        assert_eq!(face.typographic_subfamily.as_deref(), Some("SemiBold"));
        assert_eq!(face.postscript_name.as_deref(), Some("Asap-SemiBold"));
        assert_eq!(face.weight, Some(600));
        assert_eq!(face.width, Some(5));
        assert!(face.fs_type.is_some());
        assert!(
            face.version
                .as_deref()
                .is_some_and(|v| v.starts_with("Version"))
        );
        assert!(face.foundry.is_some());
        assert!(face.designer.is_some());
        assert!(
            face.copyright
                .as_deref()
                .is_some_and(|c| c.contains("Copyright"))
        );
        assert!(
            face.license
                .as_deref()
                .is_some_and(|l| l.contains("Open Font License"))
        );
        assert!(
            face.license_url
                .as_deref()
                .is_some_and(|u| u.starts_with("http"))
        );
        assert!(!face.is_variable());
    }

    #[test]
    fn the_otf_and_ttf_formats_come_from_the_magic_bytes() {
        assert_eq!(
            parse(&read("fontshare/asap/Asap-SemiBold.otf")).format,
            Some(FontFormat::Otf)
        );
        assert_eq!(
            parse(&read("google-repo/castoro/Castoro[wght].ttf")).format,
            Some(FontFormat::Ttf)
        );
    }

    #[test]
    fn the_foundry_comes_from_name_id_8_and_the_vendor_from_os2() {
        let face = &parse(&read("derived/agdasima/agdasima.ttc")).faces[0];
        assert_eq!(face.foundry.as_deref(), Some("Google"));
        assert_eq!(face.vendor_id.as_deref(), Some("GOOG"));
        assert_eq!(
            face.designer.as_deref(),
            Some("The DocRepair Project, Patric King")
        );
        assert_eq!(
            face.license_url.as_deref(),
            Some("https://scripts.sil.org/OFL")
        );
        assert_eq!(face.version.as_deref(), Some("Version 2.002"));
    }

    #[test]
    fn the_italic_angle_comes_from_post() {
        let italic = single("google-repo/albertsans/AlbertSans-Italic[wght].ttf");
        assert!(
            italic.italic_angle.is_some_and(|a| a < 0.0),
            "{:?}",
            italic.italic_angle
        );
        let upright = single("google-repo/castoro/Castoro[wght].ttf");
        assert_eq!(upright.italic_angle, Some(0.0));
    }

    #[test]
    fn variation_axes_and_named_instances_are_read() {
        let face = single("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        assert!(face.is_variable());
        assert_eq!(
            face.axes,
            [
                Axis {
                    tag: "wght".into(),
                    name: "Weight".into(),
                    min: 100.0,
                    default: 400.0,
                    max: 900.0,
                },
                Axis {
                    tag: "wdth".into(),
                    name: "Width".into(),
                    min: 75.0,
                    default: 100.0,
                    max: 125.0,
                },
            ]
        );
        let thin = face.named_instances.first().expect("named instances");
        assert_eq!(thin.name, "Thin");
        assert_eq!(
            thin.coordinates,
            [
                AxisValue {
                    tag: "wght".into(),
                    value: 100.0
                },
                AxisValue {
                    tag: "wdth".into(),
                    value: 100.0
                },
            ]
        );
        assert!(face.named_instances.iter().any(|i| i.name == "Bold"));
    }

    #[test]
    fn feature_tags_merge_gsub_and_gpos_sorted_and_unique() {
        let face = single("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        // liga lives in GSUB, kern and mark in GPOS.
        for tag in ["liga", "kern", "mark"] {
            assert!(
                face.features.iter().any(|f| f == tag),
                "{tag} in {:?}",
                face.features
            );
        }
        assert!(
            face.features.windows(2).all(|w| w[0] < w[1]),
            "{:?}",
            face.features
        );
    }

    #[test]
    fn cmap_code_points_are_sorted_and_unique() {
        let face = single("google-repo/castoro/Castoro[wght].ttf");
        assert!(face.code_points.contains(&u32::from('A')));
        assert!(face.code_points.contains(&u32::from('z')));
        assert!(face.code_points.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn a_truncated_file_is_broken_with_a_reason() {
        let file = parse(&read("derived/agdasima/agdasima-truncated.ttf"));
        assert_eq!(file.format, Some(FontFormat::Ttf));
        let Health::Broken(reason) = &file.health else {
            panic!("expected broken, got {:?}", file.health);
        };
        assert!(reason.contains("name table is cut off"), "{reason}");
        assert!(matches!(file.faces[0].health, Health::Broken(_)));
    }

    /// The byte offset of the table record for `tag` in a single font's
    /// table directory (12-byte header, then 16-byte records).
    fn table_record(data: &[u8], tag: &[u8; 4]) -> usize {
        let count = usize::from(u16::from_be_bytes([data[4], data[5]]));
        (0..count)
            .map(|i| 12 + 16 * i)
            .find(|&at| &data[at..at + 4] == tag)
            .expect("table record")
    }

    fn table_offset(data: &[u8], record: usize) -> usize {
        let bytes: [u8; 4] = data[record + 8..record + 12].try_into().expect("offset");
        u32::from_be_bytes(bytes) as usize
    }

    fn broken_reason(face: &ParsedFace) -> &str {
        match &face.health {
            Health::Broken(reason) => reason,
            other => panic!("expected broken, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_table_inside_the_file_breaks_the_face() {
        let mut data = read("google-repo/castoro/Castoro[wght].ttf");
        // Shrink the OS/2 record to 10 bytes: still inside the file, too short
        // for the table.
        let record = table_record(&data, b"OS/2");
        data[record + 12..record + 16].copy_from_slice(&10u32.to_be_bytes());
        let file = parse(&data);
        let face = &file.faces[0];
        let reason = broken_reason(face);
        assert!(reason.contains("OS/2 table is malformed"), "{reason}");
        assert_eq!(face.weight, None);
        // The other tables are still read.
        assert!(face.family.is_some());
        assert!(!face.code_points.is_empty());
    }

    #[test]
    fn a_null_feature_list_is_no_features_not_broken() {
        let mut data = read("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        // A null GSUB FeatureList offset (header bytes 6..8), as in Londrina Solid.
        let gsub = table_offset(&data, table_record(&data, b"GSUB"));
        data[gsub + 6..gsub + 8].copy_from_slice(&0u16.to_be_bytes());
        let face = single_from(&data);
        assert_eq!(face.health, Health::Ok);
        // GSUB-only features are gone; GPOS features are still read.
        assert!(
            !face.features.iter().any(|f| f == "liga"),
            "{:?}",
            face.features
        );
        assert!(
            face.features.iter().any(|f| f == "kern"),
            "{:?}",
            face.features
        );
    }

    fn single_from(data: &[u8]) -> ParsedFace {
        let file = parse(data);
        assert_eq!(file.health, Health::Ok);
        file.faces.into_iter().next().expect("one face")
    }

    #[test]
    fn a_malformed_feature_list_breaks_the_face() {
        let mut data = read("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        // Point the GSUB FeatureList offset (header bytes 6..8) past the table.
        let gsub = table_offset(&data, table_record(&data, b"GSUB"));
        data[gsub + 6..gsub + 8].copy_from_slice(&u16::MAX.to_be_bytes());
        let file = parse(&data);
        let face = &file.faces[0];
        let reason = broken_reason(face);
        assert!(
            reason.contains("GSUB feature list is malformed"),
            "{reason}"
        );
        // GPOS features are still read.
        assert!(
            face.features.iter().any(|f| f == "kern"),
            "{:?}",
            face.features
        );
    }

    #[test]
    fn data_that_is_not_a_font_is_broken() {
        let cases: [&[u8]; 5] = [b"", b"ab", b"\0\0\0", b"not a font at all", &[0xff; 64]];
        for data in cases {
            let file = parse(data);
            assert!(
                matches!(file.health, Health::Broken(_)),
                "{data:?}: {:?}",
                file.health
            );
            assert!(file.faces.is_empty());
        }
    }

    #[test]
    fn a_bad_header_after_good_magic_is_broken() {
        let cases: [&[u8]; 4] = [
            b"OTTO",
            b"\0\x01\0\0\x00\xff",
            b"ttcf",
            b"ttcf\0\x01\0\0\xff\xff\xff\xff",
        ];
        for data in cases {
            let file = parse(data);
            assert!(
                matches!(file.health, Health::Broken(_)),
                "{data:?}: {:?}",
                file.health
            );
        }
    }

    #[test]
    fn a_collection_with_one_bad_face_keeps_the_good_one() {
        let mut data = read("derived/agdasima/agdasima.ttc");
        // Point the second table directory offset past the end of the file.
        data[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        let file = parse(&data);
        assert_eq!(file.health, Health::Ok);
        assert_eq!(file.faces.len(), 2);
        assert_eq!(file.faces[0].health, Health::Ok);
        assert!(matches!(file.faces[1].health, Health::Broken(_)));
    }

    #[test]
    fn formats_fontations_does_not_read_are_unsupported() {
        let cases = [
            (
                read("fontshare/archivo/Archivo-Bold.woff2"),
                FontFormat::Woff2,
            ),
            (
                read("fontshare/azeret-mono/AzeretMono-ExtraBoldItalic.woff"),
                FontFormat::Woff,
            ),
            (
                read("type1/inconsolata/Inconsolata-zi4r.pfb"),
                FontFormat::Type1,
            ),
            (b"%!PS-AdobeFont-1.0: Test".to_vec(), FontFormat::Type1),
            (vec![0, 0, 1, 0, 0, 0, 0, 0], FontFormat::Dfont),
        ];
        for (data, format) in cases {
            let file = parse(&data);
            assert_eq!(file.format, Some(format));
            assert!(matches!(file.health, Health::Unsupported(_)), "{format:?}");
            assert!(file.faces.is_empty());
        }
    }

    #[test]
    fn every_prefix_of_a_font_parses_without_panicking() {
        let data = read("derived/agdasima/agdasima.ttc");
        for len in (0..data.len()).step_by(97) {
            let _ = parse(&data[..len]);
        }
        let data = read("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        for len in (0..data.len()).step_by(1009) {
            let _ = parse(&data[..len]);
        }
    }

    #[test]
    fn flipped_bytes_in_the_tables_parse_without_panicking() {
        let original = read("google-repo/asapsharp/AsapSharp[wdth,wght].ttf");
        for i in (0..original.len()).step_by(211) {
            let mut data = original.clone();
            data[i] ^= 0xff;
            let _ = parse(&data);
        }
    }

    #[test]
    fn every_sfnt_fixture_parses_with_names() {
        let manifest = fs::read_to_string(fixture("manifest.tsv")).expect("manifest");
        let mut typographic = 0;
        for line in manifest.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            let (kind, path) = (cols[3], cols[4]);
            let file = parse(&read(path));
            match kind {
                "broken" => assert!(matches!(file.health, Health::Broken(_)), "{path}"),
                "woff" | "woff2" | "type1" => {
                    assert!(matches!(file.health, Health::Unsupported(_)), "{path}");
                }
                _ => {
                    assert_eq!(file.health, Health::Ok, "{path}");
                    assert!(!file.faces.is_empty(), "{path}");
                    for face in &file.faces {
                        assert_eq!(face.health, Health::Ok, "{path}");
                        assert!(face.family.is_some(), "{path}");
                        assert!(face.postscript_name.is_some(), "{path}");
                        assert!(!face.code_points.is_empty(), "{path}");
                        typographic += usize::from(face.typographic_family.is_some());
                    }
                    if kind == "variable" {
                        assert!(file.faces[0].is_variable(), "{path}");
                    }
                }
            }
        }
        assert!(typographic > 0, "no fixture face has a typographic family");
    }

    #[test]
    fn parse_file_reads_a_non_ascii_path() {
        let dir = TempDir::new("parse-unicode");
        let path = dir.path().join("Шрифт ü 字体.ttc");
        fs::copy(fixture("derived/agdasima/agdasima.ttc"), &path).expect("copy");
        let file = parse_file(&path).expect("parse");
        assert_eq!(file.faces.len(), 2);
    }

    #[test]
    fn parse_file_reports_a_missing_file_as_not_found() {
        let dir = TempDir::new("parse-missing");
        let err = parse_file(&dir.path().join("gone.ttf")).expect_err("missing");
        assert_eq!(err.code(), ErrorCode::NotFound);
    }

    #[test]
    fn an_empty_file_is_broken() {
        let dir = TempDir::new("parse-empty");
        let path = dir.path().join("empty.ttf");
        fs::write(&path, b"").expect("write");
        let file = parse_file(&path).expect("parse");
        assert!(matches!(file.health, Health::Broken(_)));
    }
}
