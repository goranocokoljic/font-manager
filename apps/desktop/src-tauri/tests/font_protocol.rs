//! The `fmfont` URI scheme serves a face's font file by face id.

use std::path::{Path, PathBuf};

use fm_api::mock::MockApi;
use fm_api::preview::{
    ComparePreviewRequest, CssValues, CssValuesRequest, FontSource, PreviewImage, PreviewRequest,
    PreviewService, SampleText, SampleTextRequest, SizeFactor, SizeFactorRequest,
};
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId};
use fm_app_lib::{catalog_font_response, font_response};
use tauri::http::{StatusCode, header};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/fonts")
        .join(rel)
}

/// Faces 1–3 point at real files: a variable TTF, an OTF and a text file.
struct Files {
    junk: PathBuf,
}

impl Files {
    /// `name` keeps the junk file of each test apart (tests run in parallel).
    fn new(name: &str) -> std::io::Result<Self> {
        let junk = std::env::temp_dir().join(format!("fm-app-{name}-{}.ttf", std::process::id()));
        std::fs::write(&junk, b"not a font")?;
        Ok(Self { junk })
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.junk);
    }
}

impl PreviewService for Files {
    fn render_preview(&self, _: PreviewRequest) -> ApiResult<PreviewImage> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }

    fn compare_preview(&self, _: ComparePreviewRequest) -> ApiResult<PreviewImage> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }

    fn font_source(&self, face: FaceId) -> ApiResult<FontSource> {
        let path = match face.0 {
            1 => fixture("google-repo/asapsharp/AsapSharp[wdth,wght].ttf"),
            2 => first_with_extension("otf")
                .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "No OTF fixture."))?,
            3 => self.junk.clone(),
            _ => return Err(ApiError::new(ErrorCode::NotFound, "Face was not found.")),
        };
        Ok(FontSource {
            face,
            path: path.to_string_lossy().into_owned(),
            collection_index: 0,
            hash: String::new(),
        })
    }

    fn sample_texts(&self, _: SampleTextRequest) -> ApiResult<Vec<SampleText>> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }

    fn css_values(&self, _: CssValuesRequest) -> ApiResult<CssValues> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }

    fn size_factors(&self, _: SizeFactorRequest) -> ApiResult<Vec<SizeFactor>> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
}

fn first_with_extension(ext: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, ext: &str) -> Option<PathBuf> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        entries.iter().find_map(|p| {
            if p.is_dir() {
                walk(p, ext)
            } else {
                (p.extension()? == ext).then(|| p.clone())
            }
        })
    }
    walk(&fixture(""), ext)
}

fn header_of(res: &tauri::http::Response<Vec<u8>>, name: header::HeaderName) -> Option<&str> {
    res.headers().get(name)?.to_str().ok()
}

#[test]
fn serves_the_file_of_a_face_by_id() {
    let files = Files::new("serve").unwrap();
    let res = font_response(&files, "/1");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header_of(&res, header::CONTENT_TYPE).unwrap(), "font/ttf");
    assert_eq!(
        header_of(&res, header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "*"
    );
    let expected =
        std::fs::read(fixture("google-repo/asapsharp/AsapSharp[wdth,wght].ttf")).unwrap();
    assert_eq!(res.body(), &expected);
}

#[test]
fn an_otf_face_is_served_as_otf() {
    let files = Files::new("otf").unwrap();
    let res = font_response(&files, "/2");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header_of(&res, header::CONTENT_TYPE).unwrap(), "font/otf");
}

#[test]
fn an_unknown_face_is_not_found() {
    let files = Files::new("unknown").unwrap();
    let res = font_response(&files, "/99");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header_of(&res, header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "*"
    );
    assert_eq!(res.body(), b"Face was not found.");
}

#[test]
fn a_file_that_vanished_is_not_found() {
    // The mock's paths do not exist on disk, like a font moved since the scan.
    let res = font_response(&MockApi::new(), "/1");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[test]
fn a_file_that_is_not_a_font_is_refused() {
    let files = Files::new("notfont").unwrap();
    let res = font_response(&files, "/3");
    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(
        header_of(&res, header::CONTENT_TYPE)
            .unwrap()
            .starts_with("text/plain")
    );
}

#[test]
fn a_malformed_address_is_a_bad_request() {
    let files = Files::new("bad").unwrap();
    for path in ["", "/", "/abc", "/-1", "/1/2", "/../1", "/4294967296"] {
        let res = font_response(&files, path);
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{path:?}");
    }
}

// ---- catalog previews (E22.2) ----

use fm_api::catalog::{
    Catalog, CatalogFamily, CatalogFontRequest, CatalogFontSource, CatalogId, CatalogRef,
    CatalogService, ListCatalogFamiliesRequest, SetCatalogEnabledRequest,
};
use fm_api::common::Paged;
use std::sync::Mutex;

/// A catalog whose only family with a preview is "Noto Sans JP"; it records
/// the requests it was asked, and `off` refuses them like an off catalog.
#[derive(Default)]
struct Previews {
    off: bool,
    asked: Mutex<Vec<CatalogFontRequest>>,
}

impl CatalogService for Previews {
    fn list_catalogs(&self) -> ApiResult<Vec<Catalog>> {
        Ok(Vec::new())
    }
    fn set_catalog_enabled(&self, _: SetCatalogEnabledRequest) -> ApiResult<Catalog> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
    fn refresh_catalog(&self, _: CatalogRef) -> ApiResult<Catalog> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
    fn list_catalog_families(
        &self,
        _: ListCatalogFamiliesRequest,
    ) -> ApiResult<Paged<CatalogFamily>> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
    fn catalog_font_source(&self, req: CatalogFontRequest) -> ApiResult<CatalogFontSource> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(req.clone());
        if self.off {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "Google Fonts is turned off.",
            ));
        }
        if req.family != "Noto Sans JP" {
            return Err(ApiError::new(ErrorCode::NotFound, "Not in the list."));
        }
        Ok(CatalogFontSource {
            id: req.id,
            family: req.family,
            path: fixture("google-repo/asapsharp/AsapSharp[wdth,wght].ttf")
                .to_string_lossy()
                .into_owned(),
        })
    }
    fn download_family(
        &self,
        _: fm_api::catalog::DownloadFamilyRequest,
    ) -> ApiResult<fm_api::catalog::DownloadFamilyResponse> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
    fn list_catalog_downloads(&self) -> ApiResult<Vec<fm_api::catalog::CatalogDownload>> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
    fn check_updates(
        &self,
        _: fm_api::catalog::CheckUpdatesRequest,
    ) -> ApiResult<Vec<fm_api::catalog::CatalogUpdate>> {
        Err(ApiError::new(ErrorCode::Unsupported, "not in this test"))
    }
}

/// The path the UI builds: the family name's UTF-8 as hex.
fn catalog_path(family: &str) -> String {
    let hex: String = family.bytes().map(|b| format!("{b:02x}")).collect();
    format!("/catalog-googleFonts-{hex}")
}

#[test]
fn a_catalog_preview_is_served_from_the_core_by_family_name() {
    let api = Previews::default();
    let res = catalog_font_response(&api, &catalog_path("Noto Sans JP")).expect("catalog path");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(header_of(&res, header::CONTENT_TYPE).unwrap(), "font/ttf");
    assert_eq!(
        header_of(&res, header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
        "*"
    );
    let asked = api.asked.lock().unwrap().clone();
    assert_eq!(
        asked,
        vec![CatalogFontRequest {
            id: CatalogId::GoogleFonts,
            family: "Noto Sans JP".into()
        }]
    );
}

#[test]
fn a_non_ascii_family_name_round_trips_through_the_path() {
    let api = Previews::default();
    let res = catalog_font_response(&api, &catalog_path("Zen Kaku Gothic Ñ")).expect("path");
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(api.asked.lock().unwrap()[0].family, "Zen Kaku Gothic Ñ");
}

#[test]
fn an_off_catalog_preview_is_forbidden() {
    let api = Previews {
        off: true,
        ..Previews::default()
    };
    let res = catalog_font_response(&api, &catalog_path("Noto Sans JP")).expect("path");
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[test]
fn a_face_path_is_not_a_catalog_path_and_a_bad_catalog_path_is_refused() {
    let api = Previews::default();
    assert!(catalog_font_response(&api, "/1").is_none());
    for path in [
        "/catalog-",
        "/catalog-googleFonts",
        "/catalog-googleFonts-",
        "/catalog-googleFonts-4",
        "/catalog-googleFonts-zz",
        "/catalog-adobe-4c61746f",
        "/catalog-googleFonts-ff",
    ] {
        let res = catalog_font_response(&api, path).expect("catalog path");
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{path:?}");
    }
    assert!(
        api.asked.lock().unwrap().is_empty(),
        "no bad path reaches the core"
    );
}
