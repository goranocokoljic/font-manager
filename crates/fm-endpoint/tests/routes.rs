//! Every `fm-api` service method has a route, and every route reaches its
//! service over HTTP (criterion 1 of E25.1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use common::Served;
use fm_endpoint::routes::{BODY_ERROR_DETAIL, ROUTES};

/// The methods of every `pub trait <Name>Service` in `fm-api`'s sources.
/// `EventSource` is not a service (ADR 0006, ADR 0015) and has no route.
fn service_methods() -> BTreeSet<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fm-api/src");
    let mut methods = BTreeSet::new();
    let mut traits = 0;
    for entry in std::fs::read_dir(&src).expect("fm-api sources") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        let mut in_service = false;
        for line in text.lines() {
            if line.starts_with("pub trait ") && line.contains("Service") {
                in_service = true;
                traits += 1;
                continue;
            }
            if in_service && line.starts_with('}') {
                in_service = false;
            }
            if in_service
                && let Some(rest) = line.trim_start().strip_prefix("fn ")
                && let Some(name) = rest.split(['(', '<']).next()
            {
                methods.insert(name.to_owned());
            }
        }
    }
    assert!(
        traits >= 11,
        "found only {traits} service traits in {src:?}"
    );
    methods
}

#[test]
fn every_service_method_has_a_route_and_every_route_a_method() {
    let methods = service_methods();
    let routes: BTreeSet<String> = ROUTES.iter().map(|r| (*r).to_owned()).collect();
    let missing: Vec<_> = methods.difference(&routes).collect();
    assert!(
        missing.is_empty(),
        "service methods without a route: {missing:?}"
    );
    let extra: Vec<_> = routes.difference(&methods).collect();
    assert!(
        extra.is_empty(),
        "routes without a service method: {extra:?}"
    );
}

/// A request body for every route (`""` for a method without one); ids
/// that the mock does not know are fine, as long as the service answers.
fn bodies(served: &Served) -> BTreeMap<&'static str, String> {
    let layout = served.call("layout_presets", "").body[0].clone();
    assert!(layout.is_object(), "a layout preset");
    let export = served.call("export_set", "1").body;
    let out = std::env::temp_dir().join("fm-endpoint-routes");
    let out = serde_json::to_string(&out.to_string_lossy()).expect("path");
    let set_file = std::env::temp_dir().join("fm-endpoint-routes.fmset.json");
    let set_file = serde_json::to_string(&set_file.to_string_lossy()).expect("path");
    let faces = r#"{"kind":"faces","faces":[1]}"#;
    let pairs: Vec<(&'static str, String)> = vec![
        // library
        ("app_info", String::new()),
        ("list_sources", String::new()),
        (
            "add_source",
            r#"{"path":"/fonts/new","mode":{"kind":"indexInPlace"}}"#.into(),
        ),
        ("remove_source", "1".into()),
        ("set_source_paused", r#"{"source":1,"paused":false}"#.into()),
        ("scan", "{}".into()),
        ("rebuild_index", String::new()),
        ("list_families", "{}".into()),
        ("get_family", "1".into()),
        ("get_face", "1".into()),
        ("list_problems", String::new()),
        ("list_duplicates", String::new()),
        (
            "keep_copy",
            r#"{"keep":"/a.ttf","trash":[],"dryRun":true}"#.into(),
        ),
        ("list_resolutions", String::new()),
        ("undo_resolution", "1".into()),
        ("list_sidecar_conflicts", String::new()),
        (
            "resolve_sidecar_conflict",
            r#"{"id":1,"keep":"mine"}"#.into(),
        ),
        ("list_glyphs", r#"{"face":1}"#.into()),
        ("get_glyph", r#"{"face":1,"codePoint":65}"#.into()),
        ("resolve_font", r#"{"font":"1"}"#.into()),
        // search
        ("search", r#"{"query":"sans"}"#.into()),
        (
            "check_coverage",
            r#"{"face":1,"target":{"kind":"text","text":"Ab"}}"#.into(),
        ),
        ("face_coverage", "1".into()),
        ("list_languages", String::new()),
        ("find_similar", r#"{"face":1,"limit":3}"#.into()),
        ("suggest_pairings", r#"{"face":1,"limit":3}"#.into()),
        // preview
        (
            "render_preview",
            r#"{"face":1,"text":"Ag","sizePx":24}"#.into(),
        ),
        (
            "compare_preview",
            r#"{"faces":[1,3],"text":"Ag","sizePx":24}"#.into(),
        ),
        ("font_source", "1".into()),
        ("sample_texts", r#"{"faces":[1],"text":"Ag"}"#.into()),
        ("css_values", r#"{"face":1}"#.into()),
        ("size_factors", r#"{"faces":[1],"basis":"xHeight"}"#.into()),
        // activation
        ("activate", format!(r#"{{"target":{faces}}}"#)),
        ("deactivate", format!(r#"{{"target":{faces}}}"#)),
        ("list_active", String::new()),
        ("conflicts", faces.into()),
        ("list_conflicts", String::new()),
        // organization
        ("list_collections", String::new()),
        ("create_collection", r#"{"name":"Over HTTP"}"#.into()),
        (
            "rename_collection",
            r#"{"collection":1,"name":"Renamed"}"#.into(),
        ),
        ("move_collection", r#"{"collection":1,"position":0}"#.into()),
        (
            "update_collection_rules",
            r#"{"collection":1,"ruleGroup":{"op":"and","rules":[]}}"#.into(),
        ),
        ("delete_collection", "999".into()),
        (
            "add_to_collection",
            r#"{"collection":1,"faces":[1]}"#.into(),
        ),
        (
            "remove_from_collection",
            r#"{"collection":1,"faces":[1]}"#.into(),
        ),
        ("list_tags", String::new()),
        ("tag_faces", r#"{"faces":[1],"tags":["http"]}"#.into()),
        ("untag_faces", r#"{"faces":[1],"tags":[1]}"#.into()),
        ("selection_tags", r#"{"faces":[1]}"#.into()),
        (
            "update_user_meta",
            r#"{"faces":[1],"favorite":true}"#.into(),
        ),
        ("list_sets", String::new()),
        ("create_set", r#"{"name":"Set","faces":[1]}"#.into()),
        ("delete_set", "999".into()),
        ("export_set", "1".into()),
        ("import_set", export.to_string()),
        ("save_set_file", format!(r#"{{"set":1,"path":{set_file}}}"#)),
        ("load_set_file", format!(r#"{{"path":{set_file}}}"#)),
        ("pin_instance", r#"{"face":1,"name":"Pinned"}"#.into()),
        ("list_virtual_styles", "{}".into()),
        ("delete_virtual_style", "999".into()),
        (
            "update_family_notes",
            r#"{"family":1,"notes":"via HTTP"}"#.into(),
        ),
        ("declare_license", r#"{"family":1}"#.into()),
        ("quick_collection", String::new()),
        ("add_to_quick_collection", r#"{"faces":[1]}"#.into()),
        ("remove_from_quick_collection", r#"{"faces":[1]}"#.into()),
        ("clear_quick_collection", String::new()),
        // export
        (
            "export_web",
            format!(r#"{{"faces":[1],"formats":["woff2"],"outDir":{out}}}"#),
        ),
        (
            "framework_snippet",
            r#"{"faces":[1],"framework":"tailwind"}"#.into(),
        ),
        ("license_inventory", "{}".into()),
        ("export_glyph_svg", r#"{"face":1,"codePoint":65}"#.into()),
        (
            "export_specimen",
            format!(r#"{{"family":1,"format":"html","outPath":{out}}}"#),
        ),
        (
            "export_collection_specimen",
            format!(r#"{{"collection":1,"format":"html","outPath":{out}}}"#),
        ),
        // workflows
        (
            "pick_fonts_for_brief",
            r#"{"brief":"a friendly sans"}"#.into(),
        ),
        (
            "prepare_for_web",
            format!(r#"{{"family":1,"outDir":{out}}}"#),
        ),
        // settings
        ("get_settings", String::new()),
        ("update_settings", r#"{"theme":"dark"}"#.into()),
        ("app_folders", String::new()),
        ("endpoint_info", String::new()),
        ("regenerate_endpoint_token", String::new()),
        ("mcp_status", String::new()),
        ("set_mcp_enabled", r#"{"enabled":true}"#.into()),
        // layout
        ("layout_presets", String::new()),
        ("clamp_layout", layout.to_string()),
        ("layout_bounds", String::new()),
        ("list_layouts", String::new()),
        ("get_layout", r#"{"id":"none"}"#.into()),
        (
            "save_layout",
            serde_json::json!({ "id": "via-http", "layout": layout }).to_string(),
        ),
        ("duplicate_layout", r#"{"id":"via-http"}"#.into()),
        ("revert_layout", r#"{"id":"via-http"}"#.into()),
        ("layout_history", r#"{"id":"via-http"}"#.into()),
        ("restore_layout", r#"{"id":"via-http","number":1}"#.into()),
        ("layout_width", layout.to_string()),
        // comparisons
        ("list_comparisons", String::new()),
        ("get_comparison", r#"{"id":1}"#.into()),
        ("save_comparison", r#"{"name":"Pair","faces":[1]}"#.into()),
        ("rename_comparison", r#"{"id":1,"name":"Renamed"}"#.into()),
        ("delete_comparison", r#"{"id":999}"#.into()),
        // catalogs
        ("list_catalogs", String::new()),
        (
            "set_catalog_enabled",
            r#"{"id":"googleFonts","enabled":true}"#.into(),
        ),
        ("refresh_catalog", r#"{"id":"googleFonts"}"#.into()),
        ("list_catalog_families", r#"{"id":"googleFonts"}"#.into()),
        (
            "catalog_font_source",
            r#"{"id":"googleFonts","family":"Inter"}"#.into(),
        ),
        (
            "download_family",
            r#"{"id":"googleFonts","family":"Inter","kind":"static","source":1}"#.into(),
        ),
        ("list_catalog_downloads", String::new()),
        ("check_updates", r#"{"cachedOnly":true}"#.into()),
    ];
    pairs.into_iter().collect()
}

#[test]
fn every_route_reaches_its_service_over_http() {
    let served = Served::new();
    let bodies = bodies(&served);
    let names: BTreeSet<&str> = bodies.keys().copied().collect();
    let routes: BTreeSet<&str> = ROUTES.iter().copied().collect();
    assert_eq!(names, routes, "the body table must name every route once");
    // `call` reads the token anew each time, so the regeneration is harmless.
    for route in ROUTES {
        let reply = served.call(route, &bodies[route]);
        let refused_body = reply.body["detail"]
            .as_str()
            .is_some_and(|d| d.starts_with(BODY_ERROR_DETAIL));
        assert!(
            !refused_body,
            "{route}: the body was refused before the service ran: {}",
            reply.body
        );
        // 200 with the value, or the service's own error as JSON.
        if reply.status != 200 {
            assert!(
                reply.body["code"].is_string() && reply.body["message"].is_string(),
                "{route}: {} {}",
                reply.status,
                reply.body
            );
            assert!(
                ![401, 405, 413].contains(&reply.status),
                "{route}: refused by the transport: {} {}",
                reply.status,
                reply.body
            );
        }
        assert!(
            reply
                .headers
                .to_ascii_lowercase()
                .contains("content-type: application/json")
        );
    }
}

#[test]
fn a_service_value_comes_back_as_its_json() {
    let served = Served::new();
    let reply = served.call("list_sources", "");
    assert_eq!(reply.status, 200);
    let direct = serde_json::to_value(
        fm_api::LibraryService::list_sources(served.api.as_ref()).expect("sources"),
    )
    .expect("json");
    assert_eq!(reply.body, direct);
    // A unit answer is `null`; a request in the body reaches the service.
    let created = served.call("create_collection", r#"{"name":"From HTTP"}"#);
    assert_eq!(created.status, 200, "{}", created.body);
    assert_eq!(created.body["name"], "From HTTP");
    let id = created.body["id"].to_string();
    let deleted = served.call("delete_collection", &id);
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(deleted.body, serde_json::Value::Null);
}

#[test]
fn a_service_error_keeps_its_shape_and_gets_its_status() {
    let served = Served::new();
    let missing = served.call("get_face", "999999");
    assert_eq!(missing.status, 404, "{}", missing.body);
    assert_eq!(missing.body["code"], "notFound");
    assert!(missing.body["message"].is_string());
    let invalid = served.call("update_settings", r#"{"cacheLimitMb":1}"#);
    assert_eq!(invalid.status, 400, "{}", invalid.body);
    assert_eq!(invalid.body["code"], "invalidRequest");
}

#[test]
fn malformed_bodies_unknown_routes_and_other_methods_are_refused() {
    let served = Served::new();
    let bad = served.call("get_face", "{not json");
    assert_eq!(bad.status, 400);
    assert!(
        bad.body["detail"]
            .as_str()
            .is_some_and(|d| d.starts_with(BODY_ERROR_DETAIL))
    );
    let wrong_type = served.call("get_face", r#""one""#);
    assert_eq!(wrong_type.status, 400);
    let no_route = served.call("drop_everything", "");
    assert_eq!(no_route.status, 404);
    assert_eq!(no_route.body["code"], "notFound");
    let auth = format!("Bearer {}", served.token());
    let get = common::send(
        served.endpoint.addr(),
        "GET",
        "/v1/list_sources",
        &[("Host", &served.host()), ("Authorization", &auth)],
        "",
    );
    assert_eq!(get.status, 405);
    assert!(get.headers.to_ascii_lowercase().contains("allow: post"));
    let outside = common::send(
        served.endpoint.addr(),
        "POST",
        "/list_sources",
        &[("Host", &served.host()), ("Authorization", &auth)],
        "",
    );
    assert_eq!(outside.status, 404);
}

#[test]
fn a_body_over_the_limit_is_refused() {
    let served = Served::new();
    let auth = format!("Bearer {}", served.token());
    let len = (fm_endpoint::MAX_BODY_BYTES + 1).to_string();
    // Only the header: the endpoint refuses on the announced length.
    let reply = common::send(
        served.endpoint.addr(),
        "POST",
        "/v1/search",
        &[
            ("Host", &served.host()),
            ("Authorization", &auth),
            ("Content-Length", &len),
        ],
        "",
    );
    assert_eq!(reply.status, 413);
}
