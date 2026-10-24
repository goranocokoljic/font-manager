//! The catalogs' state and cache, over a scripted fetcher that counts its
//! calls: nothing here reaches the network.

use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::catalog::google::GoogleFonts;
use crate::test_support::TempDir;

const GOOGLE: CatalogId = CatalogId::GoogleFonts;

fn index_fixture() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/catalogs/google-fonts/metadata-fonts.json");
    fs::read(path).expect("catalog fixture")
}

/// A fetcher that answers every call with one scripted answer and counts
/// the calls.
#[derive(Default)]
struct Scripted {
    calls: AtomicU32,
    answer: Mutex<Option<FmResult<Vec<u8>>>>,
}

impl Scripted {
    fn answering(answer: FmResult<Vec<u8>>) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU32::new(0),
            answer: Mutex::new(Some(answer)),
        })
    }

    /// A fetcher that fails on any call.
    fn failing() -> Arc<Self> {
        Self::answering(Err(FmError::new(ErrorCode::Io, "offline")))
    }

    fn set(&self, answer: FmResult<Vec<u8>>) {
        *lock(&self.answer) = Some(answer);
    }

    fn calls(&self) -> u32 {
        self.calls.load(Ordering::Relaxed)
    }
}

impl Fetcher for Scripted {
    fn get(&self, _url: &str) -> FmResult<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        lock(&self.answer)
            .clone()
            .unwrap_or_else(|| Err(FmError::internal("no answer")))
    }
}

fn catalogs(dir: &TempDir, fetcher: &Arc<Scripted>) -> Catalogs {
    Catalogs::new(
        dir.path().join(CATALOGS_DIR),
        Arc::clone(fetcher) as Arc<dyn Fetcher>,
        vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
    )
}

fn names(page: &Paged<CatalogFamily>) -> Vec<&str> {
    page.items.iter().map(|f| f.name.as_str()).collect()
}

/// Catalogs on `dir` with Google Fonts on and refreshed from the fixture.
fn refreshed(dir: &TempDir) -> (Catalogs, Arc<Scripted>) {
    let fetcher = Scripted::answering(Ok(index_fixture()));
    let c = catalogs(dir, &fetcher);
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");
    (c, fetcher)
}

#[test]
fn catalogs_are_off_by_default_and_refused_while_off() {
    let dir = TempDir::new("catalog-off");
    let fetcher = Scripted::failing();
    let c = catalogs(&dir, &fetcher);
    let list = c.list().expect("list");
    assert_eq!(list.len(), 1);
    assert!(!list[0].enabled);
    assert_eq!(list[0].name, "Google Fonts");
    assert_eq!(list[0].homepage, "https://fonts.google.com");
    assert_eq!(
        (list[0].refreshed_at.as_deref(), list[0].family_count),
        (None, 0)
    );

    let e = c.refresh(GOOGLE).expect_err("off");
    assert_eq!(e.code(), ErrorCode::Conflict);
    let e = c.resolve_files(GOOGLE, "Lato").expect_err("off");
    assert_eq!(e.code(), ErrorCode::Conflict);
    let page = c.families(GOOGLE, "", Page::default()).expect("families");
    assert_eq!(page.total, 0);
    assert_eq!(fetcher.calls(), 0, "nothing fetched while off");
    assert!(!dir.path().join(CATALOGS_DIR).exists(), "nothing written");
}

#[test]
fn only_refresh_fetches() {
    let dir = TempDir::new("catalog-only-refresh");
    let fetcher = Scripted::failing();
    let c = catalogs(&dir, &fetcher);
    c.set_enabled(GOOGLE, true).expect("on");
    c.list().expect("list");
    c.families(GOOGLE, "lato", Page::default())
        .expect("families");
    assert_eq!(fetcher.calls(), 0);
    c.refresh(GOOGLE).expect_err("offline");
    assert_eq!(fetcher.calls(), 1);
}

#[test]
fn a_refresh_is_cached_with_its_time_and_read_after_a_restart() {
    let dir = TempDir::new("catalog-restart");
    let (c, fetcher) = refreshed(&dir);
    let catalog = &c.list().expect("list")[0];
    assert_eq!(catalog.family_count, 5);
    let at = catalog.refreshed_at.clone().expect("refresh time");
    assert_eq!(at.len(), "2026-10-24T08:00:00Z".len());
    assert!(at.ends_with('Z') && at.as_bytes()[10] == b'T', "{at}");
    assert!(
        dir.path()
            .join(CATALOGS_DIR)
            .join("google-fonts.json")
            .is_file()
    );
    assert_eq!(fetcher.calls(), 1);
    drop(c);

    // A restart, offline: a new instance on the same folder.
    let offline = Scripted::failing();
    let c = catalogs(&dir, &offline);
    let catalog = &c.list().expect("list")[0];
    assert!(catalog.enabled, "on/off survives the restart");
    assert_eq!(catalog.refreshed_at.as_deref(), Some(at.as_str()));
    let page = c.families(GOOGLE, "", Page::default()).expect("families");
    assert_eq!(
        names(&page),
        [
            "Bebas Neue",
            "Lato",
            "Noto Sans JP",
            "Playfair Display",
            "Roboto Flex"
        ],
        "by name"
    );
    let lato = &page.items[1];
    assert_eq!(lato.category.as_deref(), Some("Sans Serif"));
    assert_eq!(lato.styles.len(), 10);
    assert_eq!(lato.subsets, ["latin", "latin-ext"]);
    assert_eq!(lato.last_modified.as_deref(), Some("2025-09-16"));
    assert!(lato.open_source);
    let flex = &page.items[4];
    assert!(
        flex.axes
            .iter()
            .any(|a| a.tag == "wght" && a.name == "Weight")
    );
    assert_eq!(offline.calls(), 0, "answered from the cache");
}

#[test]
fn a_failed_or_malformed_refresh_leaves_the_cache_untouched() {
    let dir = TempDir::new("catalog-keep");
    let (c, fetcher) = refreshed(&dir);
    let file = dir.path().join(CATALOGS_DIR).join("google-fonts.json");
    let bytes = fs::read(&file).expect("cache");
    let before = c.list().expect("list");

    for (answer, code) in [
        (
            Err(FmError::new(
                ErrorCode::Io,
                "The catalog could not be reached.",
            )),
            ErrorCode::Io,
        ),
        (Ok(b"<html>Sorry</html>".to_vec()), ErrorCode::Io),
        (Ok(b"{\"familyMetadataList\":[]}".to_vec()), ErrorCode::Io),
        (Ok(index_fixture()[..4000].to_vec()), ErrorCode::Io),
    ] {
        fetcher.set(answer);
        let e = c.refresh(GOOGLE).expect_err("refused");
        assert_eq!(e.code(), code);
        assert!(
            e.message()
                .starts_with("Google Fonts could not be refreshed."),
            "{e}"
        );
        assert_eq!(fs::read(&file).expect("cache"), bytes, "file untouched");
        assert_eq!(c.list().expect("list"), before, "state untouched");
        assert_eq!(
            c.families(GOOGLE, "", Page::default())
                .expect("families")
                .total,
            5
        );
    }
    let left: Vec<_> = fs::read_dir(dir.path().join(CATALOGS_DIR))
        .expect("dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(left.len(), 2, "no temporary file left: {left:?}");
}

#[test]
fn turning_a_catalog_off_hides_its_families_and_keeps_the_cache() {
    let dir = TempDir::new("catalog-toggle");
    let (c, fetcher) = refreshed(&dir);
    let off = c.set_enabled(GOOGLE, false).expect("off");
    assert!(!off.enabled);
    assert_eq!(off.family_count, 5, "cache kept");
    assert_eq!(
        c.families(GOOGLE, "", Page::default())
            .expect("families")
            .total,
        0
    );
    assert_eq!(
        c.refresh(GOOGLE).expect_err("off").code(),
        ErrorCode::Conflict
    );
    c.set_enabled(GOOGLE, true).expect("on");
    assert_eq!(
        c.families(GOOGLE, "", Page::default())
            .expect("families")
            .total,
        5
    );
    assert_eq!(fetcher.calls(), 1, "only the first refresh fetched");
}

#[test]
fn families_are_found_by_name_and_paged() {
    let dir = TempDir::new("catalog-query");
    let (c, _) = refreshed(&dir);
    let page = |query: &str, offset, limit| {
        c.families(GOOGLE, query, Page { offset, limit })
            .expect("families")
    };
    assert_eq!(names(&page("  SANS ", 0, 10)), ["Noto Sans JP"]);
    assert_eq!(
        names(&page("o", 0, 10)),
        ["Lato", "Noto Sans JP", "Roboto Flex"]
    );
    let second = page("", 2, 2);
    assert_eq!(second.total, 5);
    assert_eq!(names(&second), ["Noto Sans JP", "Playfair Display"]);
    assert_eq!(page("", 9, 2).items.len(), 0);
    assert_eq!(page("zzz", 0, 10).total, 0);
}

#[test]
fn a_page_is_capped_at_500_families() {
    let dir = TempDir::new("catalog-cap");
    let mut body = String::from("{\"familyMetadataList\":[");
    for i in 0..600 {
        if i > 0 {
            body.push(',');
        }
        body.push_str(&format!(
            "{{\"family\":\"F{i:03}\",\"fonts\":{{\"400\":{{}}}}}}"
        ));
    }
    body.push_str("]}");
    let fetcher = Scripted::answering(Ok(body.into_bytes()));
    let c = catalogs(&dir, &fetcher);
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");
    let page = c
        .families(
            GOOGLE,
            "",
            Page {
                offset: 0,
                limit: 10_000,
            },
        )
        .expect("families");
    assert_eq!(page.total, 600);
    assert_eq!(page.items.len(), MAX_PAGE as usize);
}

#[test]
fn a_damaged_cache_asks_for_a_refresh_and_a_refresh_repairs_it() {
    let dir = TempDir::new("catalog-damaged");
    drop(refreshed(&dir));
    let file = dir.path().join(CATALOGS_DIR).join("google-fonts.json");
    fs::write(&file, b"{ not json").expect("damage");

    let fetcher = Scripted::answering(Ok(index_fixture()));
    let c = catalogs(&dir, &fetcher);
    let e = c
        .families(GOOGLE, "", Page::default())
        .expect_err("damaged");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(e.message().contains("Refresh the catalog"), "{e}");
    c.refresh(GOOGLE).expect("refresh");
    assert_eq!(
        c.families(GOOGLE, "", Page::default())
            .expect("families")
            .total,
        5
    );
}

#[test]
fn an_unreadable_state_file_means_every_catalog_off() {
    let dir = TempDir::new("catalog-bad-state");
    let folder = dir.path().join(CATALOGS_DIR);
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join(STATE_FILE), b"[1, 2").expect("state");
    let fetcher = Scripted::failing();
    let c = catalogs(&dir, &fetcher);
    assert!(!c.list().expect("list")[0].enabled);
    assert_eq!(
        c.refresh(GOOGLE).expect_err("off").code(),
        ErrorCode::Conflict
    );
    assert!(
        c.set_enabled(GOOGLE, true).expect("on").enabled,
        "rewritten on a change"
    );
    assert_eq!(fetcher.calls(), 0);
}

#[test]
fn a_catalog_without_an_adapter_is_unsupported() {
    let dir = TempDir::new("catalog-none");
    let c = Catalogs::new(dir.path().to_path_buf(), Scripted::failing(), Vec::new());
    assert!(c.list().expect("list").is_empty());
    assert_eq!(
        c.set_enabled(GOOGLE, true).expect_err("no adapter").code(),
        ErrorCode::Unsupported
    );
}

#[test]
fn timestamps_are_rfc_3339_in_utc() {
    assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(rfc3339(1_792_800_000 + 3_661), "2026-10-24T01:01:01Z");
}

#[test]
fn the_shipped_catalogs_are_google_fonts_and_fontshare_all_off() {
    let dir = TempDir::new("catalog-shipped");
    // The real adapters over HTTP; listing makes no request.
    let c = Catalogs::open(dir.path().join(CATALOGS_DIR));
    let list = c.list().expect("list");
    let ids: Vec<CatalogId> = list.iter().map(|c| c.id).collect();
    assert_eq!(ids, CatalogId::ALL);
    assert_eq!(ids, [CatalogId::GoogleFonts, CatalogId::Fontshare]);
    let fontshare = &list[1];
    assert_eq!(fontshare.name, "Fontshare");
    assert_eq!(fontshare.homepage, "https://www.fontshare.com");
    assert!(list.iter().all(|c| !c.enabled), "off by default");
    let e = c.refresh(CatalogId::Fontshare).expect_err("off");
    assert_eq!(e.code(), ErrorCode::Conflict);
    assert_eq!(cache_file(CatalogId::Fontshare), "fontshare.json");
}
