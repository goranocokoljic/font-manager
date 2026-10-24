use std::path::Path;

use fm_api::common::AxisValue;

use super::*;
use crate::features;

/// Decodes standard base64 with padding; `None` on anything else.
fn decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::new();
    for quad in bytes.chunks(4) {
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        let mut n = 0u32;
        for &c in &quad[..4 - pad] {
            n = n << 6 | u32::from(value(c)?);
        }
        n <<= 6 * pad;
        let got = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&got[..3 - pad]);
    }
    Some(out)
}

#[test]
fn base64_matches_the_rfc_4648_test_vectors() {
    let vectors = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (plain, encoded) in vectors {
        assert_eq!(base64(plain.as_bytes()), encoded, "{plain}");
    }
    // Every byte value, at every length mod 3, round-trips.
    let all: Vec<u8> = (0..=255).collect();
    for len in [0, 1, 2, 3, 254, 255, 256] {
        let data = &all[..len.min(all.len())];
        let encoded = base64(data);
        assert!(
            encoded
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+/=".contains(&c))
        );
        assert_eq!(decode(&encoded).as_deref(), Some(data), "{len}");
    }
}

#[test]
fn font_names_are_escaped() {
    assert_eq!(
        escape(r#"<script>alert("x")</script> & 'y'"#),
        "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;y&#39;"
    );
    // A colon is escaped too, so a name never forms an address.
    assert_eq!(escape("https://example.com"), "https&#58;//example.com");
    assert_eq!(escape("Ünïcödé Ж"), "Ünïcödé Ж");
}

fn face(id: u32, style: &str, weight: u16, slant: Slant) -> SpecimenFace {
    SpecimenFace {
        id: FaceId(id),
        label: format!("Test Sans {style}"),
        style: style.to_owned(),
        weight,
        width: 5,
        slant,
        version: Some(format!("Version {id}.000")),
        designer: None,
        foundry: None,
        embedding: Embedding::Installable,
        declared: None,
        open_license: Some(OpenLicense::Ofl),
        web_use: WebReason::Open(OpenLicense::Ofl),
        instances: Vec::new(),
        features: Vec::new(),
        code_points: CodePointSet::from_code_points(
            (0x20..0x7F).chain([0xE9, 0x391, 0x3B1, 0x0301]),
        ),
        path: PathBuf::from(format!("/fonts/TestSans-{style}.ttf")),
    }
}

/// A family with tricky names: a static Light, a static Regular (the face
/// the header shows), a variable italic with named instances, and features.
fn tricky() -> Specimen {
    let mut light = face(1, "Light", 300, Slant::Upright);
    light.designer = Some("Ann <b>Designer</b> & \"Co\"".to_owned());
    light.features = vec![
        features::feature_info_named("ss01", Some("Alt <a> 'single'")),
        features::feature_info_named("ccmp", None),
    ];
    let mut regular = face(2, "Regular", 400, Slant::Upright);
    regular.foundry = Some("Foundry: https://foundry.example/".to_owned());
    regular.features = vec![
        features::feature_info_named("smcp", None),
        features::feature_info_named("ss01", Some("Never shown")),
        features::feature_info_named("kern", None),
    ];
    let mut italic = face(3, "Italic", 400, Slant::Italic);
    italic.instances = vec![
        NamedInstance {
            name: "Thin <Italic>".to_owned(),
            coordinates: vec![
                AxisValue {
                    tag: "wght".to_owned(),
                    value: 100.0,
                },
                AxisValue {
                    tag: "sl\"n".to_owned(),
                    value: -10.5,
                },
            ],
        },
        NamedInstance {
            name: "Black Italic".to_owned(),
            coordinates: vec![AxisValue {
                tag: "wght".to_owned(),
                value: 900.0,
            }],
        },
    ];
    italic.features = vec![features::feature_info_named("onum", None)];
    Specimen {
        family: "Test <Sans> & \"Friends\"".to_owned(),
        faces: vec![light, regular, italic],
    }
}

fn stub(i: usize) -> String {
    format!("data:font/ttf;base64,FACE{i}")
}

#[test]
fn the_page_matches_its_snapshot_with_the_fonts_stubbed() {
    let actual = page(&tricky(), stub).expect("page");
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/snapshots/specimen.html");
    if std::env::var_os("FM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&snapshot, &actual).expect("write snapshot");
    }
    let expected = std::fs::read_to_string(&snapshot)
        .expect("read snapshot")
        .replace("\r\n", "\n");
    assert!(
        actual == expected,
        "the specimen page changed; review and rerun with FM_UPDATE_SNAPSHOTS=1\n{actual}"
    );
}

#[test]
fn the_same_family_gives_byte_identical_pages() {
    let a = page(&tricky(), stub).expect("page");
    let b = page(&tricky(), stub).expect("page");
    assert_eq!(a, b);
}

#[test]
fn the_page_has_every_part_and_escapes_every_name() {
    let p = page(&tricky(), stub).expect("page");
    // Header: family, designer (from the first face that has one), foundry,
    // version and license of the shown face (Regular, not Light), web use.
    assert!(p.contains("<h1 class=\"f1\">Test &lt;Sans&gt; &amp; &quot;Friends&quot;</h1>"));
    assert!(p.contains(
        "<dt>Designer</dt><dd>Ann &lt;b&gt;Designer&lt;/b&gt; &amp; &quot;Co&quot;</dd>"
    ));
    assert!(p.contains("<dt>Foundry</dt><dd>Foundry&#58; https&#58;//foundry.example/</dd>"));
    assert!(p.contains("<dt>Version</dt><dd>Version 2.000</dd>"));
    assert!(p.contains("<dt>License</dt><dd>Open license (OFL)</dd>"));
    assert!(p.contains("<dt>Web use</dt><dd>OK for web (Open license (OFL))</dd>"));
    // Every face embedded once, in its own CSS family.
    for i in 0..3 {
        assert_eq!(p.matches(&format!("url(\"{}\")", stub(i))).count(), 1);
    }
    // Styles: the static faces by style name, the variable one by instance;
    // an axis tag CSS cannot quote is left out.
    assert!(p.contains("<p class=\"label\">Light</p><p class=\"sample f0\">"));
    assert!(p.contains("<p class=\"label\">Regular</p><p class=\"sample f1\">"));
    assert!(p.contains(
        "<p class=\"label\">Thin &lt;Italic&gt;</p><p class=\"sample f2\" style=\"font-variation-settings: &quot;wght&quot; 100\">"
    ));
    assert!(p.contains(
        "<p class=\"label\">Black Italic</p><p class=\"sample f2\" style=\"font-variation-settings: &quot;wght&quot; 900\">"
    ));
    assert!(!p.contains("<p class=\"label\">Italic</p>"));
    // Waterfall at every size, in the shown face.
    for size in WATERFALL_SIZES {
        assert!(p.contains(&format!(
            "<p class=\"label\">{size} px</p><p class=\"sample f1\" style=\"font-size: {size}px\">The quick brown fox jumps over the lazy dog</p>"
        )));
    }
    // Character set by block, in code point order; the space is left out,
    // the escaped characters are escaped.
    let latin = p.find("<h3>Basic Latin</h3>").expect("Basic Latin");
    let latin1 = p.find("<h3>Latin-1 Supplement</h3>").expect("Latin-1");
    let greek = p.find("<h3>Greek and Coptic</h3>").expect("Greek");
    let marks = p
        .find("<h3>Combining Diacritical Marks</h3>")
        .expect("marks");
    assert!(latin < latin1 && latin1 < marks && marks < greek);
    assert!(p.contains("<span title=\"U+0041\">A</span>"));
    assert!(p.contains("<span title=\"U+003C\">&lt;</span>"));
    assert!(p.contains("<span title=\"U+003A\">&#58;</span>"));
    assert!(p.contains("<span title=\"U+03B1\">α</span>"));
    assert!(!p.contains("U+0020"));
    // Features: switchable ones only, once each, in the first face that has
    // it, off then on, with the friendly or font-given name.
    assert!(p.contains("<h3>Alt &lt;a&gt; &#39;single&#39; <code>ss01</code></h3>"));
    assert!(!p.contains("Never shown"));
    assert!(!p.contains("<code>ccmp</code>"));
    assert!(p.contains(
        "<p class=\"label\">Off</p><p class=\"sample f0\" style=\"font-feature-settings: &quot;ss01&quot; 0\">"
    ));
    assert!(p.contains(
        "<p class=\"label\">On</p><p class=\"sample f0\" style=\"font-feature-settings: &quot;ss01&quot; 1\">"
    ));
    let smcp = features::feature_info("smcp").name.expect("smcp name");
    assert!(p.contains(&format!("<h3>{smcp} <code>smcp</code></h3>")));
    assert!(p.contains("font-feature-settings: &quot;smcp&quot; 1"));
    assert!(p.contains("font-feature-settings: &quot;kern&quot; 0"));
    assert!(
        p.contains("<p class=\"sample f2\" style=\"font-feature-settings: &quot;onum&quot; 1\">")
    );
    // Nothing from outside, no script, and no raw markup from a name.
    let lower = p.to_lowercase();
    for bad in [
        "http:", "https:", "file:", "<script", "<b>", "<a>", "<sans>", "<italic>",
    ] {
        assert!(!lower.contains(bad), "{bad}");
    }
}

#[test]
fn a_face_with_no_switchable_feature_has_no_features_section() {
    let mut spec = tricky();
    for f in &mut spec.faces {
        f.features.retain(|f| f.automatic);
    }
    let p = page(&spec, stub).expect("page");
    assert!(!p.contains("<h2>Features</h2>"));
    // A missing designer, foundry or version says so.
    let mut spec = tricky();
    for f in &mut spec.faces {
        f.designer = None;
        f.foundry = None;
        f.version = None;
    }
    let p = page(&spec, stub).expect("page");
    assert!(p.contains("<dt>Designer</dt><dd>Not stated</dd>"));
    assert!(p.contains("<dt>Foundry</dt><dd>Not stated</dd>"));
    assert!(p.contains("<dt>Version</dt><dd>Not stated</dd>"));
}

#[test]
fn the_license_and_web_lines_follow_the_faces() {
    let mut spec = tricky();
    spec.faces[1].declared = Some(LicenseType::Desktop);
    spec.faces[2].web_use = WebReason::Declared(LicenseType::Desktop);
    let p = page(&spec, stub).expect("page");
    assert!(p.contains("<dt>License</dt><dd>Declared desktop license</dd>"));
    // One face not OK for web makes the family's line say so.
    assert!(p.contains("<dt>Web use</dt><dd>Not OK for web (Declared desktop license)</dd>"));
    spec.faces[1].declared = None;
    spec.faces[1].open_license = None;
    let p = page(&spec, stub).expect("page");
    assert!(p.contains("<dt>License</dt><dd>No license declared</dd>"));
}

#[test]
fn restricted_embedding_without_a_license_refuses_the_specimen() {
    let mut spec = tricky();
    spec.faces[0].embedding = Embedding::Restricted;
    spec.faces[2].embedding = Embedding::Restricted;
    spec.faces[2].declared = Some(LicenseType::Unknown);
    spec.faces[1].embedding = Embedding::PreviewAndPrint;
    let e = check_embedding(&spec).expect_err("restricted");
    assert_eq!(e.code(), ErrorCode::PermissionDenied);
    assert_eq!(
        e.message(),
        "Test Sans Light, Test Sans Italic: Embedding restricted and no license declared, \
         so the font may not be embedded in a specimen. Declare a license for the family to export it."
    );
    assert_eq!(e.detail(), Some("blocked faces: 1, 3"));
    // Nothing is read or written: the faces' files do not exist.
    let dir = crate::test_support::TempDir::new("specimen-refused");
    let out = dir.path().join("out.html");
    let e = write_html(&spec, &out).expect_err("restricted");
    assert_eq!(e.code(), ErrorCode::PermissionDenied);
    assert!(!out.exists());
    // A declared license lifts it; preview and print always embeds.
    spec.faces[0].declared = Some(LicenseType::Desktop);
    spec.faces[2].declared = Some(LicenseType::Trial);
    check_embedding(&spec).expect("declared");
}

/// A second family for a collection: one static face, a declared desktop
/// license, a designer of its own.
fn plain() -> Specimen {
    let mut bold = face(10, "Bold", 700, Slant::Upright);
    bold.label = "Plain Serif Bold".to_owned();
    bold.designer = Some("Second Designer".to_owned());
    bold.declared = Some(LicenseType::Desktop);
    bold.open_license = None;
    bold.web_use = license::web_use(Embedding::Installable, None, Some(LicenseType::Desktop));
    bold.path = PathBuf::from("/fonts/PlainSerif-Bold.ttf");
    Specimen {
        family: "Plain Serif".to_owned(),
        faces: vec![bold],
    }
}

fn collection() -> CollectionSpecimen {
    CollectionSpecimen {
        name: "Client <A> & \"B\"".to_owned(),
        families: vec![tricky(), plain()],
    }
}

#[test]
fn a_collection_page_has_a_cover_then_a_page_per_family_in_order() {
    let c = collection();
    let p = collection_page(&c, "2026-10-24", stub).expect("page");
    // The cover: the name (escaped), the date and the number of families.
    let cover =
        &p[p.find("<header class=\"cover\">").expect("cover")..p.find("</header>").expect("end")];
    assert!(
        cover.contains("<h1>Client &lt;A&gt; &amp; &quot;B&quot;</h1>"),
        "{cover}"
    );
    assert!(
        cover.contains("<dt>Date</dt><dd>2026-10-24</dd>"),
        "{cover}"
    );
    assert!(cover.contains("<dt>Families</dt><dd>2</dd>"), "{cover}");
    assert!(p.contains("<title>Client &lt;A&gt; &amp; &quot;B&quot; specimen</title>"));

    // Every face of every family is embedded once, numbered across families.
    assert_eq!(p.matches("@font-face").count(), 4);
    for i in 0..4 {
        assert!(p.contains(&stub(i)), "face {i}");
    }

    // One section per family, in the collection's order, each on a new
    // printed page.
    let sections: Vec<&str> = p.split("<section class=\"family\">").skip(1).collect();
    assert_eq!(sections.len(), 2);
    assert!(COLLECTION_STYLE.contains(".family { break-before: page;"));
    assert!(p.find("Test &lt;Sans&gt;").expect("first") < p.find("Plain Serif").expect("second"));

    // The first family: its name in its Regular (f1), every style, the
    // sample at three sizes, its license and web use.
    let first = sections[0];
    assert!(
        first.starts_with("\n<h2 class=\"f1\">Test &lt;Sans&gt; &amp; &quot;Friends&quot;</h2>"),
        "{first}"
    );
    for label in ["Light", "Regular", "Thin &lt;Italic&gt;", "Black Italic"] {
        assert!(
            first.contains(&format!("<p class=\"label\">{label}</p>")),
            "{label}"
        );
    }
    for size in COLLECTION_SIZES {
        assert!(
            first.contains(&format!("<p class=\"label\">{size} px</p><p class=\"sample f1\" style=\"font-size: {size}px\">")),
            "{size}"
        );
    }
    assert_eq!(first.matches(" px</p>").count(), 3);
    let ofl = &tricky().faces[1];
    assert!(first.contains(&format!(
        "<dt>License</dt><dd>{}</dd>",
        escape(&license_line(ofl))
    )));
    assert!(first.contains(&format!(
        "<dt>Web use</dt><dd>{}</dd>",
        escape(&web_line(ofl.web_use))
    )));

    // The second family's faces start after the first family's: f3.
    let second = sections[1];
    assert!(
        second.starts_with("\n<h2 class=\"f3\">Plain Serif</h2>"),
        "{second}"
    );
    assert!(second.contains("<p class=\"label\">Bold</p><p class=\"sample f3\">"));
    assert!(second.contains("<dt>Designer</dt><dd>Second Designer</dd>"));
    assert!(second.contains("<dt>License</dt><dd>Declared desktop license</dd>"));
    let web = web_line(plain().faces[0].web_use);
    assert!(web.starts_with("OK for web") || web.starts_with("Not OK for web"));
    assert!(second.contains(&format!("<dt>Web use</dt><dd>{}</dd>", escape(&web))));
    assert!(!second.contains("Test Sans"), "{second}");

    // Self-contained and deterministic.
    let lower = p.to_lowercase();
    for bad in ["http:", "https:", "file:", "<script"] {
        assert!(!lower.contains(bad), "{bad}");
    }
    assert_eq!(collection_page(&c, "2026-10-24", stub).expect("again"), p);
}

#[test]
fn an_empty_collection_and_a_restricted_face_refuse_the_collection_specimen() {
    let dir = crate::test_support::TempDir::new("specimen-collection-refused");
    let out = dir.path().join("out.html");
    let empty = CollectionSpecimen {
        name: "Nothing yet".to_owned(),
        families: Vec::new(),
    };
    let e = write_collection_html(&empty, "2026-10-24", &out).expect_err("empty");
    assert_eq!(e.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        e.message(),
        "The collection Nothing yet is empty. Add fonts to it to export a specimen."
    );
    assert!(!out.exists());

    // A restricted face in the second family refuses the whole document,
    // the same way as a family's specimen, before any file is read.
    let mut c = collection();
    c.families[1].faces[0].embedding = Embedding::Restricted;
    c.families[1].faces[0].declared = None;
    let e = write_collection_html(&c, "2026-10-24", &out).expect_err("restricted");
    assert_eq!(e.code(), ErrorCode::PermissionDenied);
    assert_eq!(
        e.message(),
        "Plain Serif Bold: Embedding restricted and no license declared, so the font may \
         not be embedded in a specimen. Declare a license for the family to export it."
    );
    assert_eq!(e.detail(), Some("blocked faces: 10"));
    assert!(!out.exists());
}

#[test]
fn utc_dates_follow_the_gregorian_calendar() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(-1), "1969-12-31");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(951_868_800), "2000-03-01");
    assert_eq!(utc_date(1_700_000_000), "2023-11-14");
    assert_eq!(utc_date(4_107_542_399), "2100-02-28");
    assert_eq!(utc_date(4_107_542_400), "2100-03-01");
    assert_eq!(today().len(), 10);
}
