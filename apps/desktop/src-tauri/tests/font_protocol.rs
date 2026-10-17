//! The `fmfont` URI scheme serves a face's font file by face id.

use std::path::{Path, PathBuf};

use fm_api::mock::MockApi;
use fm_api::preview::{FontSource, PreviewImage, PreviewRequest, PreviewService};
use fm_api::{ApiError, ApiResult, ErrorCode, FaceId};
use fm_app_lib::font_response;
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
