#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Cursor;
use std::path::{Path, PathBuf};

use fm_api::ErrorCode;
use fm_api::workflows::{DocumentFontRef, DocumentFormat};

use super::{Fonts, UNREADABLE, UNSUPPORTED, check, document_fonts, format_of, psd, read, xml};
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
    // E32.4: only the fonts the text runs use. Both font sets also hold
    // MyriadPro-Regular (Photoshop's default), which no run uses: the Title
    // layer's default sheet names it but every run has its own font, and the
    // Body layer's font-less run takes the sheet `TheNormalStyleSheet 1`
    // names (Georgia), not the first one (MyriadPro-Regular).
    // AdobeInvisFont is Photoshop's placeholder; "Normal RGB" is a style
    // sheet name outside the FontSet. The global blocks after the layers
    // have lengths of 77 and 10 bytes padded to 4 (Photoshop 2025, E36.6):
    // the walk must not stop at them.
    assert_eq!(
        fonts_of("sample.psd"),
        ["ArialMT", "Georgia", "Inter-Bold"].map(ps)
    );
}

/// Engine data with a `FontSet` of `fonts`, `sheets` (the `/Font` of each
/// style sheet, `None` for a sheet without one) and `runs` (the same for
/// each run); `runs: None` leaves the run array out.
fn engine(fonts: &[&str], sheets: &[Option<&str>], runs: Option<&[Option<&str>]>) -> Vec<u8> {
    let data = |font: &Option<&str>| {
        format!(
            "<< /StyleSheetData << {} /FontSize 12 >> >>",
            font.map(|f| format!("/Font {f}")).unwrap_or_default()
        )
    };
    let runs = runs.map_or(String::new(), |runs| {
        let runs: Vec<String> = runs
            .iter()
            .map(|r| format!("<< /StyleSheet {} >>", data(r)))
            .collect();
        format!("/StyleRun << /RunArray [ {} ] >>", runs.join(" "))
    });
    let sheets: Vec<String> = sheets
        .iter()
        .map(|s| {
            let font = s.map(|f| format!("/Font {f}")).unwrap_or_default();
            format!("<< /Name (Normal RGB) /StyleSheetData << {font} /FontSize 12 >> >>")
        })
        .collect();
    let set: Vec<String> = fonts
        .iter()
        .map(|f| format!("<< /Name ({f}) /Script 0 >>"))
        .collect();
    format!(
        "\n\n<< /EngineDict << /Editor << /Text (Hi) >> {runs} /AntiAlias 4 >> \
         /ResourceDict << /StyleSheetSet [ {} ] /FontSet [ {} ] /SuperscriptSize .583 >> >>",
        sheets.join(" "),
        set.join(" ")
    )
    .into_bytes()
}

/// `engine` data inside a `TySh` block, as its text descriptor holds it.
fn psd_text_fonts(engine: &[u8]) -> Vec<DocumentFontRef> {
    let mut block = vec![0u8; 56];
    block.extend(b"\0\0\0\nEngineDatatdta");
    block.extend(u32::try_from(engine.len()).unwrap().to_be_bytes());
    block.extend(engine);
    block.extend(b"\0\0\0\x01warp");
    let mut f = Fonts::default();
    psd::text_fonts(&block, &mut f);
    f.0.into_iter().collect()
}

const PSD_SET: [&str; 4] = [
    "Actor-Regular",
    "AdobeInvisFont",
    "MyriadPro-Regular",
    "Lato-Bold",
];

#[test]
fn psd_lists_the_run_fonts_and_the_default_sheet_for_a_run_without_one() {
    // A real Photoshop 2025 layer in Actor: Myriad Pro is in the FontSet and
    // the default sheet, but every run has its own font.
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[Some("2")], Some(&[Some("0")]))),
        [ps("Actor-Regular")]
    );
    // A run without a font takes the default (first) sheet's.
    assert_eq!(
        psd_text_fonts(&engine(
            &PSD_SET,
            &[Some("3"), Some("2")],
            Some(&[Some("0"), None])
        )),
        [ps("Actor-Regular"), ps("Lato-Bold")]
    );
    // A run in AdobeInvisFont is still left out.
    assert_eq!(
        psd_text_fonts(&engine(
            &PSD_SET,
            &[Some("2")],
            Some(&[Some("1"), Some("3")])
        )),
        [ps("Lato-Bold")]
    );
}

#[test]
fn psd_falls_back_to_every_font_set_name_when_the_runs_cannot_be_followed() {
    let all = || ["Actor-Regular", "Lato-Bold", "MyriadPro-Regular"].map(ps);
    // No run array.
    assert_eq!(psd_text_fonts(&engine(&PSD_SET, &[Some("2")], None)), all());
    // An empty run array.
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[Some("2")], Some(&[]))),
        all()
    );
    // An index outside the FontSet, in a run or in the default sheet.
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[Some("2")], Some(&[Some("4")]))),
        all()
    );
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[Some("9")], Some(&[None]))),
        all()
    );
    // A run without a font and no default sheet font; a font that is not an index.
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[None], Some(&[None]))),
        all()
    );
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[], Some(&[Some("-1")]))),
        all()
    );
    assert_eq!(
        psd_text_fonts(&engine(&PSD_SET, &[], Some(&[Some("0.5")]))),
        all()
    );
    // Engine data that does not parse: cut short, or a stray closing bracket.
    let good = engine(&PSD_SET, &[Some("2")], Some(&[Some("0")]));
    let cut = &good[..good.len() - 3];
    assert_eq!(psd_text_fonts(cut), all());
    let mut stray = good.clone();
    stray.splice(2..2, *b"] ");
    assert_eq!(psd_text_fonts(&stray), all());
    // A block without an `EngineData` item, or whose length runs past it.
    let mut f = Fonts::default();
    psd::text_fonts(&good, &mut f);
    assert_eq!(f.0.into_iter().collect::<Vec<_>>(), all());
    let mut f = Fonts::default();
    let mut long = b"EngineDatatdta\xFF\xFF\xFF\xFF".to_vec();
    long.extend(&good);
    psd::text_fonts(&long, &mut f);
    assert_eq!(f.0.into_iter().collect::<Vec<_>>(), all());
    // Nesting deeper than the parser follows.
    let mut deep = b"<< /EngineDict ".to_vec();
    deep.extend(b"[ ".repeat(200));
    deep.extend(b"] ".repeat(200));
    deep.extend(b">> /ResourceDict << /FontSet [ << /Name (Lato-Bold) >> ] >> >>");
    assert_eq!(psd_text_fonts(&deep), [ps("Lato-Bold")]);
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
    assert_eq!(e.message(), UNSUPPORTED);
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

/// E32.2: a matched font is active or in the library by its face; one
/// without a match is missing; two fonts on one inactive face activate it once.
#[test]
fn check_marks_each_font_active_in_library_or_missing() {
    use std::collections::HashSet;

    use fm_api::FaceId;
    use fm_api::activation::{NeededFontMatch, NeededFontRule};
    use fm_api::workflows::{DocumentFontStatus, DocumentFonts};

    let fonts = vec![
        ps("Active-Regular"),
        ps("Inter-Bold"),
        family("Inter"),
        ps("Gone"),
    ];
    let doc = DocumentFonts {
        format: DocumentFormat::Psd,
        fonts: fonts.clone(),
    };
    let hit = |font: &DocumentFontRef, face: u32| NeededFontMatch {
        font: super::needed_font(font),
        face: FaceId(face),
        matched_by: NeededFontRule::PostscriptName,
    };
    let matched = [hit(&fonts[0], 1), hit(&fonts[1], 2), hit(&fonts[2], 2)];
    let active: HashSet<FaceId> = [FaceId(1)].into_iter().collect();
    let got = check(doc, &matched, &active);
    assert_eq!(got.format, DocumentFormat::Psd);
    let statuses: Vec<_> = got.fonts.iter().map(|f| (f.status, f.face)).collect();
    assert_eq!(
        statuses,
        [
            (DocumentFontStatus::Active, Some(FaceId(1))),
            (DocumentFontStatus::InLibrary, Some(FaceId(2))),
            (DocumentFontStatus::InLibrary, Some(FaceId(2))),
            (DocumentFontStatus::Missing, None),
        ]
    );
    assert_eq!(got.fonts[3].font, fonts[3]);
    assert_eq!(got.to_activate, [FaceId(2)]);
}
