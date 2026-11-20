//! The languages the user works in through the API (E56.3): fm-core's own
//! reads at start (the preview cache limit, the watcher, activation start,
//! the setup, the app info) leave the one report of a dropped language id to
//! the first `get_settings` an API caller makes.

use super::*;

#[test]
fn the_start_up_reads_leave_the_dropped_language_report_to_the_api_caller() {
    let dir = TempDir::new("api-my-languages-gone");
    let base = dir.path();
    let paths = AppPaths {
        data: base.join("data"),
        config: base.join("config"),
        cache: base.join("cache"),
        logs: base.join("logs"),
        library: base.join("Fonts"),
    };
    crate::startup::prepare(&paths).expect("app folders");
    std::fs::write(
        paths.config_file(),
        r#"{"myLanguages":["de","xx-Gone"],"theme":"dark"}"#,
    )
    .expect("config");
    let index = Index::open(&paths.data.join(INDEX_FILE))
        .expect("index")
        .index;
    let api = CoreApi::open(
        &paths,
        index,
        Arc::new(MockActivator::new()) as Arc<dyn FontActivator>,
    )
    .with_watcher();
    api.start().expect("start");
    api.setup_status().expect("setup status");
    api.app_info().expect("app info");
    let quiet = api.read_settings().expect("read");
    assert_eq!(quiet.my_languages, ["de"], "left out of every answer");
    assert!(quiet.dropped_languages.is_empty());

    let first = api.get_settings().expect("settings");
    assert_eq!(first.my_languages, ["de"]);
    assert_eq!(
        first.dropped_languages,
        ["xx-Gone"],
        "reported to the caller"
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(paths.config_file()).expect("file"))
            .expect("json");
    assert_eq!(json["myLanguages"], serde_json::json!(["de"]));
    assert_eq!(json["theme"], "dark");
    assert!(
        api.get_settings()
            .expect("settings")
            .dropped_languages
            .is_empty(),
        "reported once"
    );
}
