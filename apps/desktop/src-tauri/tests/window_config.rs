//! The main window's settings in the shipped `tauri.conf.json` that features
//! depend on (ADR 0017).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use serde_json::Value;

fn config() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("json")
}

/// E34.1: Tauri's drag-drop handler replaces WebView2's drop target on
/// Windows, so with it on the page gets no `dragenter`/`dragleave` and a font
/// dragged out of the window is never activated (nor dropped on a collection).
#[test]
fn the_main_window_leaves_html_drag_and_drop_to_the_webview() {
    let window = &config()["app"]["windows"][0];
    assert_eq!(window["dragDropEnabled"], Value::Bool(false), "{window}");
}

/// Grid thumbnails are object URLs (`blob:`) used as CSS mask images, which
/// the content security policy checks under `img-src`. Without `blob:` there
/// every card stays blank and the only trace is a policy violation in the
/// webview; Playwright runs without the policy and cannot catch it.
#[test]
fn the_policy_lets_the_grid_show_blob_thumbnails() {
    let csp = config()["app"]["security"]["csp"]
        .as_str()
        .expect("csp string")
        .to_owned();
    let img_src = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("img-src "))
        .expect("img-src directive");
    assert!(
        img_src.split_whitespace().any(|s| s == "blob:"),
        "{img_src}"
    );
}
