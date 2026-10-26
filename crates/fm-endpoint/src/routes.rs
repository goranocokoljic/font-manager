//! The route table: one `POST /v1/<method>` per `fm-api` service method.
//!
//! A method that takes a request value reads it from the JSON body; a method
//! that takes none accepts an empty body, `null` or `{}`. The answer is the
//! method's value as JSON (`null` for `()`). A test compares this table with
//! the service traits in `fm-api`, so a new method without a route fails CI.
//! An answer that contains a deprecated field names it
//! ([`Answer::deprecations`], [`fm_api::deprecations_in`]).

use fm_api::{Api, ApiError, ApiResult, Deprecation};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Why a body was refused before any service ran; its detail starts with
/// this, so a caller (and the route test) can tell it from a service error.
pub const BODY_ERROR_DETAIL: &str = "request body: ";

macro_rules! routes {
    ($($name:ident($($arg:ident)?)),* $(,)?) => {
        /// Every route, by method name (the path is `/v1/<name>`).
        pub const ROUTES: &[&str] = &[$(stringify!($name)),*];

        /// Runs `method` on `api` with the request in `body`; `None` when
        /// there is no such route. `deprecated` is the list the answer is
        /// checked against ([`fm_api::deprecations`] when served).
        pub fn dispatch(
            api: &dyn Api,
            method: &str,
            body: &[u8],
            deprecated: &[Deprecation],
        ) -> Option<ApiResult<Answer>> {
            Some(match method {
                $(stringify!($name) => routes!(@call api body deprecated $name $($arg)?),)*
                _ => return None,
            })
        }
    };
    (@call $api:ident $body:ident $list:ident $name:ident) => {
        no_body($body).and_then(|()| $api.$name()).and_then(|v| answer(&v, $list))
    };
    (@call $api:ident $body:ident $list:ident $name:ident $arg:ident) => {
        parse($body).and_then(|$arg| $api.$name($arg)).and_then(|v| answer(&v, $list))
    };
}

routes! {
    // library
    app_info(),
    list_sources(),
    add_source(req),
    remove_source(source),
    set_source_paused(req),
    scan(req),
    rebuild_index(),
    list_families(req),
    get_family(family),
    get_face(face),
    list_problems(),
    list_duplicates(),
    keep_copy(req),
    list_resolutions(),
    undo_resolution(resolution),
    list_sidecar_conflicts(),
    resolve_sidecar_conflict(req),
    list_glyphs(req),
    get_glyph(req),
    resolve_font(req),
    // search
    search(req),
    check_coverage(req),
    face_coverage(face),
    list_languages(),
    find_similar(req),
    suggest_pairings(req),
    // preview
    render_preview(req),
    compare_preview(req),
    font_source(face),
    sample_texts(req),
    css_values(req),
    size_factors(req),
    // activation
    activate(req),
    deactivate(req),
    list_active(),
    conflicts(target),
    list_conflicts(),
    // organization
    list_collections(),
    create_collection(req),
    rename_collection(req),
    move_collection(req),
    update_collection_rules(req),
    delete_collection(collection),
    add_to_collection(req),
    remove_from_collection(req),
    list_tags(),
    tag_faces(req),
    untag_faces(req),
    selection_tags(req),
    update_user_meta(req),
    list_sets(),
    create_set(req),
    delete_set(set),
    export_set(set),
    import_set(export),
    pin_instance(req),
    list_virtual_styles(req),
    delete_virtual_style(style),
    update_family_notes(req),
    declare_license(req),
    quick_collection(),
    add_to_quick_collection(req),
    remove_from_quick_collection(req),
    clear_quick_collection(),
    // export
    export_web(req),
    framework_snippet(req),
    license_inventory(req),
    export_glyph_svg(req),
    export_specimen(req),
    export_collection_specimen(req),
    // workflows
    pick_fonts_for_brief(req),
    prepare_for_web(req),
    // settings
    get_settings(),
    update_settings(req),
    app_folders(),
    endpoint_info(),
    regenerate_endpoint_token(),
    mcp_status(),
    set_mcp_enabled(req),
    // layout
    layout_presets(),
    clamp_layout(layout),
    layout_bounds(),
    list_layouts(),
    get_layout(req),
    save_layout(req),
    duplicate_layout(req),
    revert_layout(req),
    layout_history(req),
    restore_layout(req),
    layout_width(layout),
    // comparisons
    list_comparisons(),
    get_comparison(req),
    save_comparison(req),
    rename_comparison(req),
    delete_comparison(req),
    // catalogs
    list_catalogs(),
    set_catalog_enabled(req),
    refresh_catalog(req),
    list_catalog_families(req),
    catalog_font_source(req),
    download_family(req),
    list_catalog_downloads(),
    check_updates(req),
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> ApiResult<T> {
    serde_json::from_slice(body).map_err(|e| {
        ApiError::invalid("The request is not valid JSON for this method.")
            .with_detail(format!("{BODY_ERROR_DETAIL}{e}"))
    })
}

/// A method without a request takes an empty body, `null` or `{}`.
fn no_body(body: &[u8]) -> ApiResult<()> {
    let text = body.trim_ascii();
    if matches!(text, b"" | b"null") {
        return Ok(());
    }
    match serde_json::from_slice::<serde_json::Value>(text) {
        Ok(serde_json::Value::Object(map)) if map.is_empty() => Ok(()),
        _ => Err(ApiError::invalid("This method takes no request.")
            .with_detail(format!("{BODY_ERROR_DETAIL}expected no body, null or {{}}"))),
    }
}

/// A service's answer: its JSON, and the [`Deprecation::header_value`] of
/// each deprecated field in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub json: Vec<u8>,
    pub deprecations: Vec<String>,
}

fn answer<T: Serialize + specta::Type>(value: &T, deprecated: &[Deprecation]) -> ApiResult<Answer> {
    let json = serde_json::to_vec(value).map_err(not_encoded)?;
    if deprecated.is_empty() {
        return Ok(Answer {
            json,
            deprecations: Vec::new(),
        });
    }
    // A tree only to look for keys: the bytes keep the field order serde wrote.
    let tree = serde_json::to_value(value).map_err(not_encoded)?;
    let deprecations = fm_api::deprecations_in::<T>(deprecated, &tree)
        .into_iter()
        .map(Deprecation::header_value)
        .collect();
    Ok(Answer { json, deprecations })
}

fn not_encoded(e: serde_json::Error) -> ApiError {
    ApiError::new(
        fm_api::ErrorCode::Internal,
        "The answer could not be encoded.",
    )
    .with_detail(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_body_takes_empty_null_and_an_empty_object() {
        for ok in [&b""[..], b"  ", b"null", b"{}", b" { } "] {
            assert!(no_body(ok).is_ok(), "{:?}", String::from_utf8_lossy(ok));
        }
        for bad in [&b"1"[..], b"{\"a\":1}", b"[", b"[]"] {
            let e = no_body(bad).expect_err("refused");
            assert!(
                e.detail
                    .as_deref()
                    .is_some_and(|d| d.starts_with(BODY_ERROR_DETAIL))
            );
        }
    }

    #[test]
    fn route_names_are_unique() {
        let mut names = ROUTES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ROUTES.len());
    }
}
