//! Tauri commands round-trip through the real IPC layer to `fm-api`.

use std::error::Error;
use std::sync::Arc;

use fm_api::API_VERSION;
use fm_api::library::{AppInfo, LibraryService};
use fm_api::mock::MockApi;
use fm_app_lib::setup;
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;

type IpcResponse = Result<InvokeResponseBody, serde_json::Value>;

/// Invokes `cmd` on a mock app; the outer error is a failure to set the app up.
fn invoke(cmd: &str) -> Result<IpcResponse, Box<dyn Error>> {
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
            body: InvokeBody::default(),
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
