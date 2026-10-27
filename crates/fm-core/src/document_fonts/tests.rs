#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;
use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use fm_api::workflows::{DocumentFontRef, DocumentFormat};

use super::{Fonts, UNREADABLE, document_fonts, format_of, read, xml};
use crate::test_support::TempDir;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/documents")
        .join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).expect("fixture (node scripts/build-document-fixtures.mjs)")
}

fn ps(name: &str) -> DocumentFontRef {
    DocumentFontRef {
        postscript_name: Some(name.into()),
        family: None,
        style: None,
    }
}

fn family(name: &str) -> DocumentFontRef {
    DocumentFontRef {
        postscript_name: None,
        family: Some(name.into()),
        style: None,
    }
}

fn full(ps: Option<&str>, family: &str, style: &str) -> DocumentFontRef {
    DocumentFontRef {
        postscript_name: ps.map(Into::into),
        family: Some(family.into()),
        style: Some(style.into()),
    }
}

fn fonts_of(name: &str) -> Vec<DocumentFontRef> {
    let doc = document_fonts(&fixture(name)).unwrap();
    assert_eq!(Some(doc.format), format_of(Path::new(name)));
    doc.fonts
}

#[test]
fn docx_reads_the_font_table_and_every_run_font() {
    // fontTable: Calibri, Cambria, Arial; document runs: Inter, Noto Sans JP,
    // Arial (cs); styles: Georgia. The theme's "Calibri Light" is not named
    // by any run, and `w:asciiTheme` is not a face.
    assert_eq!(
        fonts_of("sample.docx"),
        [
            "Arial",
            "Calibri",
            "Cambria",
            "Georgia",
            "Inter",
            "Noto Sans JP"
        ]
        .map(family)
    );
}

#[test]
fn pptx_resolves_theme_placeholders_through_layout_and_master() {
    // Slide: Roboto, +mn-ea → Noto Sans KR, +mj-lt → Montserrat; master:
    // +mn-lt → Open Sans, +mj-ea → "" (nothing). theme2 is used by no master.
    assert_eq!(
        fonts_of("sample.pptx"),
        ["Montserrat", "Noto Sans KR", "Open Sans", "Roboto"].map(family)
    );
}

#[test]
fn sketch_reads_text_layer_font_attributes_of_every_page() {
    assert_eq!(
        fonts_of("sample.sketch"),
        ["Inter-Bold", "Inter-Regular", "SFProText-Semibold"].map(ps)
    );
}

#[test]
fn fig_reads_font_names_from_the_kiwi_data() {
    assert_eq!(
        fonts_of("sample.fig"),
        [
            full(None, "Roboto Mono", "Regular"),
            full(Some("Inter-Bold"), "Inter", "Bold"),
            full(Some("Inter-Regular"), "Inter", "Regular"),
        ]
    );
}

#[test]
fn indd_reads_the_xmp_font_list_in_both_rdf_forms() {
    assert_eq!(
        fonts_of("sample.indd"),
        [
            full(Some("Brand&Co-Sans"), "Brand&Co Sans", "Book"),
            full(Some("MinionPro-Regular"), "Minion Pro", "Regular"),
            full(Some("MyriadPro-Bold"), "Myriad Pro", "Bold"),
        ]
    );
}

#[test]
fn ai_reads_xmp_fonts_and_pdf_base_fonts_inside_object_streams() {
    // /ABCDEF+MinionPro-Regular folds into the XMP entry; #20 is a space;
    // Lato-Bold is only in the Flate-compressed object stream.
    assert_eq!(
        fonts_of("sample.ai"),
        [
            ps("Lato-Bold"),
            full(Some("MinionPro-Regular"), "Minion Pro", "Regular"),
            ps("Source Sans Pro"),
        ]
    );
}

#[test]
fn psd_reads_the_font_set_of_every_text_layer() {
    // AdobeInvisFont is Photoshop's placeholder; "Normal RGB" is a style
    // sheet name outside the FontSet.
    assert_eq!(
        fonts_of("sample.psd"),
        ["ArialMT", "Georgia", "Inter-Bold"].map(ps)
    );
}

const SAMPLES: [&str; 7] = [
    "sample.docx",
    "sample.pptx",
    "sample.sketch",
    "sample.fig",
    "sample.indd",
    "sample.ai",
    "sample.psd",
];

fn format(name: &str) -> DocumentFormat {
    format_of(Path::new(name)).unwrap()
}

#[test]
fn every_prefix_of_every_fixture_gives_a_result_and_never_panics() {
    for name in SAMPLES {
        let data = bytes(name);
        let step = (data.len() / 300).max(1);
        for cut in (0..data.len()).step_by(step) {
            if let Err(e) = read(format(name), &mut Cursor::new(&data[..cut])) {
                assert_eq!(e.message(), UNREADABLE, "{name} cut at {cut}");
            }
        }
    }
}

#[test]
fn a_truncated_document_is_an_error_with_a_plain_message() {
    for name in SAMPLES {
        let data = bytes(name);
        let mut cuts = vec![
            0,
            data.len() / 4,
            data.len() / 2,
            data.len() * 3 / 4,
            data.len() - 4,
        ];
        if name.ends_with(".indd") {
            // Inside the XMP object, and right after the database pages
            // (the XMP object is gone, nothing else is incomplete).
            let xmp = data.windows(10).position(|w| w == b"<x:xmpmeta").unwrap();
            cuts.extend([xmp + 40, 3 * 4096]);
        }
        for cut in cuts {
            let e = read(format(name), &mut Cursor::new(&data[..cut]))
                .expect_err(&format!("{name} cut at {cut}"));
            assert_eq!(e.message(), UNREADABLE);
            assert_eq!(e.code(), ErrorCode::InvalidRequest, "{name} cut at {cut}");
        }
    }
}

#[test]
fn an_indd_whose_page_count_exceeds_the_file_is_truncated() {
    let mut data = bytes("sample.indd");
    // The current master page (higher sequence number) is the second one.
    data[4096 + 280..4096 + 284].copy_from_slice(&99u32.to_le_bytes());
    let e = read(DocumentFormat::Indd, &mut Cursor::new(&data)).unwrap_err();
    assert_eq!(e.code(), ErrorCode::InvalidRequest);
    assert!(e.detail().unwrap().contains("truncated"), "{e:?}");
    // The older master page's count is ignored.
    let mut data = bytes("sample.indd");
    data[280..284].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        read(DocumentFormat::Indd, &mut Cursor::new(&data))
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn malformed_bytes_are_an_error_for_every_format() {
    let junk: Vec<u8> = (0..4096u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    for name in SAMPLES {
        for input in [&[][..], &junk[..]] {
            let e = read(format(name), &mut Cursor::new(input)).unwrap_err();
            assert_eq!(e.message(), UNREADABLE, "{name}");
        }
    }
}

#[test]
fn a_corrupt_deflate_stream_is_an_error() {
    let mut data = bytes("sample.docx");
    // word/document.xml is the first deflated entry: damage its first bytes.
    let at = data
        .windows(17)
        .position(|w| w == b"word/document.xml")
        .unwrap()
        + 17;
    for b in &mut data[at..at + 24] {
        *b = 0xFF;
    }
    let e = read(DocumentFormat::Docx, &mut Cursor::new(&data)).unwrap_err();
    assert_eq!(e.message(), UNREADABLE);
}

#[test]
fn encrypted_documents_are_unsupported() {
    for name in ["encrypted.docx", "encrypted.ai"] {
        let e = document_fonts(&fixture(name)).unwrap_err();
        assert_eq!(e.code(), ErrorCode::Unsupported, "{name}");
        assert_eq!(e.message(), UNREADABLE);
        assert!(
            e.detail().unwrap().contains("encrypt") || e.detail().unwrap().contains("password")
        );
    }
}

#[test]
fn a_zstd_compressed_fig_is_unsupported_with_a_plain_message() {
    let e = document_fonts(&fixture("zstd.fig")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Unsupported);
    assert_eq!(e.message(), UNREADABLE);
    assert!(e.detail().unwrap().contains("zstd"));
}

#[test]
fn an_unknown_extension_and_a_missing_file_are_errors() {
    let e = document_fonts(&fixture("README.md")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Unsupported);
    assert_eq!(e.message(), UNREADABLE);
    let e = document_fonts(&fixture("missing.psd")).unwrap_err();
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert_eq!(e.message(), UNREADABLE);
}

#[test]
fn the_document_is_only_read_even_under_a_non_ascii_path() {
    let dir = TempDir::new("docfonts");
    let sub = dir.path().join("Ünïcødé 文書");
    std::fs::create_dir_all(&sub).unwrap();
    let path = sub.join("Brief.PSD");
    std::fs::copy(fixture("sample.psd"), &path).unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let doc = document_fonts(&path).unwrap();
    assert_eq!(doc.format, DocumentFormat::Psd);
    assert_eq!(doc.fonts.len(), 3);
    assert_eq!(std::fs::read(&path).unwrap(), bytes("sample.psd"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(std::fs::read_dir(&sub).unwrap().count(), 1);
}

#[test]
fn references_are_deduplicated_and_partial_ones_fold_into_full_ones() {
    let mut f = Fonts::default();
    f.postscript("Inter-Bold");
    f.postscript(" Inter-Bold ");
    f.add(Some("Inter-Bold"), Some("Inter"), Some("Bold"));
    f.family("Inter");
    f.family("Lora");
    f.add(Some(""), Some("  "), None);
    assert_eq!(
        f.finish(),
        [family("Lora"), full(Some("Inter-Bold"), "Inter", "Bold")]
    );
}

#[test]
fn xml_decodes_entities_cdata_and_skips_comments_and_declarations() {
    let doc = r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY e "v">]><!-- <no/> -->
<a:b k='1 &amp; 2' w:x="&#x41;&#66;"><![CDATA[<raw>]]>t&lt;&e;</a:b>"#;
    let ev = xml::parse(doc).unwrap();
    let start = ev
        .iter()
        .find_map(|e| match e {
            xml::Event::Start { name, attrs, .. } => Some((*name, attrs)),
            _ => None,
        })
        .unwrap();
    assert_eq!(start.0, "a:b");
    assert_eq!(xml::attr(start.1, "k"), Some("1 & 2"));
    assert_eq!(xml::attr(start.1, "x"), Some("AB"));
    let text: String = ev
        .iter()
        .filter_map(|e| match e {
            xml::Event::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text.trim(), "<raw>t<&e;");
    for bad in [
        "<a",
        "<a k=v>",
        "<a k=\"1>",
        "<!-- open",
        "&#xFFFFFFFF;",
        "<>",
    ] {
        assert!(xml::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn mutated_fixtures_never_panic() {
    // A tiny in-process fuzz pass (the cargo-fuzz target goes further):
    // overwrite a few bytes of each fixture, pseudo-randomly but repeatably.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for name in SAMPLES {
        let original = bytes(name);
        for _ in 0..400 {
            let mut data = original.clone();
            for _ in 0..1 + next() % 4 {
                let at = (next() % data.len() as u64) as usize;
                data[at] = next() as u8;
            }
            if let Err(e) = read(format(name), &mut Cursor::new(&data)) {
                assert_eq!(e.message(), UNREADABLE);
            }
        }
    }
}
