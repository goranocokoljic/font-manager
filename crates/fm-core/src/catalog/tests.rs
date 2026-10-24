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

// ---- previews (E22.2) ----

fn fixture(rel: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(rel),
    )
    .expect("fixture")
}

/// A fetcher that answers by address: the index, Lato's file list, and a
/// font (or `font`) for any other URL; it records every URL it was asked.
struct Routed {
    font: Mutex<Vec<u8>>,
    urls: Mutex<Vec<String>>,
}

impl Routed {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            font: Mutex::new(fixture(
                "fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf",
            )),
            urls: Mutex::new(Vec::new()),
        })
    }

    fn urls(&self) -> Vec<String> {
        lock(&self.urls).clone()
    }
}

impl Fetcher for Routed {
    fn get(&self, url: &str) -> FmResult<Vec<u8>> {
        lock(&self.urls).push(url.to_owned());
        if url.contains("/metadata/fonts") {
            Ok(index_fixture())
        } else if url.contains("/download/list") {
            Ok(fixture("catalogs/google-fonts/download-list-lato.json"))
        } else {
            Ok(lock(&self.font).clone())
        }
    }
}

fn previewing(dir: &TempDir, fetcher: &Arc<Routed>) -> Catalogs {
    Catalogs::new(
        dir.path().join(CATALOGS_DIR),
        Arc::clone(fetcher) as Arc<dyn Fetcher>,
        vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
    )
    .with_previews_in(dir.path().join("cache").join(CATALOGS_DIR))
}

#[test]
fn a_preview_is_fetched_once_into_the_cache_folder() {
    let dir = TempDir::new("catalog-preview");
    let fetcher = Routed::new();
    let c = previewing(&dir, &fetcher);
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");

    let path = c.font_source(GOOGLE, "Lato").expect("preview");
    assert!(
        path.starts_with(
            dir.path()
                .join("cache")
                .join(CATALOGS_DIR)
                .join("google-fonts")
        )
    );
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some("ttf"));
    assert_eq!(
        fs::read(&path).expect("file"),
        fixture("fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf")
    );
    let urls = fetcher.urls();
    assert_eq!(urls.len(), 3, "index, file list, file: {urls:?}");
    assert!(
        urls[2].ends_with("S6uyw4BMUTPHvxk6XweuBCY.ttf"),
        "Lato-Regular: {urls:?}"
    );

    // The second time, and after a restart, it is read from the cache.
    assert_eq!(c.font_source(GOOGLE, "Lato").expect("again"), path);
    let restarted = previewing(&dir, &fetcher);
    assert_eq!(
        restarted.font_source(GOOGLE, "Lato").expect("restart"),
        path
    );
    assert_eq!(fetcher.urls().len(), 3, "no new request");
}

#[test]
fn a_preview_is_refused_while_off_or_for_an_unlisted_family_before_any_request() {
    let dir = TempDir::new("catalog-preview-refused");
    let fetcher = Routed::new();
    let c = previewing(&dir, &fetcher);
    let e = c.font_source(GOOGLE, "Lato").expect_err("off");
    assert_eq!(e.code(), ErrorCode::Conflict);
    c.set_enabled(GOOGLE, true).expect("on");
    let e = c.font_source(GOOGLE, "Lato").expect_err("never refreshed");
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert!(fetcher.urls().is_empty(), "nothing fetched");

    c.refresh(GOOGLE).expect("refresh");
    let e = c
        .font_source(GOOGLE, "../../etc/passwd")
        .expect_err("unlisted");
    assert_eq!(e.code(), ErrorCode::NotFound);
    c.font_source(GOOGLE, "Lato").expect("preview");
    c.set_enabled(GOOGLE, false).expect("off");
    let e = c
        .font_source(GOOGLE, "Lato")
        .expect_err("off again, even cached");
    assert_eq!(e.code(), ErrorCode::Conflict);
    assert_eq!(fetcher.urls().len(), 3, "only the refresh and one preview");
}

#[test]
fn a_preview_that_is_not_a_font_is_not_kept() {
    let dir = TempDir::new("catalog-preview-junk");
    let fetcher = Routed::new();
    *lock(&fetcher.font) = b"<html>not a font</html>".to_vec();
    let c = previewing(&dir, &fetcher);
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");
    let e = c.font_source(GOOGLE, "Lato").expect_err("junk");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(e.message().contains("Lato"), "{}", e.message());
    let previews = dir
        .path()
        .join("cache")
        .join(CATALOGS_DIR)
        .join("google-fonts");
    assert!(
        !previews.exists() || fs::read_dir(&previews).expect("dir").next().is_none(),
        "nothing kept"
    );
}

#[test]
fn a_failed_preview_fetch_is_an_error_that_names_the_family() {
    let dir = TempDir::new("catalog-preview-offline");
    let fetcher = Scripted::answering(Ok(index_fixture()));
    let c = Catalogs::new(
        dir.path().join(CATALOGS_DIR),
        Arc::clone(&fetcher) as Arc<dyn Fetcher>,
        vec![Arc::new(GoogleFonts::with_base_url("http://127.0.0.1:9"))],
    )
    .with_previews_in(dir.path().join("cache"));
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");
    fetcher.set(Err(FmError::new(ErrorCode::Io, "offline")));
    let e = c.font_source(GOOGLE, "Lato").expect_err("offline");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(
        e.message()
            .starts_with("The preview of Lato could not be fetched.")
    );
}

#[test]
fn the_preview_file_is_the_upright_regular_else_a_variable_else_an_upright_one() {
    let file = |name: &str| CatalogFile {
        name: name.to_owned(),
        url: format!("https://x/{name}"),
        variable: name.contains('['),
    };
    let pick = |files: &[CatalogFile]| preview_file(files).map(|(f, ext)| (f.name.clone(), ext));
    assert_eq!(
        pick(&[
            file("OFL.txt"),
            file("Lato-Italic.ttf"),
            file("Lato-Bold.ttf"),
            file("Lato-Regular.ttf"),
        ]),
        Some(("Lato-Regular.ttf".to_owned(), "ttf"))
    );
    assert_eq!(
        pick(&[
            file("Inter-Italic[opsz,wght].ttf"),
            file("Inter[opsz,wght].ttf"),
            file("static/Inter-Black.ttf"),
        ]),
        Some(("Inter[opsz,wght].ttf".to_owned(), "ttf"))
    );
    assert_eq!(
        pick(&[file("A-ThinItalic.OTF"), file("A-Thin.OTF")]),
        Some(("A-Thin.OTF".to_owned(), "otf"))
    );
    assert_eq!(
        pick(&[file("A-Italic.woff2")]),
        Some(("A-Italic.woff2".to_owned(), "woff2"))
    );
    assert_eq!(pick(&[file("OFL.txt"), file("README")]), None);
}

#[test]
fn preview_names_are_safe_file_names() {
    let a = preview_stem("Noto Sans");
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, preview_stem("Noto-Sans"));
    assert_eq!(preview_stem("../x").len(), 32);
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

// ---- downloads (E22.3), over the 127.0.0.1 fixture server ----

const REGULAR: &str = "fonts/google-repo/cagliostro/Cagliostro-Regular.ttf";
const BOLD: &str = "fonts/google-repo/asul/Asul-Bold.ttf";
const VARIABLE: &str = "fonts/google-repo/castoro/Castoro[wght].ttf";

/// A Google Fonts server with the recorded index and a Lato file list of two
/// static files, one variable file and the license, served from itself;
/// `bold` answers the bold file (status and body).
pub(crate) fn lato_server(bold: (u16, Vec<u8>)) -> crate::test_support::LocalServer {
    crate::test_support::LocalServer::start_with(|base| {
        let list = format!(
            r#")]}}'
{{"manifest":{{"files":[{{"filename":"OFL.txt","contents":"SIL Open Font License"}}],
"fileRefs":[
 {{"filename":"static/Lato-Regular.ttf","url":"{base}/f/regular"}},
 {{"filename":"static/Lato-Bold.ttf","url":"{base}/f/bold"}},
 {{"filename":"Lato[wght].ttf","url":"{base}/f/variable"}}]}}}}"#
        );
        vec![
            ("/metadata/fonts".to_owned(), 200, index_fixture()),
            (
                "/download/list?family=Lato".to_owned(),
                200,
                list.into_bytes(),
            ),
            ("/f/regular".to_owned(), 200, fixture(REGULAR)),
            ("/f/bold".to_owned(), bold.0, bold.1),
            ("/f/variable".to_owned(), 200, fixture(VARIABLE)),
        ]
    })
}

/// A file name no file system takes (over 255 characters): saving it fails
/// on every OS, after the files before it were placed.
pub(crate) fn unsavable_name() -> String {
    format!("Lato-{}.ttf", "Bold".repeat(70))
}

/// As [`lato_server`], but the second static file has [`unsavable_name`]:
/// every file fetches and checks out, and saving stops at the second one.
pub(crate) fn stopping_server() -> crate::test_support::LocalServer {
    crate::test_support::LocalServer::start_with(|base| {
        let list = format!(
            r#"{{"manifest":{{"files":[],"fileRefs":[
 {{"filename":"static/Lato-Regular.ttf","url":"{base}/f/regular"}},
 {{"filename":"static/{}","url":"{base}/f/bold"}},
 {{"filename":"static/Lato-Thin.ttf","url":"{base}/f/regular"}}]}}}}"#,
            unsavable_name()
        );
        vec![
            ("/metadata/fonts".to_owned(), 200, index_fixture()),
            (
                "/download/list?family=Lato".to_owned(),
                200,
                list.into_bytes(),
            ),
            ("/f/regular".to_owned(), 200, fixture(REGULAR)),
            ("/f/bold".to_owned(), 200, fixture(BOLD)),
        ]
    })
}

/// The bytes of a fixture font of the Lato server.
pub(crate) fn lato_font(which: &str) -> Vec<u8> {
    fixture(match which {
        "bold" => BOLD,
        "variable" => VARIABLE,
        _ => REGULAR,
    })
}

/// Catalogs on `dir` over HTTP to `server`, Google Fonts on and refreshed.
fn downloading(dir: &TempDir, server: &crate::test_support::LocalServer) -> Catalogs {
    let c = Catalogs::new(
        dir.path().join(CATALOGS_DIR),
        Arc::new(HttpFetcher::new()),
        vec![Arc::new(GoogleFonts::with_base_url(server.base()))],
    );
    c.set_enabled(GOOGLE, true).expect("on");
    c.refresh(GOOGLE).expect("refresh");
    c
}

/// The names in `dir`, sorted; none when it is not there.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_download_writes_the_static_files_into_a_folder_named_after_the_family() {
    let dir = TempDir::new("catalog-download");
    let server = lato_server((200, fixture(BOLD)));
    let c = downloading(&dir, &server);
    let source = dir.path().join("My Fonts");
    fs::create_dir_all(&source).expect("source");

    let got = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, &source)
        .expect("download");
    assert_eq!(got.folder, source.join("Lato"));
    assert_eq!(got.kind, CatalogFileKind::Static);
    assert_eq!(got.family_id, "Lato");
    assert_eq!(
        got.version.as_deref(),
        Some("2025-09-16"),
        "Lato's lastModified"
    );
    assert!(got.skipped.is_empty());
    let names: Vec<&str> = got.written.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["Lato-Regular.ttf", "Lato-Bold.ttf"],
        "no `static/`, no variable"
    );
    for (file, rel) in got.written.iter().zip([REGULAR, BOLD]) {
        assert_eq!(file.path, source.join("Lato").join(&file.name));
        let bytes = fs::read(&file.path).expect("written");
        assert_eq!(bytes, fixture(rel));
        assert_eq!(file.hash, *blake3::hash(&bytes).as_bytes());
    }
    assert_eq!(
        listing(&source.join("Lato")),
        ["Lato-Bold.ttf", "Lato-Regular.ttf"],
        "no temporary file left"
    );
}

#[test]
fn a_variable_download_writes_only_the_variable_files() {
    let dir = TempDir::new("catalog-download-variable");
    let server = lato_server((200, fixture(BOLD)));
    let c = downloading(&dir, &server);
    let got = c
        .download(GOOGLE, "Lato", CatalogFileKind::Variable, dir.path())
        .expect("download");
    assert_eq!(got.kind, CatalogFileKind::Variable);
    let names: Vec<&str> = got.written.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Lato[wght].ttf"]);
    assert_eq!(
        fs::read(&got.written[0].path).expect("file"),
        fixture(VARIABLE)
    );
}

#[test]
fn a_download_never_replaces_a_file_already_there() {
    let dir = TempDir::new("catalog-download-keep");
    let server = lato_server((200, fixture(BOLD)));
    let c = downloading(&dir, &server);
    let folder = dir.path().join("Lato");
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("Lato-Bold.ttf"), b"the user's own file").expect("theirs");

    let got = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
        .expect("download");
    assert_eq!(got.skipped, ["Lato-Bold.ttf"]);
    let names: Vec<&str> = got.written.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Lato-Regular.ttf"]);
    assert_eq!(
        fs::read(folder.join("Lato-Bold.ttf")).expect("theirs"),
        b"the user's own file"
    );

    // Again: everything is there, nothing is written or replaced.
    let again = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
        .expect("again");
    assert!(again.written.is_empty());
    assert_eq!(again.skipped, ["Lato-Regular.ttf", "Lato-Bold.ttf"]);
    assert_eq!(listing(&folder), ["Lato-Bold.ttf", "Lato-Regular.ttf"]);
}

#[test]
fn a_failed_download_leaves_no_file() {
    for (label, bold) in [
        ("missing", (404, b"not found".to_vec())),
        ("not-a-font", (200, b"<html>an error page</html>".to_vec())),
    ] {
        let dir = TempDir::new(&format!("catalog-download-{label}"));
        let server = lato_server(bold);
        let c = downloading(&dir, &server);
        let e = c
            .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
            .expect_err(label);
        assert_eq!(e.code(), ErrorCode::Io, "{label}");
        assert!(
            e.message().starts_with("Lato could not be downloaded."),
            "{label}: {}",
            e.message()
        );
        assert!(
            listing(&dir.path().join("Lato")).is_empty(),
            "{label}: no file, not even the one fetched before the failure"
        );
    }
}

#[test]
fn a_download_is_refused_while_off_or_for_an_unlisted_family_before_any_request() {
    let dir = TempDir::new("catalog-download-refused");
    let server = lato_server((200, fixture(BOLD)));
    let c = Catalogs::new(
        dir.path().join(CATALOGS_DIR),
        Arc::new(HttpFetcher::new()),
        vec![Arc::new(GoogleFonts::with_base_url(server.base()))],
    );
    let e = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
        .expect_err("off");
    assert_eq!(e.code(), ErrorCode::Conflict);
    c.set_enabled(GOOGLE, true).expect("on");
    let e = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
        .expect_err("never refreshed");
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert_eq!(server.hits(), 0, "nothing fetched");

    c.refresh(GOOGLE).expect("refresh");
    let e = c
        .download(GOOGLE, "Comic Sans", CatalogFileKind::Static, dir.path())
        .expect_err("unlisted");
    assert_eq!(e.code(), ErrorCode::NotFound);
    let e = c
        .download(
            GOOGLE,
            "Lato",
            CatalogFileKind::Static,
            &dir.path().join("gone"),
        )
        .expect_err("no folder");
    assert_eq!(e.code(), ErrorCode::NotFound);
    assert_eq!(server.hits(), 1, "only the refresh");
    assert!(!dir.path().join("Lato").exists() && !dir.path().join("gone").exists());
}

#[test]
fn a_file_that_cannot_be_saved_stops_the_download_and_keeps_what_was_placed() {
    let dir = TempDir::new("catalog-download-stopped");
    let server = stopping_server();
    let c = downloading(&dir, &server);
    let got = c
        .download(GOOGLE, "Lato", CatalogFileKind::Static, dir.path())
        .expect("stopped after one file, not an error");
    let names: Vec<&str> = got.written.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Lato-Regular.ttf"], "the file before the failure");
    assert!(got.skipped.is_empty());
    let e = got.stopped.expect("why it stopped");
    assert_eq!(e.code(), ErrorCode::Io);
    assert!(
        e.message().starts_with("Lato could not be downloaded."),
        "{}",
        e.message()
    );
    assert_eq!(
        listing(&dir.path().join("Lato")),
        ["Lato-Regular.ttf"],
        "the placed file is whole and stays; no temporary file, nothing after the failure"
    );
    assert_eq!(
        fs::read(dir.path().join("Lato").join("Lato-Regular.ttf")).expect("file"),
        fixture(REGULAR)
    );

    // When the very first file cannot be saved, nothing was written: an error.
    let empty = TempDir::new("catalog-download-stopped-first");
    let first = Catalogs::new(
        empty.path().join(CATALOGS_DIR),
        Arc::new(HttpFetcher::new()),
        vec![Arc::new(GoogleFonts::with_base_url(server.base()))],
    );
    first.set_enabled(GOOGLE, true).expect("on");
    first.refresh(GOOGLE).expect("refresh");
    let folder = empty.path().join("Lato");
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("Lato-Regular.ttf"), b"the user's own file").expect("theirs");
    let e = first
        .download(GOOGLE, "Lato", CatalogFileKind::Static, empty.path())
        .expect_err("Regular skipped, then the second file fails: nothing written");
    assert_eq!(e.code(), ErrorCode::Io);
    assert_eq!(listing(&folder), ["Lato-Regular.ttf"]);
    assert_eq!(
        fs::read(folder.join("Lato-Regular.ttf")).expect("theirs"),
        b"the user's own file"
    );
}

#[test]
fn download_files_pick_the_kind_asked_for_else_the_other() {
    let file = |name: &str| CatalogFile {
        name: name.to_owned(),
        url: format!("https://x/{name}"),
        variable: name.contains('['),
    };
    let names = |picked: Option<(CatalogFileKind, Vec<(String, &CatalogFile)>)>| {
        picked.map(|(kind, files)| (kind, files.into_iter().map(|(n, _)| n).collect::<Vec<_>>()))
    };
    let only_static = [file("OFL.txt"), file("A-Regular.ttf"), file("A-Bold.otf")];
    assert_eq!(
        names(download_files(&only_static, CatalogFileKind::Variable)),
        Some((
            CatalogFileKind::Static,
            vec!["A-Regular.ttf".to_owned(), "A-Bold.otf".to_owned()]
        )),
        "no variable file: the static ones"
    );
    let only_variable = [file("A[wght].ttf"), file("A-Italic[wght].ttf")];
    assert_eq!(
        names(download_files(&only_variable, CatalogFileKind::Static)).map(|(k, n)| (k, n.len())),
        Some((CatalogFileKind::Variable, 2))
    );
    // A name from the catalog never leaves the family's folder, and comes once.
    let tricky = [
        file("../../evil.ttf"),
        file("static\\..\\B.ttf"),
        file("sub/B.TTF"),
        file("README.md"),
    ];
    assert_eq!(
        names(download_files(&tricky, CatalogFileKind::Static)),
        Some((
            CatalogFileKind::Static,
            vec!["evil.ttf".to_owned(), "B.ttf".to_owned()]
        ))
    );
    assert_eq!(
        names(download_files(&[file("OFL.txt")], CatalogFileKind::Static)),
        None
    );
}

#[test]
fn safe_file_names_stay_inside_the_folder_on_every_os() {
    assert_eq!(safe_file_name("Lato").as_deref(), Some("Lato"));
    assert_eq!(
        safe_file_name("Noto Sans JP").as_deref(),
        Some("Noto Sans JP")
    );
    assert_eq!(safe_file_name("Été 2").as_deref(), Some("Été 2"));
    assert_eq!(safe_file_name("../x").as_deref(), Some("_x"));
    assert_eq!(safe_file_name("a/b\\c:d*e?").as_deref(), Some("a_b_c_d_e_"));
    assert_eq!(safe_file_name(" trailing. ").as_deref(), Some("trailing"));
    assert_eq!(safe_file_name("CON").as_deref(), Some("_CON"));
    assert_eq!(safe_file_name("com1.ttf").as_deref(), Some("_com1.ttf"));
    assert_eq!(safe_file_name("Console").as_deref(), Some("Console"));
    assert_eq!(safe_file_name("..").as_deref(), None);
    assert_eq!(safe_file_name("/").as_deref(), None);
    assert_eq!(safe_file_name("").as_deref(), None);
}
