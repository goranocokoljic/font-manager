//! The same calls against `MockApi` directly and against `MockApi` behind
//! the endpoint through `fm-client` give equal results and equal errors
//! (E25.4 criterion 3). Every route is called; the two mocks start equal
//! and get the same calls in the same order, so their state stays equal.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use common::Served;
use fm_api::mock::MockApi;
use fm_api::*;
use fm_endpoint::routes::ROUTES;
use serde_json::json;

/// Calls `$m` on both and compares; a request comes from JSON, typed by
/// the method itself.
macro_rules! same {
    ($seen:ident, $d:ident, $c:ident; $m:ident) => {{
        $seen.push(stringify!($m));
        let direct = $d.$m();
        let through = $c.$m();
        assert_eq!(through, direct, "{}", stringify!($m));
    }};
    ($seen:ident, $d:ident, $c:ident; $m:ident, $json:expr) => {{
        $seen.push(stringify!($m));
        let req = serde_json::from_value($json)
            .unwrap_or_else(|e| panic!("{} request: {e}", stringify!($m)));
        let direct = $d.$m(Clone::clone(&req));
        let through = $c.$m(req);
        assert_eq!(through, direct, "{}", stringify!($m));
    }};
}

#[test]
fn every_route_gives_the_same_result_and_error_as_the_mock() {
    let d = MockApi::new();
    let served = Served::new();
    let c = &served.client;
    let mut seen: Vec<&str> = Vec::new();
    let out = std::env::temp_dir().join("fm-client-conformance");
    let out = out.to_string_lossy().into_owned();
    let set_file = std::env::temp_dir().join("fm-client-conformance-set");
    let set_file = set_file.to_string_lossy().into_owned();
    let faces = json!({ "kind": "faces", "faces": [1] });
    let layout = serde_json::to_value(&d.layout_presets().unwrap()[0]).unwrap();
    let export = serde_json::to_value(d.export_set(SetId(1)).unwrap()).unwrap();

    // library
    same!(seen, d, c; app_info);
    same!(seen, d, c; list_sources);
    same!(seen, d, c; add_source, json!({ "path": "/fonts/new", "mode": { "kind": "indexInPlace" } }));
    same!(seen, d, c; remove_source, json!(1));
    same!(seen, d, c; set_source_paused, json!({ "source": 1, "paused": false }));
    same!(seen, d, c; scan, json!({}));
    same!(seen, d, c; rebuild_index);
    same!(seen, d, c; list_families, json!({}));
    same!(seen, d, c; get_family, json!(1));
    same!(seen, d, c; get_face, json!(1));
    same!(seen, d, c; get_face, json!(999_999));
    same!(seen, d, c; list_problems);
    same!(seen, d, c; list_duplicates);
    same!(seen, d, c; keep_copy, json!({ "keep": "/a.ttf", "trash": [], "dryRun": true }));
    same!(seen, d, c; list_resolutions);
    same!(seen, d, c; undo_resolution, json!(1));
    same!(seen, d, c; list_sidecar_conflicts);
    same!(seen, d, c; resolve_sidecar_conflict, json!({ "id": 1, "keep": "mine" }));
    same!(seen, d, c; list_glyphs, json!({ "face": 1 }));
    same!(seen, d, c; get_glyph, json!({ "face": 1, "codePoint": 65 }));
    same!(seen, d, c; resolve_font, json!({ "font": "1" }));
    // search
    same!(seen, d, c; search, json!({ "query": "sans" }));
    same!(seen, d, c; check_coverage, json!({ "face": 1, "target": { "kind": "text", "text": "Ab" } }));
    same!(seen, d, c; face_coverage, json!(1));
    same!(seen, d, c; list_languages);
    same!(seen, d, c; find_similar, json!({ "face": 1, "limit": 3 }));
    same!(seen, d, c; suggest_pairings, json!({ "face": 1, "limit": 3 }));
    // preview
    same!(seen, d, c; render_preview, json!({ "face": 1, "text": "Ag", "sizePx": 24 }));
    same!(seen, d, c; compare_preview, json!({ "faces": [1, 3], "text": "Ag", "sizePx": 24 }));
    same!(seen, d, c; compare_preview, json!({ "faces": [1], "text": "Ag", "sizePx": 24 }));
    same!(seen, d, c; font_source, json!(1));
    same!(seen, d, c; sample_texts, json!({ "faces": [1], "text": "Ag" }));
    same!(seen, d, c; css_values, json!({ "face": 1 }));
    same!(seen, d, c; size_factors, json!({ "faces": [1], "basis": "xHeight" }));
    // activation
    same!(seen, d, c; activate, json!({ "target": faces }));
    same!(seen, d, c; list_active);
    same!(seen, d, c; conflicts, faces.clone());
    same!(seen, d, c; list_conflicts);
    same!(seen, d, c; fonts_needed, json!({
        "fonts": [{ "postscriptName": "Inter-Italic" }, { "fullName": "Nope" }],
        "document": "poster.indd",
    }));
    same!(seen, d, c; document_closed, json!({ "document": "poster.indd" }));
    same!(seen, d, c; deactivate, json!({ "target": faces }));
    // organization
    same!(seen, d, c; list_collections);
    same!(seen, d, c; create_collection, json!({ "name": "Over HTTP" }));
    same!(seen, d, c; create_collection, json!({ "name": "Over HTTP" }));
    same!(seen, d, c; rename_collection, json!({ "collection": 1, "name": "Renamed" }));
    same!(seen, d, c; move_collection, json!({ "collection": 1, "position": 0 }));
    same!(seen, d, c; update_collection_rules, json!({ "collection": 1, "ruleGroup": { "op": "and", "rules": [] } }));
    same!(seen, d, c; delete_collection, json!(999));
    same!(seen, d, c; add_to_collection, json!({ "collection": 1, "faces": [1] }));
    same!(seen, d, c; remove_from_collection, json!({ "collection": 1, "faces": [1] }));
    same!(seen, d, c; list_tags);
    same!(seen, d, c; tag_faces, json!({ "faces": [1], "tags": ["http"] }));
    same!(seen, d, c; untag_faces, json!({ "faces": [1], "tags": [1] }));
    same!(seen, d, c; selection_tags, json!({ "faces": [1] }));
    same!(seen, d, c; update_user_meta, json!({ "faces": [1], "favorite": true }));
    same!(seen, d, c; list_sets);
    same!(seen, d, c; create_set, json!({ "name": "Set", "faces": [1] }));
    same!(seen, d, c; delete_set, json!(999));
    same!(seen, d, c; export_set, json!(1));
    same!(seen, d, c; import_set, export);
    same!(seen, d, c; save_set_file, json!({ "set": 1, "path": set_file }));
    same!(seen, d, c; load_set_file, json!({ "path": format!("{set_file}.fmset.json") }));
    same!(seen, d, c; pin_instance, json!({ "face": 1, "name": "Pinned" }));
    same!(seen, d, c; list_virtual_styles, json!({}));
    same!(seen, d, c; delete_virtual_style, json!(999));
    same!(seen, d, c; update_family_notes, json!({ "family": 1, "notes": "via HTTP" }));
    same!(seen, d, c; declare_license, json!({ "family": 1 }));
    same!(seen, d, c; quick_collection);
    same!(seen, d, c; add_to_quick_collection, json!({ "faces": [1] }));
    same!(seen, d, c; remove_from_quick_collection, json!({ "faces": [1] }));
    same!(seen, d, c; clear_quick_collection);
    // export
    same!(seen, d, c; export_web, json!({ "faces": [1], "formats": ["woff2"], "outDir": out }));
    same!(seen, d, c; framework_snippet, json!({ "faces": [1], "framework": "tailwind" }));
    same!(seen, d, c; license_inventory, json!({}));
    same!(seen, d, c; export_glyph_svg, json!({ "face": 1, "codePoint": 65 }));
    same!(seen, d, c; export_specimen, json!({ "family": 1, "format": "html", "outPath": out }));
    same!(seen, d, c; export_collection_specimen, json!({ "collection": 1, "format": "html", "outPath": out }));
    // workflows
    same!(seen, d, c; pick_fonts_for_brief, json!({ "brief": "a friendly sans" }));
    same!(seen, d, c; prepare_for_web, json!({ "family": 1, "outDir": out }));
    // settings
    same!(seen, d, c; get_settings);
    same!(seen, d, c; update_settings, json!({ "theme": "dark" }));
    same!(seen, d, c; update_settings, json!({ "cacheLimitMb": 1 }));
    same!(seen, d, c; app_folders);
    same!(seen, d, c; endpoint_info);
    // The client takes the new token, so the calls after this still get in.
    same!(seen, d, c; regenerate_endpoint_token);
    same!(seen, d, c; endpoint_info);
    same!(seen, d, c; mcp_status);
    same!(seen, d, c; set_mcp_enabled, json!({ "enabled": true }));
    same!(seen, d, c; mcp_status);
    // layout
    same!(seen, d, c; layout_presets);
    same!(seen, d, c; clamp_layout, layout.clone());
    same!(seen, d, c; layout_bounds);
    same!(seen, d, c; list_layouts);
    same!(seen, d, c; get_layout, json!({ "id": "none" }));
    same!(seen, d, c; save_layout, json!({ "id": "via-http", "layout": layout }));
    same!(seen, d, c; duplicate_layout, json!({ "id": "via-http" }));
    same!(seen, d, c; revert_layout, json!({ "id": "via-http" }));
    same!(seen, d, c; layout_history, json!({ "id": "via-http" }));
    same!(seen, d, c; restore_layout, json!({ "id": "via-http", "number": 1 }));
    same!(seen, d, c; layout_width, layout);
    // comparisons
    same!(seen, d, c; list_comparisons);
    same!(seen, d, c; save_comparison, json!({ "name": "Pair", "faces": [1] }));
    same!(seen, d, c; get_comparison, json!({ "id": 1 }));
    same!(seen, d, c; rename_comparison, json!({ "id": 1, "name": "Renamed" }));
    same!(seen, d, c; delete_comparison, json!({ "id": 999 }));
    // catalogs
    same!(seen, d, c; list_catalogs);
    same!(seen, d, c; set_catalog_enabled, json!({ "id": "googleFonts", "enabled": true }));
    same!(seen, d, c; refresh_catalog, json!({ "id": "googleFonts" }));
    same!(seen, d, c; list_catalog_families, json!({ "id": "googleFonts" }));
    same!(seen, d, c; catalog_font_source, json!({ "id": "googleFonts", "family": "Inter" }));
    same!(seen, d, c; download_family, json!({ "id": "googleFonts", "family": "Inter", "kind": "static", "source": 1 }));
    same!(seen, d, c; list_catalog_downloads);
    same!(seen, d, c; check_updates, json!({ "cachedOnly": true }));

    let called: BTreeSet<&str> = seen.into_iter().collect();
    let routes: BTreeSet<&str> = ROUTES.iter().copied().collect();
    assert_eq!(called, routes, "the conformance test must call every route");
}

#[test]
fn errors_come_back_equal_with_code_message_and_detail() {
    let d = MockApi::new();
    let served = Served::new();
    let c = &served.client;
    let direct = d.get_face(FaceId(999_999)).unwrap_err();
    assert_eq!(direct.code, ErrorCode::NotFound);
    assert_eq!(c.get_face(FaceId(999_999)).unwrap_err(), direct);
    let bad = settings::SettingsUpdate {
        cache_limit_mb: Some(1),
        ..settings::SettingsUpdate::default()
    };
    let direct = d.update_settings(bad).unwrap_err();
    assert_eq!(direct.code, ErrorCode::InvalidRequest);
    assert_eq!(c.update_settings(bad).unwrap_err(), direct);
}

fn take(rx: &Receiver<Event>, n: usize) -> Vec<Event> {
    (0..n)
        .map(|_| rx.recv_timeout(Duration::from_secs(10)).expect("event"))
        .collect()
}

#[test]
fn a_scan_streams_the_same_events_through_the_client() {
    let d = MockApi::new();
    let served = Served::new();
    let c = &served.client;
    let direct_events = d.subscribe();
    let client_events = c.subscribe();
    let direct_job = d.scan(library::ScanRequest::default()).unwrap();
    let client_job = c.scan(library::ScanRequest::default()).unwrap();
    assert_eq!(client_job, direct_job);
    let direct = take(&direct_events, 3);
    assert!(matches!(direct[0], Event::ScanStarted { .. }), "{direct:?}");
    assert_eq!(take(&client_events, 3), direct);
}

#[test]
fn a_dropped_client_subscription_ends_its_stream() {
    let served = Served::new();
    let events = served.client.subscribe();
    served.wait_for_streams(1, || {});
    drop(events);
    // The next frame finds the receiver gone; a scan sends three.
    served.wait_for_streams(0, || {
        served.client.scan(library::ScanRequest::default()).unwrap();
    });
}

#[test]
fn a_stream_that_ends_disconnects_the_receiver() {
    let served = Served::new();
    let events = served.client.subscribe();
    served.wait_for_streams(1, || {});
    served.endpoint.stop();
    assert_eq!(
        events.recv_timeout(Duration::from_secs(10)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    );
}
