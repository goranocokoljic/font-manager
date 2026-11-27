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

fn first_codes(api: &CoreApi, n: usize) -> Vec<String> {
    api.list_languages()
        .expect("languages")
        .into_iter()
        .take(n)
        .map(|p| p.code)
        .collect()
}

fn set_mine(api: &CoreApi, ids: &[&str]) {
    api.update_settings(SettingsUpdate {
        my_languages: Some(ids.iter().map(|s| (*s).to_owned()).collect()),
        ..SettingsUpdate::default()
    })
    .expect("saved");
}

/// E58.1: a change of `my_languages` is announced, and `list_languages`
/// answers in the new order at once; an update that leaves the list as it
/// is announces nothing.
#[test]
fn a_change_of_my_languages_is_an_event_and_reorders_the_list() {
    let lib = Lib::new("api-language-order");
    let rx = lib.api.subscribe();
    assert_eq!(first_codes(&lib.api, 2), ["en", "es"], "the file's order");

    set_mine(&lib.api, &["sr-Latn", "de", "ja"]);
    assert_eq!(rx.recv_timeout(WAIT), Ok(Event::LanguageOrderChanged));
    assert_eq!(first_codes(&lib.api, 4), ["sr-Latn", "de", "ja", "en"]);

    set_mine(&lib.api, &["sr-Latn", "de", "ja"]);
    lib.api
        .update_settings(SettingsUpdate {
            theme: Some(fm_api::settings::Theme::Dark),
            ..SettingsUpdate::default()
        })
        .expect("saved");
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "nothing changed the order"
    );

    set_mine(&lib.api, &[]);
    assert_eq!(rx.recv_timeout(WAIT), Ok(Event::LanguageOrderChanged));
    assert_eq!(first_codes(&lib.api, 1), ["en"]);
}

/// E58.1: the system's language the shell hands over leads while no
/// language is set, and its change is announced once.
#[test]
fn the_system_language_leads_until_the_user_sets_languages() {
    let lib = Lib::new("api-language-system");
    let rx = lib.api.subscribe();
    lib.api.set_system_language("sr");
    assert_eq!(rx.recv_timeout(WAIT), Ok(Event::LanguageOrderChanged));
    assert_eq!(first_codes(&lib.api, 3), ["sr-Cyrl", "sr-Latn", "en"]);
    lib.api.set_system_language("sr");
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the same language again is no change"
    );
    set_mine(&lib.api, &["fr"]);
    assert_eq!(rx.recv_timeout(WAIT), Ok(Event::LanguageOrderChanged));
    assert_eq!(first_codes(&lib.api, 2), ["fr", "en"]);
}
