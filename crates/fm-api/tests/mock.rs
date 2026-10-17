//! The mock honors the contract: every service reachable through `dyn Api`,
//! per-face outcomes, no deactivation of fonts the app did not register,
//! the license guard, and events instead of polling.

use std::sync::Arc;

use fm_api::activation::{
    ActivateRequest, ActivationMode, ConflictPolicy, ConflictWith, DeactivateRequest,
    OutcomeStatus, Target,
};
use fm_api::common::Page;
use fm_api::export::{ExportWebRequest, WebFormat};
use fm_api::library::{
    AddSourceRequest, FamilySort, ImportMode, ListFamiliesRequest, ScanRequest,
    SetSourcePausedRequest,
};
use fm_api::mock::MockApi;
use fm_api::organization::{
    CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest, TagFacesRequest,
    UserMetaUpdate,
};
use fm_api::preview::PreviewRequest;
use fm_api::search::{CoverageRequest, Filters, SearchRequest, Supports};
use fm_api::workflows::{BriefRequest, PrepareForWebRequest};
use fm_api::{API_VERSION, Api, ErrorCode, Event, FaceId, FamilyId, SourceId};

fn api() -> Arc<dyn Api> {
    Arc::new(MockApi::new())
}

fn acts(faces: &[u32], mode: ActivationMode, on_conflict: ConflictPolicy) -> ActivateRequest {
    ActivateRequest {
        target: Target::Faces {
            faces: faces.iter().copied().map(FaceId).collect(),
        },
        mode,
        on_conflict,
    }
}

#[test]
fn app_info_carries_the_api_version() {
    assert_eq!(api().app_info().unwrap().api_version, API_VERSION);
}

#[test]
fn families_page_and_sort() {
    let api = api();
    let all = api.list_families(ListFamiliesRequest::default()).unwrap();
    assert_eq!(all.total, 4);
    assert_eq!(all.items[0].name, "Inter");
    let page = api
        .list_families(ListFamiliesRequest {
            sort: FamilySort::StyleCount,
            page: Page {
                offset: 0,
                limit: 1,
            },
            ..Default::default()
        })
        .unwrap();
    assert_eq!((page.items.len(), page.total), (1, 4));
    assert_eq!(page.items[0].name, "Source Serif 4");

    let fam = api.get_family(FamilyId(2)).unwrap();
    let styles: Vec<_> = fam.faces.iter().map(|f| f.style.as_str()).collect();
    assert_eq!(styles, ["Regular", "Italic", "Bold"]); // weight, then slant
}

#[test]
fn paused_sources_hide_their_fonts() {
    let api = api();
    api.set_source_paused(SetSourcePausedRequest {
        source: SourceId(1),
        paused: true,
    })
    .unwrap();
    assert_eq!(
        api.list_families(ListFamiliesRequest::default())
            .unwrap()
            .total,
        0
    );
}

#[test]
fn sources_reject_bad_input_with_structured_errors() {
    let api = api();
    let empty = api
        .add_source(AddSourceRequest {
            path: " ".into(),
            mode: ImportMode::IndexInPlace,
        })
        .unwrap_err();
    assert_eq!(empty.code, ErrorCode::InvalidRequest);
    let dup = api
        .add_source(AddSourceRequest {
            path: "/Users/demo/Fonts".into(),
            mode: ImportMode::IndexInPlace,
        })
        .unwrap_err();
    assert_eq!(dup.code, ErrorCode::Conflict);
    assert_eq!(
        api.remove_source(SourceId(2)).unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        api.get_face(FaceId(999)).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn activation_reports_per_face() {
    let api = api();
    // 1 Inter: done; 6 JetBrains Mono: clashes with a system font; 7 Type 1: skipped.
    let out = api
        .activate(acts(
            &[1, 6, 7],
            ActivationMode::Session,
            ConflictPolicy::KeepExisting,
        ))
        .unwrap();
    assert_eq!(out[0].outcome, OutcomeStatus::Done);
    assert!(matches!(
        &out[1].outcome,
        OutcomeStatus::Conflict {
            with: ConflictWith::System { .. }
        }
    ));
    assert!(matches!(out[2].outcome, OutcomeStatus::Skipped { .. }));

    let again = api
        .activate(acts(
            &[1, 6],
            ActivationMode::Session,
            ConflictPolicy::Replace,
        ))
        .unwrap();
    assert_eq!(again[0].outcome, OutcomeStatus::Unchanged);
    assert_eq!(again[1].outcome, OutcomeStatus::Done);
    let active: Vec<_> = api.list_active().unwrap().iter().map(|a| a.face).collect();
    assert_eq!(active, [FaceId(1), FaceId(6)]);
    assert_eq!(
        api.get_face(FaceId(1)).unwrap().face.active,
        Some(ActivationMode::Session)
    );
}

#[test]
fn deactivate_never_touches_fonts_the_app_did_not_register() {
    let api = api();
    api.activate(acts(
        &[3],
        ActivationMode::Permanent,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();
    let out = api
        .deactivate(DeactivateRequest {
            target: Some(Target::Family {
                family: FamilyId(2),
            }),
        })
        .unwrap();
    let by_face: Vec<_> = out
        .iter()
        .map(|o| (o.face.0, matches!(o.outcome, OutcomeStatus::Done)))
        .collect();
    assert_eq!(by_face, [(3, true), (4, false), (5, false)]);
    assert!(api.list_active().unwrap().is_empty());
}

#[test]
fn events_are_pushed_to_every_subscriber() {
    let api = api();
    let a = api.subscribe();
    let b = api.subscribe();
    let job = api.scan(ScanRequest::default()).unwrap();
    api.activate(acts(
        &[1],
        ActivationMode::Session,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();
    let got: Vec<Event> = a.try_iter().collect();
    assert!(matches!(got[0], Event::ScanProgress { job: j, .. } if j == job));
    assert!(matches!(got[1], Event::ScanFinished { problems: 1, .. }));
    assert_eq!(
        got[2],
        Event::ActivationChanged {
            activated: vec![FaceId(1)],
            deactivated: vec![]
        }
    );
    assert_eq!(b.try_iter().count(), 3);
    drop(b); // a dropped subscriber does not break the next send
    api.deactivate(DeactivateRequest::default()).unwrap();
    assert_eq!(a.try_iter().count(), 1);
}

#[test]
fn search_groups_by_family_and_reports_near_misses() {
    let api = api();
    let r = api
        .search(SearchRequest {
            query: "serif italic".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.families[0].faces, [FaceId(4)]);

    // Serbian Cyrillic: Inter and JetBrains Mono cover it; Source Serif misses far more than 5.
    let filters = Filters {
        supports: Some(Supports::Languages {
            codes: vec!["sr-Cyrl".into()],
        }),
        ..Default::default()
    };
    let r = api
        .search(SearchRequest {
            filters,
            ..Default::default()
        })
        .unwrap();
    let names: Vec<_> = r.families.iter().map(|h| h.family.name.as_str()).collect();
    assert_eq!(names, ["Inter", "JetBrains Mono"]);

    // A pasted text one character short is a near-miss with the exact character.
    let filters = Filters {
        supports: Some(Supports::Text {
            text: "Zdravo ж".into(),
        }),
        ..Default::default()
    };
    let r = api
        .search(SearchRequest {
            query: "source".into(),
            filters,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.families[0].missing.as_ref().unwrap().chars, ["ж"]);

    let report = api
        .check_coverage(CoverageRequest {
            face: FaceId(3),
            target: Supports::Languages {
                codes: vec!["pl".into()],
            },
        })
        .unwrap();
    assert!(report.complete);
    let bad = api.check_coverage(CoverageRequest {
        face: FaceId(3),
        target: Supports::Languages {
            codes: vec!["xx".into()],
        },
    });
    assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidRequest);
}

#[test]
fn collections_tags_meta_and_sets() {
    let api = api();
    let manual = api
        .create_collection(CreateCollectionRequest {
            name: "Clients".into(),
            parent: None,
            rules: None,
        })
        .unwrap();
    let c = api
        .add_to_collection(CollectionFacesRequest {
            collection: manual.id,
            faces: vec![FaceId(1), FaceId(3)],
        })
        .unwrap();
    assert_eq!(c.face_count, 2);

    let smart = api
        .create_collection(CreateCollectionRequest {
            name: "Variable".into(),
            parent: Some(manual.id),
            rules: Some(Filters {
                variable: Some(true),
                ..Default::default()
            }),
        })
        .unwrap();
    assert_eq!(smart.face_count, 2);
    let refused = api.add_to_collection(CollectionFacesRequest {
        collection: smart.id,
        faces: vec![FaceId(3)],
    });
    assert_eq!(refused.unwrap_err().code, ErrorCode::InvalidRequest);
    api.delete_collection(manual.id).unwrap();
    assert!(api.list_collections().unwrap().is_empty()); // the child went with it

    let tags = api
        .tag_faces(TagFacesRequest {
            faces: vec![FaceId(1), FaceId(2)],
            tags: vec!["brand".into()],
        })
        .unwrap();
    assert_eq!(tags[0].face_count, 2);
    assert_eq!(api.get_face(FaceId(2)).unwrap().tags, ["brand"]);

    api.update_user_meta(UserMetaUpdate {
        faces: vec![FaceId(1)],
        favorite: Some(true),
        rating: Some(4),
        notes: None,
    })
    .unwrap();
    let f = api.get_face(FaceId(1)).unwrap();
    assert_eq!((f.favorite, f.rating), (true, Some(4)));
    let bad = api.update_user_meta(UserMetaUpdate {
        faces: vec![FaceId(1)],
        rating: Some(6),
        ..Default::default()
    });
    assert_eq!(bad.unwrap_err().code, ErrorCode::InvalidRequest);

    let set = api
        .create_set(CreateSetRequest {
            name: "Acme".into(),
            faces: vec![FaceId(1), FaceId(5)],
        })
        .unwrap();
    let mut exported = api.export_set(set.id).unwrap();
    assert_eq!(exported.fonts.len(), 2);
    exported.fonts[1].hash = "not-in-this-library".into();
    let imported = api.import_set(exported).unwrap();
    assert_eq!(imported.set.faces, [FaceId(1)]);
    assert_eq!(imported.missing.len(), 1);
}

#[test]
fn export_honors_the_license_guard() {
    let api = api();
    let ok = api
        .export_web(ExportWebRequest {
            faces: vec![FaceId(1)],
            formats: vec![WebFormat::Woff2],
            subset: None,
            out_dir: "/tmp/out".into(),
            css: true,
        })
        .unwrap();
    assert_eq!(
        ok.files,
        ["/tmp/out/Inter-Regular.woff2", "/tmp/out/fonts.css"]
    );
    assert!(ok.css.unwrap().contains("font-weight: 100 900;"));

    let refused = api.export_web(ExportWebRequest {
        faces: vec![FaceId(7)],
        formats: vec![WebFormat::Woff2],
        subset: None,
        out_dir: "/tmp/out".into(),
        css: false,
    });
    assert_eq!(refused.unwrap_err().code, ErrorCode::PermissionDenied);
}

#[test]
fn workflows_compose_the_other_services() {
    let api = api();
    let list = api
        .pick_fonts_for_brief(BriefRequest {
            brief: "bilingual Serbian brand".into(),
            languages: vec!["sr-Latn".into(), "sr-Cyrl".into()],
            license: None,
            limit: None,
        })
        .unwrap();
    let names: Vec<_> = list
        .candidates
        .iter()
        .map(|c| c.family.name.as_str())
        .collect();
    assert_eq!(names, ["Inter"]); // JetBrains Mono lacks the Serbian Latin letters

    let r = api
        .prepare_for_web(PrepareForWebRequest {
            family: FamilyId(1),
            subset: None,
            out_dir: "/tmp/web".into(),
        })
        .unwrap();
    assert_eq!(r.activated.len(), 2);
    assert!(r.export.css.is_some());
    // Refused by the license guard before anything is activated.
    assert!(
        api.prepare_for_web(PrepareForWebRequest {
            family: FamilyId(4),
            subset: None,
            out_dir: "/tmp/web".into()
        })
        .is_err()
    );
    assert_eq!(api.list_active().unwrap().len(), 2);
}

#[test]
fn previews_and_font_sources() {
    let api = api();
    let png = api
        .render_preview(PreviewRequest {
            face: FaceId(1),
            text: "Hamburgefonts".into(),
            size_px: 48.0,
            variations: vec![],
            features: vec![],
            foreground: None,
            background: None,
        })
        .unwrap();
    assert_eq!(&png.png[1..4], b"PNG");
    let too_big = PreviewRequest {
        face: FaceId(1),
        text: "x".into(),
        size_px: 400.0,
        variations: vec![],
        features: vec![],
        foreground: None,
        background: None,
    };
    assert_eq!(
        api.render_preview(too_big).unwrap_err().code,
        ErrorCode::InvalidRequest
    );
    assert!(api.font_source(FaceId(3)).unwrap().path.ends_with(".otf"));
    assert_eq!(
        api.find_similar(fm_api::search::SimilarRequest {
            face: FaceId(3),
            limit: 2
        })
        .unwrap()[0]
            .face
            .family,
        FamilyId(2)
    );
}

#[test]
fn problems_list_unsupported_files_and_system_clashes() {
    let kinds: Vec<_> = api()
        .list_problems()
        .unwrap()
        .iter()
        .map(|p| (p.face.unwrap().0, format!("{:?}", p.kind)))
        .collect();
    assert_eq!(
        kinds,
        [
            (6, "SystemConflict".to_string()),
            (7, "Unsupported".to_string())
        ]
    );
}

#[test]
fn face_detail_carries_the_parsed_names_angle_and_coverage() {
    let api = api();
    let italic = api.get_face(FaceId(2)).unwrap();
    assert_eq!(italic.legacy_family.as_deref(), Some("Inter"));
    assert_eq!(italic.legacy_style.as_deref(), Some("Italic"));
    assert_eq!(italic.foundry.as_deref(), Some("rsms"));
    assert_eq!(italic.italic_angle, Some(-10.0));
    assert!(italic.code_point_count.is_some_and(|n| n > 0));
    assert_eq!(api.get_face(FaceId(1)).unwrap().italic_angle, Some(0.0));
}

#[test]
fn face_detail_without_the_newer_fields_still_reads() {
    let mut json = serde_json::to_value(api().get_face(FaceId(1)).unwrap()).unwrap();
    let obj = json.as_object_mut().unwrap();
    for key in [
        "legacyFamily",
        "legacyStyle",
        "foundry",
        "italicAngle",
        "codePointCount",
    ] {
        assert!(obj.remove(key).is_some(), "{key} is on the wire");
    }
    let detail: fm_api::library::FaceDetail = serde_json::from_value(json).unwrap();
    assert_eq!(detail.legacy_family, None);
    assert_eq!(detail.code_point_count, None);
}
