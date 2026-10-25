use std::path::{Path, PathBuf};

use read_fonts::{FileRef, FontRef};

use super::*;
use crate::test_support::TempDir;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/fonts")
        .join(rel)
}

const STATIC_TTF: &str = "google-repo/bebasneue/BebasNeue-Regular.ttf";
const CFF_OTF: &str = "fontshare/asap/Asap-SemiBold.otf";
const VARIABLE: &str = "google-repo/asapsharp/AsapSharp[wdth,wght].ttf";
const TTC: &str = "derived/agdasima/agdasima.ttc";
const WOFF: &str = "fontshare/azeret-mono/AzeretMono-ExtraBoldItalic.woff";
const WOFF2: &str = "fontshare/archivo/Archivo-Bold.woff2";
const TYPE1: &str = "type1/inconsolata/Inconsolata-zi4r.pfb";

/// The source face's tables as stored in its file (a WOFF decoded first),
/// by tag.
fn source_tables(data: &[u8], index: u32) -> Vec<([u8; 4], Vec<u8>)> {
    let sfnt = glyphs::sfnt(data).unwrap();
    let font = match FileRef::new(&sfnt).unwrap() {
        FileRef::Font(f) => f,
        FileRef::Collection(c) => c.get(index).unwrap(),
    };
    let mut out: Vec<_> = font
        .table_directory()
        .table_records()
        .iter()
        .map(|r| {
            (
                r.tag().to_be_bytes(),
                font.table_data(r.tag()).unwrap().as_bytes().to_vec(),
            )
        })
        .collect();
    out.sort();
    out
}

fn decoded_tables(sfnt: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let font = FontRef::new(sfnt).unwrap();
    let mut out: Vec<_> = font
        .table_directory()
        .table_records()
        .iter()
        .map(|r| {
            (
                r.tag().to_be_bytes(),
                font.table_data(r.tag()).unwrap().as_bytes().to_vec(),
            )
        })
        .collect();
    out.sort();
    out
}

/// `head` without its `checkSumAdjustment`, which belongs to the file
/// layout, not to the face.
fn without_adjustment(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<([u8; 4], Vec<u8>)> {
    for (tag, data) in &mut tables {
        if tag == b"head" {
            data[8..12].fill(0);
        }
    }
    tables
}

fn decode(bytes: &[u8], format: WebFormat) -> Vec<u8> {
    match format {
        WebFormat::Woff2 => {
            assert_eq!(&bytes[..4], b"wOF2");
            wuff::decompress_woff2(bytes).unwrap()
        }
        WebFormat::Woff => {
            assert_eq!(&bytes[..4], b"wOFF");
            wuff::decompress_woff1(bytes).unwrap()
        }
    }
}

#[test]
fn every_output_decodes_to_the_source_tables() {
    let cases = [
        (STATIC_TTF, 0, 0x0001_0000),
        (CFF_OTF, 0, u32::from_be_bytes(*b"OTTO")),
        (VARIABLE, 0, 0x0001_0000),
        (TTC, 1, 0x0001_0000),
        (WOFF, 0, 0),
        (WOFF2, 0, 0),
    ];
    for (rel, index, flavor) in cases {
        let data = fs::read(fixture(rel)).unwrap();
        let sfnt = standalone(&data, index).unwrap();
        if flavor != 0 {
            assert_eq!(sfnt.flavor, flavor, "{rel}");
        }
        let source = source_tables(&data, index);
        for format in [WebFormat::Woff2, WebFormat::Woff] {
            let bytes = encode(&sfnt, format).unwrap();
            assert_eq!(bytes.len() % 4, 0, "{rel} {format:?}");
            let length = u32::from_be_bytes(bytes[8..12].try_into().unwrap());
            assert_eq!(length as usize, bytes.len(), "{rel} {format:?}");
            let decoded = decode(&bytes, format);
            assert_eq!(
                without_adjustment(decoded_tables(&decoded)),
                without_adjustment(source.clone()),
                "{rel} {format:?}"
            );
            assert_eq!(
                &decoded[..4],
                &sfnt.flavor.to_be_bytes(),
                "{rel} {format:?}"
            );
            assert_eq!(
                checksum(&decoded),
                0xB1B0_AFBA,
                "{rel} {format:?}: checkSumAdjustment"
            );
        }
    }
}

#[test]
fn a_collection_face_is_written_with_only_its_own_tables() {
    let data = fs::read(fixture(TTC)).unwrap();
    for index in [0, 1] {
        let sfnt = standalone(&data, index).unwrap();
        let bytes = sfnt_bytes(&sfnt).unwrap();
        let font = FontRef::new(&bytes).unwrap();
        let name = read_fonts::TableProvider::name(&font).unwrap();
        let ps = name
            .name_record()
            .iter()
            .find(|r| r.name_id() == read_fonts::types::NameId::POSTSCRIPT_NAME)
            .and_then(|r| r.string(name.string_data()).ok())
            .map(|s| s.to_string())
            .unwrap();
        let want = if index == 0 {
            "Agdasima-Bold"
        } else {
            "Agdasima-Regular"
        };
        assert_eq!(ps, want);
        assert_eq!(&bytes[..4], &[0, 1, 0, 0]);
    }
    let err = standalone(&data, 2).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Unsupported);
}

#[test]
fn compressed_outputs_are_smaller_than_the_font() {
    let data = fs::read(fixture(STATIC_TTF)).unwrap();
    let sfnt = standalone(&data, 0).unwrap();
    let woff2 = encode(&sfnt, WebFormat::Woff2).unwrap();
    let woff = encode(&sfnt, WebFormat::Woff).unwrap();
    assert!(woff2.len() < woff.len(), "{} < {}", woff2.len(), woff.len());
    assert!(woff.len() < data.len(), "{} < {}", woff.len(), data.len());
}

#[test]
fn type1_broken_and_non_font_data_are_errors_not_panics() {
    let type1 = fs::read(fixture(TYPE1)).unwrap();
    let err = standalone(&type1, 0).unwrap_err();
    assert_eq!(err.code(), ErrorCode::Unsupported);
    assert!(err.message().contains("Type 1"), "{err:?}");

    let truncated = fs::read(fixture("derived/agdasima/agdasima-truncated.ttf")).unwrap();
    let good = fs::read(fixture(STATIC_TTF)).unwrap();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("text", b"hello world".to_vec()),
        ("truncated fixture", truncated),
        ("header only", good[..12].to_vec()),
        ("cut in a table", good[..good.len() / 2].to_vec()),
        ("bad woff", b"wOFF\0\0\0\0garbage".to_vec()),
        ("bad woff2", b"wOF2\0\0\0\0garbage".to_vec()),
        ("bad collection", b"ttcf\0\x01\0\0\0\0\0\x05".to_vec()),
    ];
    for (what, data) in cases {
        let err = standalone(&data, 0).unwrap_err();
        assert_eq!(err.code(), ErrorCode::Unsupported, "{what}: {err:?}");
        assert!(!err.message().is_empty(), "{what}");
    }
}

#[test]
fn base128_matches_the_woff2_examples() {
    let enc = |v| {
        let mut out = Vec::new();
        base128(&mut out, v);
        out
    };
    assert_eq!(enc(0), [0x00]);
    assert_eq!(enc(127), [0x7f]);
    assert_eq!(enc(128), [0x81, 0x00]);
    assert_eq!(enc(63_000), [0x83, 0xEC, 0x18]);
    assert_eq!(enc(u32::MAX), [0x8f, 0xff, 0xff, 0xff, 0x7f]);
}

#[test]
fn file_stems_are_safe_on_every_file_system() {
    assert_eq!(file_stem("Inter-Bold", 1), "Inter-Bold");
    assert_eq!(file_stem("A/B:C*D?\"<>|\\", 1), "A_B_C_D______");
    assert_eq!(file_stem("Schrift Grün", 1), "Schrift_Gr_n");
    assert_eq!(file_stem("", 7), "face-7");
}

fn face(rel: &str, index: u32, stem: &str) -> ExportFace {
    ExportFace {
        path: fixture(rel),
        collection_index: index,
        file_stem: stem.into(),
        label: stem.into(),
    }
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn export_writes_every_face_in_every_format_and_leaves_the_sources_alone() {
    let out = TempDir::new("web-export");
    let faces = [
        face(STATIC_TTF, 0, "BebasNeue-Regular"),
        face(TTC, 1, "Agdasima-Regular"),
    ];
    let before: Vec<_> = faces.iter().map(|f| fs::read(&f.path).unwrap()).collect();
    let files = export(
        &faces,
        &[WebFormat::Woff2, WebFormat::Woff],
        None,
        out.path(),
    )
    .unwrap();
    let names: Vec<_> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "BebasNeue-Regular.woff2",
            "BebasNeue-Regular.woff",
            "Agdasima-Regular.woff2",
            "Agdasima-Regular.woff"
        ]
    );
    for f in &files {
        assert!(f.is_absolute(), "{}", f.display());
        assert!(f.is_file(), "{}", f.display());
    }
    // No temporary files are left.
    assert_eq!(listing(out.path()).len(), 4);
    let after: Vec<_> = faces.iter().map(|f| fs::read(&f.path).unwrap()).collect();
    assert_eq!(before, after);
    // Each output decodes to its face.
    let decoded = wuff::decompress_woff2(&fs::read(&files[2]).unwrap()).unwrap();
    assert_eq!(
        without_adjustment(decoded_tables(&decoded)),
        without_adjustment(source_tables(&fs::read(fixture(TTC)).unwrap(), 1))
    );
}

#[test]
fn an_existing_target_refuses_the_export_and_is_kept() {
    let out = TempDir::new("web-export-clash");
    let clash = out.path().join("Agdasima-Regular.woff");
    fs::write(&clash, b"mine").unwrap();
    let faces = [
        face(STATIC_TTF, 0, "BebasNeue-Regular"),
        face(TTC, 1, "Agdasima-Regular"),
    ];
    let err = export(
        &faces,
        &[WebFormat::Woff2, WebFormat::Woff],
        None,
        out.path(),
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().contains("Agdasima-Regular.woff"), "{err:?}");
    assert_eq!(listing(out.path()), ["Agdasima-Regular.woff"]);
    assert_eq!(fs::read(&clash).unwrap(), b"mine");
}

#[test]
fn a_failing_face_leaves_no_files() {
    let out = TempDir::new("web-export-fail");
    let faces = [
        face(STATIC_TTF, 0, "BebasNeue-Regular"),
        face(CFF_OTF, 0, "Asap-SemiBold"),
        face(TYPE1, 0, "Inconsolata"),
    ];
    let err = export(
        &faces,
        &[WebFormat::Woff2, WebFormat::Woff],
        None,
        out.path(),
    )
    .unwrap_err();
    assert_eq!(err.code(), ErrorCode::Unsupported);
    assert!(err.message().starts_with("Inconsolata: "), "{err:?}");
    assert!(err.message().contains("Type 1"), "{err:?}");
    assert!(listing(out.path()).is_empty(), "{:?}", listing(out.path()));
}

#[test]
fn a_missing_source_file_leaves_no_files() {
    let out = TempDir::new("web-export-missing");
    let mut gone = face(STATIC_TTF, 0, "Gone");
    gone.path = out.path().join("elsewhere").join("gone.ttf");
    let faces = [face(STATIC_TTF, 0, "BebasNeue-Regular"), gone];
    let err = export(&faces, &[WebFormat::Woff2], None, out.path()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    assert!(err.message().contains("Gone"), "{err:?}");
    assert!(listing(out.path()).is_empty());
}

#[test]
fn two_faces_with_one_name_are_refused_before_writing() {
    let out = TempDir::new("web-export-same-name");
    let faces = [face(STATIC_TTF, 0, "Same"), face(CFF_OTF, 0, "same")];
    let err = export(&faces, &[WebFormat::Woff2], None, out.path()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().contains("same.woff2"), "{err:?}");
    assert!(listing(out.path()).is_empty());
}

#[test]
fn a_missing_or_file_output_folder_is_an_error() {
    let out = TempDir::new("web-export-no-dir");
    let faces = [face(STATIC_TTF, 0, "BebasNeue-Regular")];
    let err = export(&faces, &[WebFormat::Woff2], None, &out.path().join("nope")).unwrap_err();
    assert_eq!(err.code(), ErrorCode::NotFound);
    let file = out.path().join("file");
    fs::write(&file, b"x").unwrap();
    let err = export(&faces, &[WebFormat::Woff2], None, &file).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[test]
fn a_target_that_appears_before_the_rename_undoes_the_export() {
    let out = TempDir::new("web-export-race");
    let mut written = Written::default();
    let a = out.path().join("A.woff2");
    let b = out.path().join("B.woff2");
    written.temp(&a, b"a").unwrap();
    written.temp(&b, b"b").unwrap();
    fs::write(&b, b"theirs").unwrap();
    let err = written.commit().unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert_eq!(listing(out.path()), ["B.woff2"]);
    assert_eq!(fs::read(&b).unwrap(), b"theirs");
}

#[test]
fn replace_writes_over_an_existing_target_and_refuse_does_not() {
    let out = TempDir::new("web-export-replace");
    let faces = [face(STATIC_TTF, 0, "BebasNeue-Regular")];
    let clash = out.path().join("BebasNeue-Regular.woff2");
    fs::write(&clash, b"mine").unwrap();
    let refused = write(
        &faces,
        None,
        &[WebFormat::Woff2],
        None,
        out.path(),
        Existing::Refuse,
    )
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidRequest);
    assert_eq!(fs::read(&clash).unwrap(), b"mine");

    let written = write(
        &faces,
        None,
        &[WebFormat::Woff2],
        None,
        out.path(),
        Existing::Replace,
    )
    .unwrap();
    assert_eq!(written.files, [std::path::absolute(&clash).unwrap()]);
    assert_eq!(written.css, None);
    assert_eq!(&fs::read(&clash).unwrap()[..4], b"wOF2");
    assert_eq!(listing(out.path()), ["BebasNeue-Regular.woff2"]);
}

#[test]
fn a_failed_replacing_commit_keeps_the_replaced_file_and_removes_the_new_ones() {
    let out = TempDir::new("web-export-replace-fail");
    let mut written = Written::default();
    written.existing = Existing::Replace;
    let (a, b, c) = (
        out.path().join("A.woff2"),
        out.path().join("B.woff2"),
        out.path().join("C.woff2"),
    );
    fs::write(&b, b"theirs").unwrap();
    written.temp(&a, b"a").unwrap();
    written.temp(&b, b"b").unwrap();
    written.temp(&c, b"c").unwrap();
    // A folder where C goes makes its rename fail after A and B moved.
    fs::create_dir(&c).unwrap();
    written.commit().unwrap_err();
    let mut left = listing(out.path());
    left.sort();
    assert_eq!(left, ["B.woff2", "C.woff2"]);
    assert_eq!(fs::read(&b).unwrap(), b"b");
}

/// Latin, Cyrillic and Greek, with a `wght` axis.
const MULTI_SCRIPT: &str = "google-repo/mulish/Mulish[wght].ttf";

#[test]
fn a_latin_subset_export_is_smaller_than_the_whole_web_font() {
    let whole = TempDir::new("web-export-whole");
    let part = TempDir::new("web-export-latin");
    let faces = [face(MULTI_SCRIPT, 0, "Mulish")];
    let full = export(&faces, &[WebFormat::Woff2], None, whole.path()).unwrap();
    let latin = crate::subset::code_points(&fm_api::export::Subset::Presets {
        names: vec!["latin".into()],
    })
    .unwrap();
    let cut = export(
        &faces,
        &[WebFormat::Woff2, WebFormat::Woff],
        Some(&latin),
        part.path(),
    )
    .unwrap();
    assert_eq!(listing(part.path()), ["Mulish.woff", "Mulish.woff2"]);
    let size = |p: &Path| fs::metadata(p).unwrap().len();
    assert!(
        size(&cut[0]) < size(&full[0]),
        "{} < {}",
        size(&cut[0]),
        size(&full[0])
    );
    let decoded = decode(&fs::read(&cut[0]).unwrap(), WebFormat::Woff2);
    let font = FontRef::new(&decoded).unwrap();
    use skrifa::MetadataProvider as _;
    let charmap = font.charmap();
    assert!(charmap.map('A').is_some());
    assert!(charmap.map('Ж').is_none() && charmap.map('Ω').is_none());
}

#[test]
fn a_subset_no_face_maps_writes_nothing() {
    let out = TempDir::new("web-export-none");
    let faces = [
        face(STATIC_TTF, 0, "BebasNeue-Regular"),
        face(MULTI_SCRIPT, 0, "Mulish"),
    ];
    let cjk = crate::subset::code_points(&fm_api::export::Subset::Unicode {
        ranges: vec!["U+4E00-9FFF".into()],
    })
    .unwrap();
    let err = export(&faces, &[WebFormat::Woff2], Some(&cjk), out.path()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    assert!(err.message().starts_with("BebasNeue-Regular: "), "{err:?}");
    assert!(listing(out.path()).is_empty());

    // The first face has the characters, the second does not: the file
    // written for the first is removed.
    let cyrillic = crate::subset::code_points(&fm_api::export::Subset::Unicode {
        ranges: vec!["U+0410-044F".into()],
    })
    .unwrap();
    let faces = [
        face(MULTI_SCRIPT, 0, "Mulish"),
        face(STATIC_TTF, 0, "BebasNeue-Regular"),
    ];
    let bebas = fs::read(fixture(STATIC_TTF)).unwrap();
    let bebas = skrifa::FontRef::new(&bebas).unwrap();
    use skrifa::MetadataProvider as _;
    assert!(
        (0x410_u32..=0x44F).all(|cp| bebas.charmap().map(cp).is_none()),
        "the fixture has no Cyrillic letters"
    );
    let err = export(&faces, &[WebFormat::Woff2], Some(&cyrillic), out.path()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::InvalidRequest, "{err:?}");
    assert!(listing(out.path()).is_empty());
}
