//! The Google Fonts adapter on the recorded, trimmed answers in
//! `fixtures/catalogs/google-fonts/`, parsed directly and served from a
//! local server: no test reaches the internet.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fm_api::ErrorCode;

use super::*;
use crate::catalog::{Catalogs, HttpFetcher};
use crate::test_support::{LocalServer, TempDir};

fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/catalogs/google-fonts")
        .join(name);
    std::fs::read(&path).expect("catalog fixture")
}

fn by_name<'a>(families: &'a [CatalogFamily], name: &str) -> &'a CatalogFamily {
    families
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("{name} in the index"))
}

/// The recorded Lato entry, field by field.
fn assert_lato(lato: &CatalogFamily) {
    assert_eq!(lato.category.as_deref(), Some("Sans Serif"));
    let upright: Vec<u32> = lato
        .styles
        .iter()
        .filter(|s| s.slant == Slant::Upright)
        .map(|s| s.weight)
        .collect();
    let italic: Vec<u32> = lato
        .styles
        .iter()
        .filter(|s| s.slant == Slant::Italic)
        .map(|s| s.weight)
        .collect();
    assert_eq!(upright, [100, 300, 400, 700, 900]);
    assert_eq!(italic, [100, 300, 400, 700, 900]);
    assert_eq!(lato.styles[0].slant, Slant::Upright, "upright first");
    assert!(lato.axes.is_empty(), "static family");
    assert_eq!(lato.subsets, ["latin", "latin-ext"], "menu dropped");
    assert_eq!(lato.designers, ["Łukasz Dziedzic"]);
    assert_eq!(lato.last_modified.as_deref(), Some("2025-09-16"));
    assert_eq!(lato.version, None, "the index has no version");
    assert!(lato.open_source);
    assert_eq!(lato.license, None, "the index names no license");
}

#[test]
fn the_recorded_index_parses_into_its_families() {
    let families = parse_index(&fixture("metadata-fonts.json")).expect("index");
    assert_eq!(families.len(), 5);
    assert_lato(by_name(&families, "Lato"));
}

#[test]
fn axes_take_their_names_from_the_axis_registry() {
    let families = parse_index(&fixture("metadata-fonts.json")).expect("index");
    let flex = by_name(&families, "Roboto Flex");
    assert_eq!(flex.axes.len(), 13);
    let wght = flex.axes.iter().find(|a| a.tag == "wght").expect("wght");
    assert_eq!(wght.name, "Weight");
    assert_eq!((wght.min, wght.default, wght.max), (100.0, 400.0, 1000.0));
    let grad = flex.axes.iter().find(|a| a.tag == "GRAD").expect("GRAD");
    assert_eq!((grad.min, grad.default, grad.max), (-200.0, 0.0, 150.0));
    assert_eq!(flex.styles.len(), 10, "100–1000, upright only");
}

#[test]
fn an_axis_the_registry_does_not_name_is_named_by_its_tag() {
    let body = br#"{"familyMetadataList":[{"family":"X","fonts":{"400":{}},
        "axes":[{"tag":"ZZZZ","min":0,"max":1,"defaultValue":0}]}]}"#;
    let families = parse_index(body).expect("index");
    assert_eq!(families[0].axes[0].name, "ZZZZ");
    assert!(!families[0].open_source, "not said to be open source");
}

#[test]
fn the_json_guard_and_a_byte_order_mark_are_skipped() {
    let body =
        b"\xEF\xBB\xBF)]}'\n{\"familyMetadataList\":[{\"family\":\"A\",\"fonts\":{\"700i\":{}}}]}";
    let families = parse_index(body).expect("index");
    assert_eq!(
        families[0].styles,
        [CatalogStyle {
            weight: 700,
            slant: Slant::Italic
        }]
    );
}

#[test]
fn a_family_that_cannot_be_read_is_left_out() {
    let body = br#"{"familyMetadataList":[
        {"family":"Good","fonts":{"400":{},"bold":{},"0":{}}},
        {"family":"Broken","axes":[{"tag":"wght"}]},
        {"family":"  "},
        {"category":"Serif"}
    ]}"#;
    let families = parse_index(body).expect("index");
    assert_eq!(families.len(), 1);
    assert_eq!(families[0].name, "Good");
    assert_eq!(families[0].styles.len(), 1, "unknown style keys skipped");
}

#[test]
fn a_malformed_index_is_an_error() {
    for body in [
        &b""[..],
        b"<html>Service unavailable</html>",
        b"{\"familyMetadataList\":[]}",
        b"{\"axisRegistry\":[]}",
        b"{\"familyMetadataList\":[{\"family\":\"Cut",
        b"{\"familyMetadataList\":[{\"name\":\"no family key\"}]}",
    ] {
        let e = parse_index(body).expect_err("malformed");
        assert_eq!(e.code(), ErrorCode::Io, "{}", String::from_utf8_lossy(body));
        assert!(e.message().contains("cannot read"), "{}", e.message());
    }
}

#[test]
fn the_recorded_file_list_gives_urls_and_the_license() {
    let files = parse_files(&fixture("download-list-lato.json"), "Lato").expect("file list");
    assert_eq!(files.files.len(), 10);
    assert_eq!(files.files[0].name, "Lato-Thin.ttf");
    assert!(
        files.files[0]
            .url
            .starts_with("https://fonts.gstatic.com/s/lato/")
    );
    assert_eq!(files.license, Some(OpenLicense::Ofl));
    assert_eq!(
        files.family_id, "Lato",
        "Google Fonts keys a family by its name"
    );
    assert!(files.files.iter().all(|f| !f.variable), "Lato is static");
}

#[test]
fn a_file_that_names_its_axes_is_variable() {
    let body = br#"{"manifest":{"files":[],"fileRefs":[
        {"filename":"Inter[opsz,wght].ttf","url":"https://x/a"},
        {"filename":"static/Inter_18pt-Bold.ttf","url":"https://x/b"}]}}"#;
    let files = parse_files(body, "Inter").expect("list");
    let kinds: Vec<(&str, bool)> = files
        .files
        .iter()
        .map(|f| (f.name.as_str(), f.variable))
        .collect();
    assert_eq!(
        kinds,
        [
            ("Inter[opsz,wght].ttf", true),
            ("static/Inter_18pt-Bold.ttf", false)
        ]
    );
}

#[test]
fn the_license_file_names_the_license() {
    for (name, license) in [
        ("LICENSE.txt", Some(OpenLicense::Apache)),
        ("static/UFL.txt", Some(OpenLicense::Ufl)),
        ("README.txt", None),
    ] {
        let body = format!(
            r#"{{"manifest":{{"files":[{{"filename":"{name}"}}],
                "fileRefs":[{{"filename":"A.ttf","url":"https://x/A.ttf"}}]}}}}"#
        );
        assert_eq!(
            parse_files(body.as_bytes(), "X").expect("list").license,
            license
        );
    }
}

#[test]
fn a_file_list_without_files_is_an_error() {
    for body in [
        &b"{}"[..],
        b"{\"manifest\":{\"files\":[{\"filename\":\"OFL.txt\"}]}}",
        b"not json",
    ] {
        assert_eq!(
            parse_files(body, "X").expect_err("no files").code(),
            ErrorCode::Io
        );
    }
}

#[test]
fn urls_are_built_on_the_configured_base() {
    let google = GoogleFonts::with_base_url("http://127.0.0.1:9/");
    assert_eq!(google.index_url(), "http://127.0.0.1:9/metadata/fonts");
    assert_eq!(
        google.files_url("Noto Sans JP"),
        "http://127.0.0.1:9/download/list?family=Noto%20Sans%20JP"
    );
    assert_eq!(
        GoogleFonts::new().index_url(),
        "https://fonts.google.com/metadata/fonts"
    );
    assert_eq!(percent_encode("A&B=C/é"), "A%26B%3DC%2F%C3%A9");
}

/// The catalogs over HTTP against a local server at `base`.
fn catalogs_at(dir: &TempDir, base: &str) -> Catalogs {
    Catalogs::new(
        dir.path().join("catalogs"),
        Arc::new(HttpFetcher::new()),
        vec![Arc::new(GoogleFonts::with_base_url(base))],
    )
}

#[test]
fn refresh_through_http_from_a_local_server() {
    let server = LocalServer::start(vec![
        ("/metadata/fonts", 200, fixture("metadata-fonts.json")),
        (
            "/download/list?family=Lato",
            200,
            fixture("download-list-lato.json"),
        ),
    ]);
    let dir = TempDir::new("catalog-http");
    let catalogs = catalogs_at(&dir, server.base());
    catalogs
        .set_enabled(CatalogId::GoogleFonts, true)
        .expect("on");
    let catalog = catalogs.refresh(CatalogId::GoogleFonts).expect("refresh");
    assert_eq!(catalog.family_count, 5);
    assert_eq!(server.hits(), 1);

    let page = catalogs
        .families(CatalogId::GoogleFonts, "lato", Default::default())
        .expect("families");
    assert_eq!(page.total, 1);
    assert_lato(&page.items[0]);

    let files = catalogs
        .resolve_files(CatalogId::GoogleFonts, "Lato")
        .expect("files");
    assert_eq!(files.files.len(), 10);
    assert_eq!(files.license, Some(OpenLicense::Ofl));
    assert_eq!(server.hits(), 2);
}

#[test]
fn an_error_status_from_the_server_keeps_the_cache() {
    let good = LocalServer::start(vec![(
        "/metadata/fonts",
        200,
        fixture("metadata-fonts.json"),
    )]);
    let bad = LocalServer::start(vec![("/metadata/fonts", 503, b"busy".to_vec())]);
    let dir = TempDir::new("catalog-http-503");
    let catalogs = catalogs_at(&dir, good.base());
    catalogs
        .set_enabled(CatalogId::GoogleFonts, true)
        .expect("on");
    let before = catalogs.refresh(CatalogId::GoogleFonts).expect("refresh");

    let catalogs = catalogs_at(&dir, bad.base());
    let e = catalogs
        .refresh(CatalogId::GoogleFonts)
        .expect_err("503 is an error");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(
        e.message()
            .starts_with("Google Fonts could not be refreshed.")
    );
    assert!(e.detail().unwrap_or_default().contains("503"), "{e}");
    assert_eq!(bad.hits(), 1);
    let after = &catalogs.list().expect("list")[0];
    assert_eq!(after, &before, "the previous cache stays");
}

#[test]
fn a_server_that_is_not_there_is_an_io_error() {
    // Port 9 (discard) on loopback: nothing listens there on a test runner.
    let dir = TempDir::new("catalog-http-down");
    let catalogs = catalogs_at(&dir, "http://127.0.0.1:9");
    catalogs
        .set_enabled(CatalogId::GoogleFonts, true)
        .expect("on");
    let e = catalogs
        .refresh(CatalogId::GoogleFonts)
        .expect_err("unreachable");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(e.message().contains("could not be reached"), "{e}");
}
