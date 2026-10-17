//! Tauri commands round-trip through the real IPC layer to `fm-api`.

use std::error::Error;
use std::sync::Arc;

use fm_api::activation::{ActivationOutcome, OutcomeStatus};
use fm_api::library::{AppInfo, FaceDetail, LibraryService, Source};
use fm_api::mock::MockApi;
use fm_api::settings::{AppFolders, Settings, SettingsUpdate, Theme};
use fm_api::{API_VERSION, ApiResult, FaceId, SettingsService};
use fm_app_lib::{AppState, setup, setup_state};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;

type IpcResponse = Result<InvokeResponseBody, serde_json::Value>;

/// Invokes `cmd` on a mock app; the outer error is a failure to set the app up.
fn invoke(cmd: &str) -> Result<IpcResponse, Box<dyn Error>> {
    invoke_with(cmd, serde_json::json!({}))
}

/// Invokes `cmd` with JSON arguments, as the webview sends them.
fn invoke_with(cmd: &str, args: serde_json::Value) -> Result<IpcResponse, Box<dyn Error>> {
    invoke_on(setup(mock_builder(), Arc::new(MockApi::new())), cmd, args)
}

/// Invokes `cmd` on an app built from this builder.
fn invoke_on(
    builder: tauri::Builder<tauri::test::MockRuntime>,
    cmd: &str,
    args: serde_json::Value,
) -> Result<IpcResponse, Box<dyn Error>> {
    let app = builder.build(mock_context(noop_assets()))?;
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build()?;
    let url = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    Ok(get_ipc_response(
        &webview,
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
    let state = || AppState::new(Arc::new(MockApi::new())).with_settings(Arc::new(Store));
    let settings: Settings = invoke_on(
        setup_state(mock_builder(), state()),
        "get_settings",
        serde_json::json!({}),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(settings.theme, Theme::Light);
    let folders: AppFolders = invoke_on(
        setup_state(mock_builder(), state()),
        "get_app_folders",
        serde_json::json!({}),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(folders.logs, "/store/logs");
}

#[test]
fn sources_can_be_listed_paused_and_removed() {
    let sources: Vec<Source> = invoke("list_sources")
        .unwrap()
        .unwrap()
        .deserialize()
        .unwrap();
    assert_eq!(sources, MockApi::new().list_sources().unwrap());
    let paused: Source = invoke_with(
        "set_source_paused",
        serde_json::json!({ "req": { "source": 1, "paused": true } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert!(paused.paused);
    let added: Source = invoke_with(
        "add_source",
        serde_json::json!({ "req": { "path": "/home/demo/More Fonts", "mode": { "kind": "indexInPlace" } } }),
    )
    .unwrap()
    .unwrap()
    .deserialize()
    .unwrap();
    assert_eq!(added.path, "/home/demo/More Fonts");
    // The system source cannot be removed.
    let err = invoke_with("remove_source", serde_json::json!({ "source": 2 }))
        .unwrap()
        .unwrap_err();
    assert_eq!(err["code"], "permissionDenied", "{err}");
    assert!(
        invoke_with("remove_source", serde_json::json!({ "source": 1 }))
            .unwrap()
            .is_ok()
    );
}
