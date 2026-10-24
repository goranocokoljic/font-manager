//! The Fontshare adapter on the recorded, trimmed index in
//! `fixtures/catalogs/fontshare/`, parsed directly and served from a local
//! server: no test reaches the internet.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fm_api::ErrorCode;

use super::*;
use crate::catalog::{Catalogs, HttpFetcher};
use crate::test_support::{LocalServer, TempDir};

fn fixture() -> Vec<u8> {
    let path: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/catalogs/fontshare/fonts.json");
    std::fs::read(&path).expect("catalog fixture")
}

fn by_name<'a>(families: &'a [CatalogFamily], name: &str) -> &'a CatalogFamily {
    families
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("{name} in the index"))
}

fn weights(family: &CatalogFamily, slant: Slant) -> Vec<u32> {
    family
        .styles
        .iter()
        .filter(|s| s.slant == slant)
        .map(|s| s.weight)
        .collect()
}

/// The recorded Literata entry, field by field.
fn assert_literata(literata: &CatalogFamily) {
    assert_eq!(literata.category.as_deref(), Some("Serif"));
    let all = [200, 300, 400, 500, 600, 700, 800, 900];
    assert_eq!(weights(literata, Slant::Upright), all);
    assert_eq!(weights(literata, Slant::Italic), all);
    assert_eq!(literata.styles.len(), 16, "the two variable fonts are axes");
    assert_eq!(literata.styles[0].slant, Slant::Upright, "upright first");
    assert_eq!(literata.axes.len(), 2);
    let wght = &literata.axes[0];
    assert_eq!((wght.tag.as_str(), wght.name.as_str()), ("wght", "Weight"));
    assert_eq!((wght.min, wght.default, wght.max), (200.0, 760.0, 900.0));
    let opsz = &literata.axes[1];
    assert_eq!(
        (opsz.tag.as_str(), opsz.name.as_str()),
        ("opsz", "Optical Size")
    );
    assert_eq!((opsz.min, opsz.default, opsz.max), (7.0, 7.0, 72.0));
    assert_eq!(literata.subsets, ["latin"]);
    assert_eq!(literata.designers, ["TypeTogether"]);
    assert_eq!(literata.version.as_deref(), Some("3.021"));
    assert_eq!(literata.last_modified, None, "the index has no change date");
    assert!(literata.open_source);
    assert_eq!(literata.license, Some(OpenLicense::Ofl));
    assert_eq!(
        literata.license_name.as_deref(),
        Some("SIL Open Font License")
    );
}

#[test]
fn the_recorded_index_parses_into_its_families() {
    let families = parse_index(&fixture()).expect("index");
    let names: Vec<&str> = families.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Kihim", "General Sans", "Gambetta", "Literata"]);
    assert_literata(by_name(&families, "Literata"));
}

#[test]
fn an_itf_free_font_license_family_is_named_but_not_open_source() {
    let families = parse_index(&fixture()).expect("index");
    let general = by_name(&families, "General Sans");
    assert_eq!(
        general.license_name.as_deref(),
        Some("ITF Free Font License")
    );
    assert_eq!(general.license, None, "not an open license");
    assert!(!general.open_source);
    assert_eq!(general.version.as_deref(), Some("1.0"));
    assert_eq!(general.designers, ["Frode Helland"]);
    assert_eq!(general.category.as_deref(), Some("Sans"));
    let all = [200, 300, 400, 500, 600, 700];
    assert_eq!(weights(general, Slant::Upright), all);
    assert_eq!(weights(general, Slant::Italic), all);
}

#[test]
fn axes_named_by_their_tag_get_the_registered_name_and_repeats_are_dropped() {
    let families = parse_index(&fixture()).expect("index");
    // General Sans names its axis "wght"; Gambetta lists its axis twice,
    // once for its upright and once for its italic variable font.
    for name in ["General Sans", "Gambetta"] {
        let family = by_name(&families, name);
        assert_eq!(family.axes.len(), 1, "{name}");
        assert_eq!(family.axes[0].tag, "wght", "{name}");
        assert_eq!(family.axes[0].name, "Weight", "{name}");
    }
    let general = by_name(&families, "General Sans");
    assert_eq!(
        (
            general.axes[0].min,
            general.axes[0].default,
            general.axes[0].max
        ),
        (200.0, 250.0, 700.0)
    );
    let kihim = by_name(&families, "Kihim");
    assert!(kihim.axes.is_empty(), "static family");
    assert_eq!(
        kihim.styles,
        [CatalogStyle {
            weight: 400,
            slant: Slant::Upright
        }]
    );
}

#[test]
fn an_unknown_license_type_is_named_by_its_id() {
    let body = br#"{"fonts":[{"name":"X","license_type":"custom_eula",
        "axes":[{"name":"","property":"ZZZZ","range_left":0,"range_right":1,"range_default":0}],
        "styles":[{"weight":{"weight":400},"is_italic":true}]}]}"#;
    let families = parse_index(body).expect("index");
    assert_eq!(families[0].license_name.as_deref(), Some("custom_eula"));
    assert_eq!(families[0].license, None);
    assert!(!families[0].open_source);
    assert_eq!(families[0].axes[0].name, "ZZZZ");
    assert_eq!(families[0].styles[0].slant, Slant::Italic);
}

#[test]
fn a_family_that_cannot_be_read_is_left_out() {
    let body = br#"{"fonts":[
        {"name":"Good","styles":[{"weight":{"weight":400}},{"weight":{"weight":0},"is_variable":true},{"weight":{"weight":5000}}]},
        {"name":"Broken","axes":[{"property":"wght"}]},
        {"name":"  "},
        {"category":"Serif"}
    ]}"#;
    let families = parse_index(body).expect("index");
    assert_eq!(families.len(), 1);
    assert_eq!(families[0].name, "Good");
    assert_eq!(
        families[0].styles.len(),
        1,
        "variable and out-of-range left out"
    );
    assert_eq!(families[0].license_name, None);
}

#[test]
fn a_malformed_or_partial_index_is_an_error() {
    let full = fixture();
    let cut = &full[..full.len() / 2];
    for body in [
        &b""[..],
        b"<html>Service unavailable</html>",
        b"{\"fonts\":[]}",
        b"{\"count\":0}",
        b"{\"fonts\":[{\"name\":\"Cut",
        b"{\"fonts\":[{\"slug\":\"no-name\"}]}",
        cut,
        // The answer says it is only a part of the index.
        b"{\"fonts\":[{\"name\":\"A\"}],\"has_more\":true}",
        b"{\"fonts\":[{\"name\":\"A\"}],\"count_total\":100}",
    ] {
        let e = parse_index(body).expect_err("malformed");
        assert_eq!(e.code(), ErrorCode::Io, "{}", String::from_utf8_lossy(body));
        assert!(e.message().contains("cannot read"), "{}", e.message());
    }
}

#[test]
fn a_family_resolves_to_its_ttf_urls() {
    let files = parse_files(&fixture(), "General Sans").expect("files");
    assert_eq!(files.files.len(), 14, "12 static and 2 variable fonts");
    assert_eq!(files.files[0].name, "GeneralSans-Extralight.ttf");
    assert_eq!(
        files.files[0].url,
        "https://cdn.fontshare.com/wf/FSGJYPLPXRLVZAZIVVF74LDN3Q3S2WUF/\
         S5KG2IZFL42XIGI5UEGBWGDAYOGDCUXQ/XC3ORZWTJXPLYORAWMH6ZXES6GBV3W5L.ttf"
    );
    let names: Vec<&str> = files.files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"GeneralSans-BoldItalic.ttf"), "{names:?}");
    assert!(
        names.contains(&"GeneralSans-VariableItalic.ttf"),
        "{names:?}"
    );
    assert_eq!(files.license, None, "ITF Free Font License");

    let literata = parse_files(&fixture(), "literata").expect("any case");
    assert_eq!(literata.files.len(), 18);
    assert_eq!(literata.license, Some(OpenLicense::Ofl));
}

#[test]
fn file_urls_and_names_are_made_safe() {
    assert_eq!(
        file_url("//cdn.fontshare.com/wf/A").as_deref(),
        Some("https://cdn.fontshare.com/wf/A.ttf")
    );
    assert_eq!(
        file_url("https://x/A.TTF").as_deref(),
        Some("https://x/A.TTF")
    );
    assert_eq!(file_url("wf/A"), None, "not an absolute URL");
    assert_eq!(file_url("javascript:alert(1)"), None);
    assert_eq!(file_part("Be Vietnam Pro/../é"), "BeVietnamPro");

    let body = r#"{"fonts":[{"name":"Été","styles":[
        {"file":"//cdn/a","weight":{"name":"Bold Italic","weight":700},"is_italic":true},
        {"file":"//cdn/b","weight":{"name":"Bold Italic","weight":700},"is_italic":true},
        {"file":"//cdn/c","weight":{"weight":300}},
        {"weight":{"name":"NoFile","weight":400}}
    ]}]}"#;
    let files = parse_files(body.as_bytes(), "Été").expect("files");
    let names: Vec<&str> = files.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["t-BoldItalic.ttf", "t-300.ttf"],
        "duplicates dropped"
    );
}

#[test]
fn an_unknown_family_or_one_without_files_is_an_error() {
    let e = parse_files(&fixture(), "Comic Sans").expect_err("unknown");
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert!(e.message().contains("Comic Sans"), "{}", e.message());

    let body = br#"{"fonts":[{"name":"A","styles":[{"weight":{"weight":400}}]}]}"#;
    assert_eq!(
        parse_files(body, "A").expect_err("no files").code(),
        ErrorCode::Io
    );
}

#[test]
fn urls_are_built_on_the_configured_base() {
    assert_eq!(
        Fontshare::with_base_url("http://127.0.0.1:9/").index_url(),
        "http://127.0.0.1:9/v2/fonts"
    );
    assert_eq!(
        Fontshare::new().index_url(),
        "https://api.fontshare.com/v2/fonts"
    );
    assert_eq!(Fontshare::new().homepage(), "https://www.fontshare.com");
    assert_eq!(Fontshare::new().id(), CatalogId::Fontshare);
}

/// The catalogs over HTTP against a local server at `base`.
fn catalogs_at(dir: &TempDir, base: &str) -> Catalogs {
    Catalogs::new(
        dir.path().join("catalogs"),
        Arc::new(HttpFetcher::new()),
        vec![Arc::new(Fontshare::with_base_url(base))],
    )
}

#[test]
fn refresh_and_resolve_through_http_from_a_local_server() {
    let server = LocalServer::start(vec![("/v2/fonts", 200, fixture())]);
    let dir = TempDir::new("fontshare-http");
    let catalogs = catalogs_at(&dir, server.base());
    let off = &catalogs.list().expect("list")[0];
    assert_eq!(off.id, CatalogId::Fontshare);
    assert_eq!(off.name, "Fontshare");
    assert!(!off.enabled, "off by default");
    assert_eq!(server.hits(), 0);

    catalogs
        .set_enabled(CatalogId::Fontshare, true)
        .expect("on");
    let catalog = catalogs.refresh(CatalogId::Fontshare).expect("refresh");
    assert_eq!(catalog.family_count, 4);
    assert_eq!(server.hits(), 1);
    assert!(dir.path().join("catalogs/fontshare.json").is_file());

    let page = catalogs
        .families(CatalogId::Fontshare, "litera", Default::default())
        .expect("families");
    assert_eq!(page.total, 1);
    assert_literata(&page.items[0]);

    let files = catalogs
        .resolve_files(CatalogId::Fontshare, "Literata")
        .expect("files");
    assert_eq!(files.files.len(), 18);
    assert!(
        files
            .files
            .iter()
            .all(|f| f.url.starts_with("https://cdn.fontshare.com/wf/"))
    );
    assert_eq!(server.hits(), 2);
}

#[test]
fn a_malformed_or_partial_answer_keeps_the_previous_cache() {
    let good = LocalServer::start(vec![("/v2/fonts", 200, fixture())]);
    let dir = TempDir::new("fontshare-http-bad");
    let catalogs = catalogs_at(&dir, good.base());
    catalogs
        .set_enabled(CatalogId::Fontshare, true)
        .expect("on");
    let before = catalogs.refresh(CatalogId::Fontshare).expect("refresh");
    let cache = std::fs::read(dir.path().join("catalogs/fontshare.json")).expect("cache");

    let full = fixture();
    for body in [
        full[..full.len() / 3].to_vec(),
        br#"{"fonts":[{"name":"A"}],"has_more":true}"#.to_vec(),
        b"<html>maintenance</html>".to_vec(),
    ] {
        let bad = LocalServer::start(vec![("/v2/fonts", 200, body)]);
        let catalogs = catalogs_at(&dir, bad.base());
        let e = catalogs
            .refresh(CatalogId::Fontshare)
            .expect_err("bad answer");
        assert_eq!(e.code(), ErrorCode::Io);
        assert!(
            e.message().starts_with("Fontshare could not be refreshed."),
            "{e}"
        );
        assert_eq!(bad.hits(), 1);
        assert_eq!(
            &catalogs.list().expect("list")[0],
            &before,
            "the cache stays"
        );
        assert_eq!(
            std::fs::read(dir.path().join("catalogs/fontshare.json")).expect("cache"),
            cache,
            "the cache file is untouched"
        );
    }
}
