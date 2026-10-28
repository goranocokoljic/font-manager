use std::path::Path;

use fm_api::common::{FontFormat, Health, Slant};
use fm_api::library::FaceSummary;
use fm_api::{FaceId, FamilyId};

use super::*;

#[test]
fn a_posix_path_is_an_absolute_file_url() {
    assert_eq!(
        file_url(Path::new("/Users/demo/Fonts/Inter[wght].ttf")),
        "file:///Users/demo/Fonts/Inter%5Bwght%5D.ttf"
    );
    assert_eq!(
        file_url(Path::new("/home/a b/Font #1.otf")),
        "file:///home/a%20b/Font%20%231.otf"
    );
}

#[test]
fn non_ascii_names_are_percent_encoded_utf8() {
    assert_eq!(
        file_url(Path::new("/fonts/עברית/é.ttf")),
        "file:///fonts/%D7%A2%D7%91%D7%A8%D7%99%D7%AA/%C3%A9.ttf"
    );
}

#[test]
fn a_windows_drive_path_gets_forward_slashes() {
    assert_eq!(
        file_url(Path::new(r"C:\Users\demo\My Fonts\Inter.ttf")),
        "file:///C:/Users/demo/My%20Fonts/Inter.ttf"
    );
    assert_eq!(
        file_url(Path::new(r"\\?\D:\fonts\a.ttf")),
        "file:///D:/fonts/a.ttf"
    );
}

#[test]
fn a_unc_path_names_the_server_as_host() {
    assert_eq!(
        file_url(Path::new(r"\\server\share\fonts\a.ttf")),
        "file://server/share/fonts/a.ttf"
    );
    assert_eq!(
        file_url(Path::new(r"\\?\UNC\server\share\a.ttf")),
        "file://server/share/a.ttf"
    );
    assert_eq!(file_url(Path::new(r"\\server")), "file://server");
}

#[test]
fn a_backslash_in_a_posix_name_is_encoded_not_a_separator() {
    assert_eq!(
        file_url(Path::new(r"/fonts/a\b.ttf")),
        "file:///fonts/a%5Cb.ttf"
    );
}

#[cfg(unix)]
#[test]
fn a_non_utf8_posix_name_keeps_its_bytes() {
    use std::os::unix::ffi::OsStrExt as _;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"/fonts/\xFFx.ttf"));
    assert_eq!(file_url(path), "file:///fonts/%FFx.ttf");
}

#[test]
fn the_payload_carries_the_names_and_the_url() {
    let face = FaceSummary {
        id: FaceId(7),
        family: FamilyId(2),
        family_name: "Inter".into(),
        style: "Bold Italic".into(),
        postscript_name: "Inter-BoldItalic".into(),
        weight: 700,
        width: 5,
        slant: Slant::Italic,
        format: FontFormat::Ttf,
        is_variable: false,
        active: None,
        health: Health::Ok,
    };
    let p = payload(&face, Path::new("/f/Inter-BoldItalic.ttf"));
    assert_eq!(p.postscript_name, "Inter-BoldItalic");
    assert_eq!(p.family, "Inter");
    assert_eq!(p.style, "Bold Italic");
    assert_eq!(p.file_url, "file:///f/Inter-BoldItalic.ttf");
}
