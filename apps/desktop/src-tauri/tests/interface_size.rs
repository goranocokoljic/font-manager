//! E58.7: the app zooms its webviews to the interface size of the settings,
//! from the start and at once on a change (ADR 0029). Built through
//! `setup`/`setup_state`, the same wiring `run()` uses.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use fm_api::SettingsService;
use fm_api::mock::MockApi;
use fm_api::settings::{InterfaceSize, SettingsUpdate};
use fm_app_lib::interface_size::InterfaceZoom;
use fm_app_lib::{AppState, setup, setup_state};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};

#[test]
fn the_zoom_at_start_is_the_size_in_the_settings() {
    let app = setup(mock_builder(), Arc::new(MockApi::new()))
        .build(mock_context(noop_assets()))
        .unwrap();
    assert_eq!(app.state::<InterfaceZoom>().factor(), 1.0);

    let api = MockApi::new();
    api.update_settings(SettingsUpdate {
        interface_size: Some(InterfaceSize::ExtraLarge),
        ..Default::default()
    })
    .unwrap();
    let app = setup_state(mock_builder(), AppState::new(Arc::new(api)))
        .build(mock_context(noop_assets()))
        .unwrap();
    assert_eq!(app.state::<InterfaceZoom>().factor(), 1.5);
}

#[test]
fn a_change_of_size_zooms_at_once_without_a_restart() {
    let mut app = setup(mock_builder(), Arc::new(MockApi::new()))
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    // `setup` hooks run on the first turn of the event loop; one turn starts
    // the event forwarder as `run` would.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});
    let url = if cfg!(windows) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "update_settings".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(serde_json::json!({ "req": { "interfaceSize": "large" } })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("update_settings answered");

    // The event reaches the zoom on the app's event thread.
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.state::<InterfaceZoom>().factor() != 1.25 {
        assert!(
            Instant::now() < deadline,
            "the zoom did not follow the setting"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
