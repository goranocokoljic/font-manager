//! Tauri commands round-trip through the real IPC layer to `fm-api`.

use std::error::Error;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use fm_api::activation::{ActivationOutcome, ActiveFace, OutcomeStatus};
use fm_api::common::{Paged, Slant};
use fm_api::library::{
    AppInfo, FaceDetail, Family, FamilySummary, GlyphPage, GlyphsRequest, LibraryService,
    ListFamiliesRequest, Source, SourceKind,
};
use fm_api::mock::MockApi;
use fm_api::organization::{Collection, Tag};
use fm_api::preview::{PreviewImage, PreviewService, SampleText, SampleTextRequest};
use fm_api::search::{LanguagePreset, SearchRequest, SearchResponse};
use fm_api::settings::{AppFolders, Settings, SettingsUpdate, Theme};
use fm_api::{
    API_VERSION, ActivationService, ApiResult, Event, FaceId, FamilyId, JobId, SearchService,
    SettingsService, SourceId,
};
use fm_app_lib::{AppState, EVENT_CHANNEL, setup, setup_state};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, WebviewWindow};

type IpcResponse = Result<InvokeResponseBody, serde_json::Value>;

/// Invokes `cmd` on a mock app; the outer error is a failure to set the app up.
fn invoke(cmd: &str) -> Result<IpcResponse, Box<dyn Error>> {
    invoke_with(cmd, serde_json::json!({}))
}

/// Invokes `cmd` with JSON arguments, as the webview sends them.
fn invoke_with(cmd: &str, args: serde_json::Value) -> Result<IpcResponse, Box<dyn Error>> {
    let (_app, webview) = app()?;
    send(&webview, cmd, args)
}

/// A mock app over a fresh `MockApi`, with its main window.
fn app() -> Result<(App<MockRuntime>, WebviewWindow<MockRuntime>), Box<dyn Error>> {
    app_on(setup(mock_builder(), Arc::new(MockApi::new())))
}

/// A mock app built from this builder, with its main window.
fn app_on(
    builder: tauri::Builder<MockRuntime>,
) -> Result<(App<MockRuntime>, WebviewWindow<MockRuntime>), Box<dyn Error>> {
    let app = builder.build(mock_context(noop_assets()))?;
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    Ok((app, webview))
}

/// Sends one command through the IPC layer of `webview`.
fn send(
    webview: &WebviewWindow<MockRuntime>,
    cmd: &str,
    args: serde_json::Value,
) -> Result<IpcResponse, Box<dyn Error>> {
    let url = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    Ok(get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse()?,
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    ))
}

#[test]
fn get_app_info_round_trips_through_fm_api() {
    let info: AppInfo = invoke("get_app_info")
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(info, MockApi::new().app_info().unwrap());
    assert_eq!(info.api_version, API_VERSION);
}

#[test]
fn get_app_info_uses_the_camel_case_wire_format() {
    let json: serde_json::Value = invoke("get_app_info")
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert!(json.get("apiVersion").is_some(), "{json}");
    assert!(json.get("api_version").is_none(), "{json}");
}

#[test]
fn an_unknown_command_is_rejected() {
    assert!(invoke("no_such_command").unwrap().is_err());
}

#[test]
fn get_face_returns_the_face_detail() {
    let detail: FaceDetail = invoke_with("get_face", serde_json::json!({ "face": 1 }))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(detail, MockApi::new().get_face(FaceId(1)).unwrap());
}

#[test]
fn list_glyphs_returns_a_page_of_the_glyph_map() {
    let page: GlyphPage = invoke_with(
        "list_glyphs",
        serde_json::json!({ "req": { "face": 1, "query": "U+0041" } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    let expected = MockApi::new()
        .list_glyphs(GlyphsRequest {
            face: FaceId(1),
            block: None,
            query: Some("U+0041".into()),
            page: Default::default(),
        })
        .unwrap();
    assert_eq!(page, expected);
    assert_eq!(page.items[0].code_point, 0x41);
}

#[test]
fn families_round_trip_through_fm_api() {
    let api = MockApi::new();
    let page: Paged<FamilySummary> = invoke_with("list_families", serde_json::json!({ "req": {} }))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(
        page,
        api.list_families(ListFamiliesRequest::default()).unwrap()
    );
    let family: Family = invoke_with("get_family", serde_json::json!({ "family": 1 }))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(family, api.get_family(FamilyId(1)).unwrap());
    let err = invoke_with("get_family", serde_json::json!({ "family": 999 }))
        .unwrap()
        .unwrap_err();
    assert_eq!(err["code"], "notFound", "{err}");
}

#[test]
fn search_returns_hits_and_filter_counts() {
    let api = MockApi::new();
    let req = serde_json::json!({ "req": { "query": "mono" } });
    let r: SearchResponse = invoke_with("search", req)
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    let want = api
        .search(SearchRequest {
            query: "mono".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r, want);
    assert!(r.counts.is_some());
    let langs: Vec<LanguagePreset> = invoke("list_languages")
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(langs, api.list_languages().unwrap());
}

#[test]
fn create_collection_saves_a_smart_collection_with_its_rules() {
    let req = serde_json::json!({
        "req": { "name": "Italics", "rules": { "slant": "italic" } }
    });
    let c: Collection = invoke_with("create_collection", req)
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(c.name, "Italics");
    assert_eq!(c.rules.unwrap().slant, Some(Slant::Italic));
    let err = invoke_with(
        "create_collection",
        serde_json::json!({ "req": { "name": " " } }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(err["code"], "invalidRequest", "{err}");
}

#[test]
fn collections_are_nested_reordered_filled_and_deleted_over_ipc() {
    // One app for every call: the collections live in its MockApi.
    let (_app, webview) = app().unwrap();
    let create = |name: &str, parent: Option<u32>| -> Collection {
        send(
            &webview,
            "create_collection",
            serde_json::json!({ "req": { "name": name, "parent": parent } }),
        )
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap()
    };
    let list = || -> Vec<Collection> {
        send(&webview, "list_collections", serde_json::json!({}))
            .unwrap()
            .unwrap()
            .deserialize()
            .unwrap()
    };
    let a = create("A", None);
    let b = create("B", None);
    let moved: Collection = send(
        &webview,
        "move_collection",
        serde_json::json!({ "req": { "collection": b.id, "parent": a.id, "position": 0 } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(moved.parent, Some(a.id));
    let renamed: Collection = send(
        &webview,
        "rename_collection",
        serde_json::json!({ "req": { "collection": b.id, "name": "Brand" } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(renamed.name, "Brand");
    let filled: Collection = send(
        &webview,
        "add_to_collection",
        serde_json::json!({ "req": { "collection": b.id, "faces": [3], "families": [1] } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert!(filled.face_count >= 2, "{filled:?}");
    let emptied: Collection = send(
        &webview,
        "remove_from_collection",
        serde_json::json!({ "req": { "collection": b.id, "faces": [3] } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(emptied.face_count, filled.face_count - 1);
    let names: Vec<_> = list().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["A", "Brand"]);
    send(
        &webview,
        "delete_collection",
        serde_json::json!({ "collection": a.id }),
    )
    .unwrap()
    .unwrap();
    assert!(list().is_empty());
}

#[test]
fn tags_favorites_and_ratings_are_edited_and_filter_over_ipc() {
    // One app for every call: the tags live in its MockApi.
    let (_app, webview) = app().unwrap();
    let call = |cmd: &str, args: serde_json::Value| send(&webview, cmd, args).unwrap();
    let tags = || -> Vec<Tag> {
        call("list_tags", serde_json::json!({}))
            .unwrap()
            .deserialize()
            .unwrap()
    };
    let made: Vec<Tag> = call(
        "tag_faces",
        serde_json::json!({ "req": { "faces": [1, 3], "tags": ["Display", "brand"] } }),
    )
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(made.len(), 2);
    let listed: Vec<_> = tags().into_iter().map(|t| (t.name, t.face_count)).collect();
    assert_eq!(listed, [("brand".into(), 2), ("Display".into(), 2)]);
    call(
        "untag_faces",
        serde_json::json!({ "req": { "faces": [1, 3], "tags": [made[1].id] } }),
    )
    .unwrap();
    assert_eq!(tags().len(), 1);
    call(
        "update_user_meta",
        serde_json::json!({ "req": { "faces": [1], "favorite": true, "rating": 4 } }),
    )
    .unwrap();
    let face: FaceDetail = call("get_face", serde_json::json!({ "face": 1 }))
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(
        (face.tags, face.favorite, face.rating),
        (vec!["Display".to_owned()], true, Some(4))
    );
    let found: SearchResponse = call(
        "search",
        serde_json::json!({ "req": { "filters": { "tags": [made[0].id], "favorite": true, "minRating": 4 } } }),
    )
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(found.total, 1);
    let counts = found.counts.unwrap();
    assert_eq!(
        counts.favorite.iter().find(|c| c.value).map(|c| c.count),
        Some(1)
    );
    assert!(counts.min_rating.iter().any(|c| c.value == 4));
    assert!(counts.tags.iter().any(|c| c.value == made[0].id));
    let err = call(
        "update_user_meta",
        serde_json::json!({ "req": { "faces": [1], "rating": 6 } }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "invalidRequest", "{err}");
}

#[test]
fn sample_texts_round_trip_through_fm_api() {
    let req = SampleTextRequest {
        faces: vec![FaceId(1), FaceId(7)],
        text: "Źdźbło".into(),
    };
    let got: Vec<SampleText> = invoke_with(
        "sample_texts",
        serde_json::json!({ "req": { "faces": [1, 7], "text": "Źdźbło" } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(got, MockApi::new().sample_texts(req).unwrap());
    assert!(got[1].fallback.is_some(), "{got:?}");
}

#[test]
fn render_preview_round_trips_through_fm_api() {
    let got: PreviewImage = invoke_with(
        "render_preview",
        serde_json::json!({ "req": { "face": 1, "text": "Aa", "sizePx": 36 } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert!(got.png.starts_with(&[0x89, b'P', b'N', b'G']), "{got:?}");
    assert!(got.width > 0 && got.height > 0, "{got:?}");
    let err = invoke_with(
        "render_preview",
        serde_json::json!({ "req": { "face": 1, "text": "Aa", "sizePx": 400 } }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(err["code"], "invalidRequest", "{err}");
}

#[test]
fn get_face_of_an_unknown_face_is_an_api_error() {
    let err = invoke_with("get_face", serde_json::json!({ "face": 999 }))
        .unwrap()
        .unwrap_err();
    assert_eq!(err["code"], "notFound", "{err}");
}

#[test]
fn activate_and_deactivate_report_one_outcome_per_face() {
    let target = serde_json::json!({ "kind": "faces", "faces": [1, 7] });
    let on: Vec<ActivationOutcome> = invoke_with(
        "activate",
        serde_json::json!({ "req": { "target": target } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(on[0].outcome, OutcomeStatus::Done);
    assert!(matches!(on[1].outcome, OutcomeStatus::Skipped { .. }));

    // A fresh app has activated nothing, so it deactivates nothing.
    let off: Vec<ActivationOutcome> = invoke_with(
        "deactivate",
        serde_json::json!({ "req": { "target": { "kind": "faces", "faces": [1] } } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert!(matches!(off[0].outcome, OutcomeStatus::Skipped { .. }));
}

#[test]
fn list_active_reports_the_faces_the_app_activated() {
    let none: Vec<ActiveFace> = invoke_with("list_active", serde_json::json!({}))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(none, MockApi::new().list_active().unwrap());
}

#[test]
fn conflicts_and_the_conflicts_view_answer_in_camel_case() {
    // Face 6 clashes with a system font in the mock library.
    let json: serde_json::Value = invoke_with(
        "conflicts",
        serde_json::json!({ "target": { "kind": "faces", "faces": [6] } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(json[0]["face"], 6);
    assert_eq!(json[0]["with"]["kind"], "system");
    assert!(json[0]["postscriptName"].is_string());

    let none: serde_json::Value = invoke_with("list_conflicts", serde_json::json!({}))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(none, serde_json::json!([]));
}

#[test]
fn the_problems_view_answers_in_camel_case() {
    let json: serde_json::Value = invoke_with("list_problems", serde_json::json!({}))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    // The mock library: face 6 clashes with a system font, face 7 is Type 1.
    let kinds: Vec<_> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["systemConflict", "unsupported"]);
    assert_eq!(json[0]["face"], 6);
    assert!(json[0]["other"].is_string());
    assert!(json[1]["message"].is_string());
}

#[test]
fn keep_this_one_round_trips_dry_run_keep_history_and_undo() {
    let mock = MockApi::new();
    let copy = "/Users/demo/Downloads/SourceSerif4-Regular.otf";
    mock.add_copy(FaceId(3), copy).unwrap();
    let (_app, webview) = app_on(setup(mock_builder(), Arc::new(mock))).unwrap();
    let call = |cmd: &str, args: serde_json::Value| -> serde_json::Value {
        send(&webview, cmd, args)
            .unwrap()
            .unwrap()
            .deserialize()
            .unwrap()
    };

    let groups = call("list_duplicates", serde_json::json!({}));
    let keep = groups[0]["copies"][0]["path"].as_str().unwrap().to_owned();
    let other = groups[0]["copies"][1]["path"].as_str().unwrap().to_owned();
    assert_eq!(groups[0]["kind"], "exact");
    assert!(groups[0]["copies"][0]["keep"].as_bool().unwrap());

    let req = |dry_run: bool| serde_json::json!({ "req": { "keep": keep, "trash": [other], "dryRun": dry_run } });
    let plan = call("keep_copy", req(true));
    assert_eq!(plan["dryRun"], true);
    assert_eq!(plan["id"], serde_json::Value::Null);
    assert_eq!(plan["moves"][0]["status"], "planned");

    let done = call("keep_copy", req(false));
    assert_eq!(done["moves"][0]["status"], "trashed");
    let id = done["id"].as_u64().unwrap();
    let history = call("list_resolutions", serde_json::json!({}));
    assert_eq!(history[0]["id"], id);

    let undone = call("undo_resolution", serde_json::json!({ "resolution": id }));
    assert_eq!(undone["undone"], true);
    assert_eq!(undone["moves"][0]["status"], "restored");
}

#[test]
fn update_settings_returns_every_setting_in_camel_case() {
    let json: serde_json::Value = invoke_with(
        "update_settings",
        serde_json::json!({ "req": { "theme": "dark", "cacheLimitMb": 256 } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(json["theme"], "dark");
    assert_eq!(json["cacheLimitMb"], 256);
    assert_eq!(json["density"], "comfortable");
}

#[test]
fn an_out_of_range_setting_is_an_api_error() {
    let err = invoke_with(
        "update_settings",
        serde_json::json!({ "req": { "rescanIntervalMinutes": 99999 } }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(err["code"], "invalidRequest", "{err}");
}

/// A settings store that is not the API, to prove the commands use it.
struct Store;

impl SettingsService for Store {
    fn get_settings(&self) -> ApiResult<Settings> {
        Ok(Settings {
            theme: Theme::Light,
            ..Settings::default()
        })
    }
    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        self.get_settings()?.apply(&req)
    }
    fn app_folders(&self) -> ApiResult<AppFolders> {
        Ok(AppFolders {
            config: "/store/config".into(),
            data: "/store/data".into(),
            cache: "/store/cache".into(),
            logs: "/store/logs".into(),
        })
    }
}

#[test]
fn the_settings_store_serves_the_settings_commands() {
    let state = AppState::new(Arc::new(MockApi::new())).with_settings(Arc::new(Store));
    let (_app, webview) = app_on(setup_state(mock_builder(), state)).unwrap();
    let settings: Settings = send(&webview, "get_settings", serde_json::json!({}))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(settings.theme, Theme::Light);
    let folders: AppFolders = send(&webview, "get_app_folders", serde_json::json!({}))
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(folders.logs, "/store/logs");
}

#[test]
fn sources_are_added_paused_resumed_and_removed() {
    let (_app, webview) = app().unwrap();
    let sources = |w| -> Vec<Source> {
        send(w, "list_sources", serde_json::json!({}))
            .unwrap()
            .unwrap()
            .deserialize()
            .unwrap()
    };
    let before = sources(&webview);
    assert!(before.iter().any(|s| s.kind == SourceKind::System));

    let added: Source = send(
        &webview,
        "add_source",
        serde_json::json!({ "req": { "path": "/Users/demo/More", "mode": { "kind": "indexInPlace" } } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(added.kind, SourceKind::Watched);
    assert_eq!(sources(&webview).len(), before.len() + 1);

    for paused in [true, false] {
        let s: Source = send(
            &webview,
            "set_source_paused",
            serde_json::json!({ "req": { "source": added.id, "paused": paused } }),
        )
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
        assert_eq!(s.paused, paused);
    }

    send(
        &webview,
        "remove_source",
        serde_json::json!({ "source": added.id }),
    )
    .unwrap()
    .unwrap();
    assert_eq!(sources(&webview), before);
}

#[test]
fn the_system_source_cannot_be_removed() {
    let (_app, webview) = app().unwrap();
    let err = send(
        &webview,
        "remove_source",
        serde_json::json!({ "source": 2 }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(err["code"], "permissionDenied", "{err}");
}

#[test]
fn scan_progress_is_forwarded_to_the_webview() {
    let (mut app, webview) = app().unwrap();
    // `setup` hooks run on the first turn of the event loop; one turn starts
    // the forwarder as `run` would.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});
    let (tx, rx) = mpsc::channel();
    app.listen(EVENT_CHANNEL, move |e| {
        let _ = tx.send(e.payload().to_owned());
    });
    let job: JobId = send(
        &webview,
        "scan",
        serde_json::json!({ "req": { "source": 1 } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    let next = || -> Event {
        serde_json::from_str(&rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap()
    };
    assert_eq!(
        next(),
        Event::ScanStarted {
            job,
            source: Some(SourceId(1))
        }
    );
    assert!(matches!(next(), Event::ScanProgress { job: j, .. } if j == job));
    assert!(matches!(next(), Event::ScanFinished { job: j, .. } if j == job));
}

#[test]
fn open_help_opens_the_page_in_the_help_window() {
    use tauri::Manager;
    let (app, webview) = app().unwrap();
    send(
        &webview,
        "open_help",
        serde_json::json!({ "page": "sources" }),
    )
    .unwrap()
    .unwrap();
    let help = app
        .get_webview_window(fm_app_lib::help::HELP_WINDOW)
        .expect("the help window is open");
    assert!(
        help.url().unwrap().path().ends_with("/help/sources.html"),
        "{}",
        help.url().unwrap()
    );

    // A second page reuses the window instead of opening another one.
    send(
        &webview,
        "open_help",
        serde_json::json!({ "page": "activation" }),
    )
    .unwrap()
    .unwrap();
    assert_eq!(app.webview_windows().len(), 2);
    assert!(
        help.url()
            .unwrap()
            .path()
            .ends_with("/help/activation.html"),
        "{}",
        help.url().unwrap()
    );
}

#[test]
fn open_help_of_an_unknown_page_is_an_api_error_and_opens_nothing() {
    use tauri::Manager;
    let (app, webview) = app().unwrap();
    let err = send(
        &webview,
        "open_help",
        serde_json::json!({ "page": "../secrets" }),
    )
    .unwrap()
    .unwrap_err();
    assert_eq!(err["code"], "notFound", "{err}");
    assert!(
        app.get_webview_window(fm_app_lib::help::HELP_WINDOW)
            .is_none()
    );
}

// muda builds menus only on the main thread on macOS, and test threads are not
// it; the mock runtime builds them inline on Windows and Linux.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_help_menu_lists_every_docs_page_first_and_opens_it() {
    use fm_app_lib::help::{self, HELP_PAGES, HELP_WINDOW};
    use tauri::Manager;
    use tauri::menu::{HELP_SUBMENU_ID, MenuEvent};

    let (app, _webview) = app_on(help::with_help_menu(setup(
        mock_builder(),
        Arc::new(MockApi::new()),
    )))
    .unwrap();
    let menu = app.menu().expect("the app has a menu");
    let submenu = menu
        .get(HELP_SUBMENU_ID)
        .and_then(|i| i.as_submenu().cloned())
        .expect("the menu has a Help submenu");
    let items = submenu.items().unwrap();
    for (item, page) in items.iter().zip(HELP_PAGES) {
        let item = item.as_menuitem().expect("a page item");
        assert_eq!(item.id().as_ref(), help::menu_id(page));
        assert_eq!(item.text().unwrap(), page.title);
    }
    assert!(
        items.len() > HELP_PAGES.len(),
        "About stays in the Help menu"
    );

    // Choosing an item opens its page; other menu events are ignored.
    help::on_menu_event(
        app.handle(),
        MenuEvent {
            id: "__other__".into(),
        },
    );
    assert!(app.get_webview_window(HELP_WINDOW).is_none());
    help::on_menu_event(
        app.handle(),
        MenuEvent {
            id: help::menu_id(&HELP_PAGES[3]).into(),
        },
    );
    let window = app.get_webview_window(HELP_WINDOW).expect("help window");
    assert!(
        window
            .url()
            .unwrap()
            .path()
            .ends_with("/help/keyboard-shortcuts.html")
    );
}
