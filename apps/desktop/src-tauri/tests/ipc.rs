//! Tauri commands round-trip through the real IPC layer to `fm-api`.

use std::error::Error;
use std::sync::Arc;

use fm_api::activation::{ActivationOutcome, OutcomeStatus};
use fm_api::library::{AppInfo, FaceDetail, LibraryService};
use fm_api::mock::MockApi;
use fm_api::{API_VERSION, FaceId};
use fm_app_lib::setup;
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
    let app = setup(mock_builder(), Arc::new(MockApi::new())).build(mock_context(noop_assets()))?;
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
