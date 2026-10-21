//! The mock honors the contract: every service reachable through `dyn Api`,
//! per-face outcomes, no deactivation of fonts the app did not register,
//! the license guard, and events instead of polling.

use std::sync::Arc;

use fm_api::activation::{
    ActivateRequest, ActivationMode, Clash, ClashWinner, ConflictPolicy, ConflictWith,
    DeactivateRequest, OutcomeStatus, Target,
};
use fm_api::common::{AxisValue, FeatureSetting, FontFormat, Page, Slant};
use fm_api::export::{ExportWebRequest, GlyphSvgRequest, WebFormat};
use fm_api::layout::{
    BlockOverflow, DuplicateLayoutRequest, LayoutPreset, LayoutRef, RestoreLayoutRequest,
    RevertLayoutRequest, SaveLayoutRequest,
};
use fm_api::library::{
    AddSourceRequest, FamilyOrder, FamilySort, FamilySortKey, FamilySummary, GlyphRequest,
    GlyphsRequest, ImportMode, KeepCopyRequest, LicenseType, ListFamiliesRequest, OpenLicense,
    ScanRequest, SetSourcePausedRequest, TrashMoveStatus,
};
use fm_api::mock::MockApi;
use fm_api::organization::{
    Collection, CollectionFacesRequest, CreateCollectionRequest, CreateSetRequest,
    FamilyNotesUpdate, LicenseDeclaration, MoveCollectionRequest, PinInstanceRequest,
    ReceiptChange, ReceiptPlace, RenameCollectionRequest, Rule, RuleGroup, RuleOp, TagFacesRequest,
    UntagFacesRequest, UserMetaUpdate, VirtualStylesRequest,
};
use fm_api::preview::{CssValuesRequest, PreviewRequest, SampleFallback, SampleTextRequest};
use fm_api::search::{CoverageRequest, Filters, SearchRequest, Supports, WeightRange};
use fm_api::workflows::{BriefRequest, PrepareForWebRequest};
use fm_api::{
    API_VERSION, Api, CollectionId, ErrorCode, Event, FaceId, FamilyId, ResolutionId, SourceId,
};

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
fn replace_is_for_the_session_and_the_clash_is_listed_with_its_winner() {
    let api = api();
    assert_eq!(api.list_conflicts().unwrap(), []);
    // 6 JetBrains Mono clashes with a system font: permanent is asked, the
    // replace makes it a session activation; 1 Inter has no clash.
    let out = api
        .activate(acts(
            &[1, 6],
            ActivationMode::Permanent,
            ConflictPolicy::Replace,
        ))
        .unwrap();
    assert_eq!(out[0].outcome, OutcomeStatus::Done);
    assert_eq!(out[1].outcome, OutcomeStatus::Done);
    let modes: Vec<_> = api
        .list_active()
        .unwrap()
        .iter()
        .map(|a| (a.face, a.mode))
        .collect();
    assert_eq!(
        modes,
        [
            (FaceId(1), ActivationMode::Permanent),
            (FaceId(6), ActivationMode::Session)
        ]
    );
    assert_eq!(
        api.list_conflicts().unwrap(),
        [Clash {
            face: FaceId(6),
            postscript_name: api.get_face(FaceId(6)).unwrap().face.postscript_name,
            with: ConflictWith::System {
                path: "/System/Library/Fonts/JetBrainsMono-Regular.ttf".into()
            },
            winner: ClashWinner::Face,
        }]
    );
    api.deactivate(DeactivateRequest::default()).unwrap();
    assert_eq!(api.list_conflicts().unwrap(), []);
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
            except: None,
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
fn deactivate_except_the_sample_set_keeps_only_its_faces() {
    let api = api();
    let set = api.list_sets().unwrap()[0].clone();
    assert_eq!(set.faces, [FaceId(1), FaceId(6)]);
    api.activate(acts(
        &[1, 2, 6],
        ActivationMode::Session,
        ConflictPolicy::Replace,
    ))
    .unwrap();
    let out = api
        .deactivate(DeactivateRequest {
            target: None,
            except: Some(Target::Set { set: set.id }),
        })
        .unwrap();
    let gone: Vec<u32> = out.iter().map(|o| o.face.0).collect();
    assert_eq!(gone, [2]);
    let left: Vec<u32> = api
        .list_active()
        .unwrap()
        .iter()
        .map(|a| a.face.0)
        .collect();
    assert_eq!(left, [1, 6]);
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
    assert_eq!(got[0], Event::ScanStarted { job, source: None });
    assert!(matches!(got[1], Event::ScanProgress { job: j, .. } if j == job));
    assert!(matches!(got[2], Event::ScanFinished { problems: 1, .. }));
    assert_eq!(
        got[3],
        Event::ActivationChanged {
            activated: vec![FaceId(1)],
            deactivated: vec![]
        }
    );
    assert_eq!(b.try_iter().count(), 4);
    drop(b); // a dropped subscriber does not break the next send
    api.deactivate(DeactivateRequest::default()).unwrap();
    assert_eq!(a.try_iter().count(), 1);
}

#[test]
fn rebuild_index_rescans_every_source_and_keeps_activation() {
    let api = api();
    api.activate(acts(
        &[1],
        ActivationMode::Session,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();
    let families = api.list_families(ListFamiliesRequest::default()).unwrap();
    let events = api.subscribe();

    let job = api.rebuild_index().unwrap();

    let got: Vec<Event> = events.try_iter().collect();
    assert_eq!(got[0], Event::ScanStarted { job, source: None });
    assert!(matches!(got.last(), Some(Event::ScanFinished { job: j, .. }) if *j == job));
    let active: Vec<FaceId> = api.list_active().unwrap().iter().map(|a| a.face).collect();
    assert_eq!(active, [FaceId(1)]);
    assert_eq!(
        api.list_families(ListFamiliesRequest::default()).unwrap(),
        families
    );
}

#[test]
fn search_counts_every_filter_value_with_the_other_filters_kept() {
    let api = api();
    let all = api.search(SearchRequest::default()).unwrap();
    let counts = all.counts.expect("the mock reports counts");
    // Some sample families have italics; every family has an upright face.
    let italic = counts
        .slant
        .iter()
        .find(|c| c.value == Slant::Italic)
        .unwrap();
    assert!(italic.count > 0 && italic.count < all.total);
    let upright = counts
        .slant
        .iter()
        .find(|c| c.value == Slant::Upright)
        .unwrap();
    assert_eq!(upright.count, all.total);
    // Values with no result are left out: the sample has no color fonts.
    assert!(counts.color.iter().all(|c| !c.value));
    // A count is the total the search gives with that value chosen.
    for c in &counts.formats {
        let r = api
            .search(SearchRequest {
                filters: Filters {
                    formats: vec![c.value],
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.total, c.count, "{:?}", c.value);
    }

    // An any-of filter is counted without its own selection, so the other formats stay.
    let ttf = Filters {
        formats: vec![FontFormat::Ttf],
        ..Default::default()
    };
    let r = api
        .search(SearchRequest {
            filters: ttf.clone(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.counts.unwrap().formats, counts.formats);
    // Other filters are kept: with italics only, no count is above the italic families.
    let r = api
        .search(SearchRequest {
            filters: Filters {
                slant: Some(Slant::Italic),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    assert!(
        r.counts
            .unwrap()
            .formats
            .iter()
            .all(|c| c.count <= italic.count)
    );
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
    // Both carry the sample's Serbian locl forms: the badge of the results.
    assert!(
        r.families
            .iter()
            .all(|h| h.missing.as_ref().is_some_and(|m| m.has_locl))
    );

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
    assert!(
        !r.families[0].missing.as_ref().unwrap().has_locl,
        "text has no language"
    );

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
fn collections_nest_reorder_and_take_families() {
    let api = api();
    let make = |name: &str, parent| {
        api.create_collection(CreateCollectionRequest {
            rule_group: None,
            name: name.into(),
            parent,
            rules: None,
        })
        .unwrap()
    };
    let names = |api: &Arc<dyn Api>| -> Vec<String> {
        api.list_collections()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect()
    };
    let a = make("A", None);
    let b = make("B", None);
    let c = make("C", Some(a.id));
    assert_eq!(names(&api), ["A", "C", "B"]);

    // Reorder at the top level, then nest B under A in front of C.
    api.move_collection(MoveCollectionRequest {
        collection: b.id,
        parent: None,
        position: 0,
    })
    .unwrap();
    assert_eq!(names(&api), ["B", "A", "C"]);
    let moved = api
        .move_collection(MoveCollectionRequest {
            collection: b.id,
            parent: Some(a.id),
            position: 0,
        })
        .unwrap();
    assert_eq!(moved.parent, Some(a.id));
    assert_eq!(names(&api), ["A", "B", "C"]);

    // Not into itself or a child; no two siblings with one name.
    let cycle = api.move_collection(MoveCollectionRequest {
        collection: a.id,
        parent: Some(c.id),
        position: 0,
    });
    assert_eq!(cycle.unwrap_err().code, ErrorCode::InvalidRequest);
    let clash = api.rename_collection(RenameCollectionRequest {
        collection: b.id,
        name: "C".into(),
    });
    assert_eq!(clash.unwrap_err().code, ErrorCode::Conflict);

    // A family adds each of its faces; a face can be in many collections.
    let inter = api
        .list_families(ListFamiliesRequest::default())
        .unwrap()
        .items[0]
        .id;
    let with_family = api
        .add_to_collection(CollectionFacesRequest {
            collection: c.id,
            faces: vec![FaceId(3)],
            families: vec![inter],
        })
        .unwrap();
    assert_eq!(with_family.face_count, 3);
    let also = api
        .add_to_collection(CollectionFacesRequest {
            collection: b.id,
            faces: vec![FaceId(3)],
            families: vec![],
        })
        .unwrap();
    assert_eq!(also.face_count, 1);
}

#[test]
fn collections_tags_meta_and_sets() {
    let api = api();
    let manual = api
        .create_collection(CreateCollectionRequest {
            rule_group: None,
            name: "Clients".into(),
            parent: None,
            rules: None,
        })
        .unwrap();
    let c = api
        .add_to_collection(CollectionFacesRequest {
            collection: manual.id,
            faces: vec![FaceId(1), FaceId(3)],
            families: vec![],
        })
        .unwrap();
    assert_eq!(c.face_count, 2);

    let smart = api
        .create_collection(CreateCollectionRequest {
            rule_group: None,
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
        families: vec![],
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
fn family_notes_are_the_familys_and_announced_as_a_library_change() {
    let api = api();
    let family = api.get_face(FaceId(1)).unwrap().face.family;
    let rx = api.subscribe();
    api.update_family_notes(FamilyNotesUpdate {
        family,
        notes: "Licensed for the rebrand".into(),
    })
    .unwrap();
    let faces: Vec<FaceId> = api
        .get_family(family)
        .unwrap()
        .faces
        .iter()
        .map(|f| f.id)
        .collect();
    assert!(matches!(
        rx.try_recv(),
        Ok(Event::LibraryChanged { updated, .. }) if updated == faces
    ));
    let summary = api.get_family(family).unwrap().summary;
    assert_eq!(summary.notes.as_deref(), Some("Licensed for the rebrand"));
    let detail = api.get_face(FaceId(1)).unwrap();
    assert_eq!(
        detail.family_notes.as_deref(),
        Some("Licensed for the rebrand")
    );
    assert_eq!(detail.notes, None, "the face's own notes are separate");

    api.update_family_notes(FamilyNotesUpdate {
        family,
        notes: String::new(),
    })
    .unwrap();
    assert_eq!(api.get_family(family).unwrap().summary.notes, None);
    let unknown = api.update_family_notes(FamilyNotesUpdate {
        family: FamilyId(999),
        notes: "x".into(),
    });
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn a_declared_license_applies_to_every_face_and_clears_field_by_field() {
    let api = api();
    let family = api.get_face(FaceId(1)).unwrap().face.family;
    let rx = api.subscribe();
    api.declare_license(LicenseDeclaration {
        family,
        license_type: Some(LicenseType::Web),
        seats: Some(5),
        purchase_date: Some("2026-03-01".into()),
        receipt: ReceiptChange::Attach {
            path: r"C:\Users\me\Downloads\invoice.pdf".into(),
            place: ReceiptPlace::AppData,
        },
    })
    .unwrap();
    let faces: Vec<FaceId> = api
        .get_family(family)
        .unwrap()
        .faces
        .iter()
        .map(|f| f.id)
        .collect();
    assert!(matches!(
        rx.try_recv(),
        Ok(Event::LibraryChanged { updated, .. }) if updated == faces
    ));
    assert_eq!(
        api.get_family(family).unwrap().summary.declared_license,
        Some(LicenseType::Web)
    );
    for face in &faces {
        let license = api.get_face(*face).unwrap().license;
        assert_eq!(license.declared, Some(LicenseType::Web));
        assert_eq!(license.seats, Some(5));
        assert_eq!(license.purchase_date.as_deref(), Some("2026-03-01"));
        assert_eq!(
            license.receipt.as_deref(),
            Some("/app-data/receipts/invoice.pdf")
        );
    }
    // Absent fields clear; the receipt stays unless cleared.
    api.declare_license(LicenseDeclaration {
        family,
        license_type: None,
        seats: None,
        purchase_date: None,
        receipt: ReceiptChange::Keep,
    })
    .unwrap();
    let license = api.get_face(FaceId(1)).unwrap().license;
    assert_eq!((license.declared, license.seats), (None, None));
    assert!(license.receipt.is_some());
    api.declare_license(LicenseDeclaration {
        family,
        license_type: None,
        seats: None,
        purchase_date: None,
        receipt: ReceiptChange::Clear,
    })
    .unwrap();
    assert_eq!(api.get_face(FaceId(1)).unwrap().license.receipt, None);

    let bad = |req: LicenseDeclaration| api.declare_license(req).unwrap_err().code;
    let base = LicenseDeclaration {
        family,
        license_type: None,
        seats: None,
        purchase_date: None,
        receipt: ReceiptChange::Keep,
    };
    assert_eq!(
        bad(LicenseDeclaration {
            seats: Some(-1),
            ..base.clone()
        }),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        bad(LicenseDeclaration {
            purchase_date: Some("2026-13-01".into()),
            ..base.clone()
        }),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        bad(LicenseDeclaration {
            receipt: ReceiptChange::Attach {
                path: String::new(),
                place: ReceiptPlace::NextToFont,
            },
            ..base.clone()
        }),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        bad(LicenseDeclaration {
            family: FamilyId(999),
            ..base
        }),
        ErrorCode::NotFound
    );
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
fn the_sample_library_has_no_duplicates() {
    // Every sample file has its own hash and PostScript name.
    assert_eq!(api().list_duplicates().unwrap(), []);
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

#[test]
fn settings_update_through_dyn_api_and_a_bad_value_changes_nothing() {
    use fm_api::settings::{Density, Settings, SettingsUpdate, Theme};
    let api = api();
    assert_eq!(api.get_settings().unwrap(), Settings::default());
    let next = api
        .update_settings(SettingsUpdate {
            theme: Some(Theme::Dark),
            on_conflict: Some(ConflictPolicy::Replace),
            ..SettingsUpdate::default()
        })
        .unwrap();
    assert_eq!(next.theme, Theme::Dark);
    assert_eq!(next.on_conflict, ConflictPolicy::Replace);
    assert_eq!(api.get_settings().unwrap(), next);

    let err = api
        .update_settings(SettingsUpdate {
            density: Some(Density::Compact),
            cache_limit_mb: Some(1),
            ..SettingsUpdate::default()
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert_eq!(
        api.get_settings().unwrap(),
        next,
        "a rejected update is not half-applied"
    );
    assert!(api.app_folders().unwrap().logs.ends_with("logs"));
}

#[test]
fn family_rows_carry_the_detail_list_columns() {
    let api = api();
    api.tag_faces(TagFacesRequest {
        faces: vec![FaceId(3)],
        tags: vec!["Body".into(), "brand".into()],
    })
    .unwrap();
    let all = api.list_families(ListFamiliesRequest::default()).unwrap();
    let by_name = |n: &str| all.items.iter().find(|f| f.name == n).unwrap();

    let inter = by_name("Inter");
    // Variable: the range is the `wght` axis, not the one face's 400.
    assert_eq!(inter.weight_range, Some(WeightRange { min: 100, max: 900 }));
    assert_eq!(inter.version.as_deref(), Some("Version 1.000"));
    assert_eq!(inter.location.as_deref(), Some("/Users/demo/Fonts"));
    assert_eq!(inter.open_license, Some(OpenLicense::Ofl));
    assert!(!inter.is_color);

    let serif = by_name("Source Serif 4");
    let marks: Vec<_> = serif.styles.iter().map(|s| (s.weight, s.italic)).collect();
    assert_eq!(marks, [(400, false), (400, true), (700, false)]);
    assert_eq!(serif.weight_range, Some(WeightRange { min: 400, max: 700 }));
    assert_eq!(serif.tags, ["Body", "brand"]); // A to Z, case-insensitive

    let legacy = by_name("Legacy Grotesk");
    assert_eq!(legacy.open_license, None); // restricted, no open license
    assert!(legacy.tags.is_empty());
}

#[test]
fn family_rows_without_the_newer_fields_still_read() {
    let all = api().list_families(ListFamiliesRequest::default()).unwrap();
    let mut json = serde_json::to_value(&all.items[0]).unwrap();
    let obj = json.as_object_mut().unwrap();
    for key in [
        "styles",
        "weightRange",
        "isColor",
        "version",
        "location",
        "openLicense",
        "declaredLicense",
        "tags",
    ] {
        assert!(obj.remove(key).is_some(), "{key} is on the wire");
    }
    let row: FamilySummary = serde_json::from_value(json).unwrap();
    assert!(row.styles.is_empty() && row.tags.is_empty());
    assert_eq!((row.weight_range, row.version), (None, None));

    // A request without `order` is the old request.
    let req: ListFamiliesRequest = serde_json::from_str(r#"{"sort":"styleCount"}"#).unwrap();
    assert_eq!((req.sort, req.order), (FamilySort::StyleCount, None));
}

fn names_in(api: &Arc<dyn Api>, by: FamilySortKey, descending: bool) -> Vec<String> {
    let page = api.list_families(ListFamiliesRequest {
        order: Some(FamilyOrder { by, descending }),
        ..Default::default()
    });
    page.map(|p| p.items.into_iter().map(|f| f.name).collect())
        .unwrap_or_default()
}

#[test]
fn families_sort_by_any_detail_list_column() {
    let api = api();
    assert_eq!(
        names_in(&api, FamilySortKey::Name, true),
        [
            "Source Serif 4",
            "Legacy Grotesk",
            "JetBrains Mono",
            "Inter"
        ]
    );
    assert_eq!(
        names_in(&api, FamilySortKey::Manufacturer, false),
        [
            "Source Serif 4",
            "JetBrains Mono",
            "Legacy Grotesk",
            "Inter"
        ] // Adobe, JetBrains, Old, rsms
    );
    assert_eq!(
        names_in(&api, FamilySortKey::DateAdded, true),
        [
            "Legacy Grotesk",
            "JetBrains Mono",
            "Source Serif 4",
            "Inter"
        ]
    );
    // Inter starts at 100; the others at 400, then by their heaviest weight.
    assert_eq!(names_in(&api, FamilySortKey::Weight, false)[0], "Inter");
    // Rows with no license come last in both directions.
    assert_eq!(
        names_in(&api, FamilySortKey::License, false)
            .last()
            .unwrap(),
        "Legacy Grotesk"
    );
    assert_eq!(
        names_in(&api, FamilySortKey::License, true).last().unwrap(),
        "Legacy Grotesk"
    );

    api.activate(acts(
        &[3],
        ActivationMode::Session,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();
    assert_eq!(
        names_in(&api, FamilySortKey::Activation, true)[0],
        "Source Serif 4" // partly active
    );
    // `order` replaces `sort`.
    let page = api
        .list_families(ListFamiliesRequest {
            sort: FamilySort::StyleCount,
            order: Some(FamilyOrder {
                by: FamilySortKey::Name,
                descending: false,
            }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.items[0].name, "Inter");
}

#[test]
fn sample_texts_fall_back_to_the_faces_own_script() {
    let api = api();
    let ask = |faces: &[u32], text: &str| {
        api.sample_texts(SampleTextRequest {
            faces: faces.iter().map(|&f| FaceId(f)).collect(),
            text: text.into(),
        })
        .expect("samples")
    };
    // Face 1 maps Latin, Latin Extended and Cyrillic; face 7 only Latin.
    let got = ask(&[1, 7], "Źdźbło trawy");
    assert_eq!(got[0].text, "Źdźbło trawy");
    assert_eq!(got[0].fallback, None);
    assert_eq!(got[1].face, FaceId(7));
    assert_eq!(got[1].text, "abcdefghijklmnopqrstuvwxyz");
    assert_eq!(
        got[1].fallback,
        Some(SampleFallback::Script {
            script: "Latn".into()
        })
    );
    assert_eq!(got[1].missing, ["Ź", "ź", "ł"]);
    let got = ask(&[7], "The quick fox");
    assert_eq!(
        (got[0].text.as_str(), &got[0].fallback),
        ("The quick fox", &None)
    );
    let err = api
        .sample_texts(SampleTextRequest {
            faces: vec![FaceId(999)],
            text: "a".into(),
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

/// A mock whose face 3 has a second copy, an exact duplicate.
#[allow(clippy::unwrap_used)] // test setup: a failure fails the test
fn with_copy() -> (Arc<dyn Api>, String, String) {
    let mock = MockApi::new();
    let copy = "/Users/demo/Downloads/SourceSerif4-Regular.otf".to_owned();
    mock.add_copy(FaceId(3), &copy).unwrap();
    let api: Arc<dyn Api> = Arc::new(mock);
    let original = api.get_face(FaceId(3)).unwrap().path;
    (api, original, copy)
}

fn keep(keep: &str, trash: &str, dry_run: bool) -> KeepCopyRequest {
    KeepCopyRequest {
        keep: keep.into(),
        trash: vec![trash.into()],
        dry_run,
    }
}

#[test]
fn keep_this_one_trashes_the_other_copy_and_undo_brings_it_back() {
    let (api, original, copy) = with_copy();
    assert_eq!(api.list_duplicates().unwrap().len(), 1);
    let events = api.subscribe();

    let plan = api.keep_copy(keep(&original, &copy, true)).unwrap();
    assert_eq!((plan.id, plan.dry_run), (None, true));
    assert_eq!(plan.moves[0].status, TrashMoveStatus::Planned);
    assert_eq!(
        api.list_duplicates().unwrap().len(),
        1,
        "a dry run moves nothing"
    );
    assert_eq!(api.list_resolutions().unwrap(), []);

    let done = api.keep_copy(keep(&original, &copy, false)).unwrap();
    let id = done.id.expect("recorded for undo");
    assert_eq!(done.moves[0].status, TrashMoveStatus::Trashed);
    assert_eq!(api.list_duplicates().unwrap(), []);
    assert_eq!(api.list_resolutions().unwrap(), [done]);
    assert!(matches!(
        events.try_iter().last(),
        Some(Event::LibraryChanged { removed, .. }) if removed.len() == 1
    ));

    let undone = api.undo_resolution(id).unwrap();
    assert!(undone.undone);
    assert_eq!(undone.moves[0].status, TrashMoveStatus::Restored);
    assert_eq!(api.list_duplicates().unwrap().len(), 1);
    assert_eq!(api.list_resolutions().unwrap(), [undone]);
}

#[test]
fn keep_this_one_skips_an_active_copy_and_refuses_a_stranger() {
    let (api, original, copy) = with_copy();
    let copy_face = api.list_duplicates().unwrap()[0]
        .copies
        .iter()
        .find(|c| c.path == copy)
        .and_then(|c| c.face)
        .unwrap();
    api.activate(acts(
        &[copy_face.0],
        ActivationMode::Session,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();

    let r = api.keep_copy(keep(&original, &copy, false)).unwrap();
    assert_eq!(r.id, None, "nothing moved, nothing to undo");
    assert_eq!(r.moves[0].status, TrashMoveStatus::Skipped);
    assert!(r.moves[0].message.as_deref().unwrap().contains("active"));

    let err = api
        .keep_copy(keep(&original, "/Users/demo/notes.txt", false))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidRequest);
    assert_eq!(
        api.undo_resolution(ResolutionId(999)).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn the_glyph_map_lists_a_face_by_block_and_searches_it() {
    let api = api();
    let req = |block: Option<&str>, query: Option<&str>| GlyphsRequest {
        face: FaceId(1),
        block: block.map(Into::into),
        query: query.map(Into::into),
        page: Page::default(),
    };
    let all = api.list_glyphs(req(None, None)).unwrap();
    assert!(all.total > 0);
    assert_eq!(all.blocks.iter().map(|b| b.count).sum::<u32>(), all.total);
    assert!(
        all.items
            .windows(2)
            .all(|w| w[0].code_point < w[1].code_point)
    );
    let latin = api.list_glyphs(req(Some("Basic Latin"), None)).unwrap();
    assert!(latin.items.iter().all(|g| g.block == "Basic Latin"));
    for q in ["A", "U+0041", "0041"] {
        let found = api.list_glyphs(req(None, Some(q))).unwrap();
        assert_eq!(found.items[0].code_point, 0x41, "{q}");
    }
    let err = api
        .list_glyphs(GlyphsRequest {
            face: FaceId(999),
            ..req(None, None)
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn the_glyph_detail_matches_the_glyph_map_and_names_features() {
    let api = api();
    let listed = api
        .list_glyphs(GlyphsRequest {
            face: FaceId(1),
            block: None,
            query: Some("a".into()),
            page: Page::default(),
        })
        .unwrap()
        .items[0]
        .clone();
    let a = api
        .get_glyph(GlyphRequest {
            face: FaceId(1),
            code_point: 'a' as u32,
        })
        .unwrap();
    assert_eq!((a.glyph_id, a.name.clone()), (listed.glyph_id, listed.name));
    assert!(a.advance_width.is_some());
    let tags: Vec<_> = a.replaced_by.iter().map(|f| f.tag.as_str()).collect();
    assert_eq!(tags, ["smcp", "ss01"]);
    assert!(a.replaced_by.iter().all(|f| f.name.is_some()));
    let err = api
        .get_glyph(GlyphRequest {
            face: FaceId(1),
            code_point: 0x10_FFFD,
        })
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
}

fn svg(api: &Arc<dyn Api>, c: char, wght: Option<f32>) -> Result<String, fm_api::ApiError> {
    api.export_glyph_svg(GlyphSvgRequest {
        face: FaceId(1),
        code_point: c as u32,
        variations: wght
            .map(|value| AxisValue {
                tag: "wght".into(),
                value,
            })
            .into_iter()
            .collect(),
    })
}

#[test]
fn a_glyph_exports_as_svg_text_and_follows_the_weight() {
    let api = api();
    let a = svg(&api, 'a', None).unwrap();
    assert!(
        a.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
        "{a}"
    );
    assert!(a.contains("<path d=\"M"), "{a}");
    let width = api
        .get_glyph(GlyphRequest {
            face: FaceId(1),
            code_point: 'a' as u32,
        })
        .unwrap()
        .advance_width
        .unwrap();
    assert!(a.contains(&format!("viewBox=\"0 0 {width} 1000\"")), "{a}");
    assert_ne!(
        svg(&api, 'a', Some(100.0)).unwrap(),
        svg(&api, 'a', Some(900.0)).unwrap()
    );

    let space = svg(&api, ' ', None).unwrap();
    assert!(space.contains("<path d=\"\"/>"), "{space}");

    let err = svg(&api, '\u{10FFFD}', None).unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    let err = svg(&api, 'a', Some(f32::NAN)).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidRequest);
}

#[test]
fn virtual_styles_pin_list_delete_and_activate_their_face_once() {
    let api = api();
    let brand = api
        .create_collection(CreateCollectionRequest {
            rule_group: None,
            name: "Brand".into(),
            parent: None,
            rules: None,
        })
        .unwrap()
        .id;
    let pin = |name: &str, face: u32, wght: Option<f32>| PinInstanceRequest {
        face: FaceId(face),
        name: name.into(),
        coordinates: wght
            .map(|value| AxisValue {
                tag: "wght".into(),
                value,
            })
            .into_iter()
            .collect(),
        collection: Some(brand),
        set: None,
    };
    // Inter (face 1) is variable: wght 100–900, default 400.
    let semibold = api.pin_instance(pin("Semibold", 1, Some(600.0))).unwrap();
    assert_eq!(semibold.coordinates[0].value, 600.0);
    let book = api.pin_instance(pin("Book", 1, None)).unwrap();
    assert_eq!(book.coordinates[0].value, 400.0, "left out: the default");
    let listed = api
        .list_virtual_styles(VirtualStylesRequest {
            collection: Some(brand),
            set: None,
        })
        .unwrap();
    assert_eq!(listed, vec![book.clone(), semibold.clone()]);

    for (req, code) in [
        (pin("Light", 3, None), ErrorCode::InvalidRequest), // static
        (pin("Too heavy", 1, Some(1000.0)), ErrorCode::InvalidRequest),
        (pin("semibold", 1, None), ErrorCode::Conflict),
        (
            PinInstanceRequest {
                collection: None,
                ..pin("Nowhere", 1, None)
            },
            ErrorCode::InvalidRequest,
        ),
    ] {
        assert_eq!(api.pin_instance(req).unwrap_err().code, code);
    }

    // Two instances of one face: activated once.
    let out = api
        .activate(ActivateRequest {
            target: Target::Collection { collection: brand },
            mode: ActivationMode::Session,
            on_conflict: ConflictPolicy::KeepExisting,
        })
        .unwrap();
    let faces: Vec<FaceId> = out.iter().map(|o| o.face).collect();
    assert_eq!(faces, vec![FaceId(1)]);

    api.delete_virtual_style(book.id).unwrap();
    assert_eq!(
        api.delete_virtual_style(book.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    api.delete_collection(brand).unwrap();
    assert_eq!(
        api.list_virtual_styles(VirtualStylesRequest {
            collection: Some(brand),
            set: None,
        })
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
}

/// The faces a search with `filters` finds, sorted.
fn faces_of(api: &Arc<dyn Api>, filters: Filters) -> fm_api::ApiResult<Vec<FaceId>> {
    let res = api.search(SearchRequest {
        filters,
        page: Page {
            offset: 0,
            limit: 1000,
        },
        ..SearchRequest::default()
    })?;
    let mut faces: Vec<FaceId> = res.families.into_iter().flat_map(|h| h.faces).collect();
    faces.sort_unstable();
    Ok(faces)
}

fn smart(
    api: &Arc<dyn Api>,
    name: &str,
    op: RuleOp,
    rules: Vec<Rule>,
) -> fm_api::ApiResult<Collection> {
    api.create_collection(CreateCollectionRequest {
        name: name.into(),
        parent: None,
        rules: None,
        rule_group: Some(RuleGroup { op, rules }),
    })
}

fn by(c: CollectionId) -> Filters {
    Filters {
        collection: Some(c),
        ..Filters::default()
    }
}

/// E14.2: the mock evaluates smart collections like the core: AND/OR,
/// nested groups, rules through other collections, live counts, the date
/// added, and loops refused.
#[test]
fn smart_collections_follow_the_rules() {
    let api = api();
    let leaf = |filters: Filters| Rule::Filters { filters };
    let italic = faces_of(
        &api,
        Filters {
            slant: Some(Slant::Italic),
            ..Filters::default()
        },
    )
    .unwrap();
    let variable = faces_of(
        &api,
        Filters {
            variable: Some(true),
            ..Filters::default()
        },
    )
    .unwrap();
    let either = smart(
        &api,
        "Italic or variable",
        RuleOp::Or,
        vec![
            leaf(Filters {
                slant: Some(Slant::Italic),
                ..Filters::default()
            }),
            leaf(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
        ],
    )
    .unwrap();
    assert_eq!(either.rules, Some(Filters::default()));
    let mut union: Vec<FaceId> = italic.iter().chain(&variable).copied().collect();
    union.sort_unstable();
    union.dedup();
    assert_eq!(either.face_count as usize, union.len());
    assert_eq!(faces_of(&api, by(either.id)).unwrap(), union);

    let nested = smart(
        &api,
        "Nested",
        RuleOp::And,
        vec![
            leaf(by(either.id)),
            Rule::Group {
                group: RuleGroup {
                    op: RuleOp::Or,
                    rules: vec![leaf(Filters {
                        variable: Some(false),
                        ..Filters::default()
                    })],
                },
            },
        ],
    )
    .unwrap();
    let static_italic: Vec<FaceId> = union
        .iter()
        .copied()
        .filter(|f| !variable.contains(f))
        .collect();
    assert_eq!(faces_of(&api, by(nested.id)).unwrap(), static_italic);

    // A phase-1 rule reads as one AND group.
    let old = api
        .create_collection(CreateCollectionRequest {
            name: "Old".into(),
            parent: None,
            rules: Some(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
            rule_group: None,
        })
        .unwrap();
    assert_eq!(
        old.rule_group,
        Some(RuleGroup::from(Filters {
            variable: Some(true),
            ..Filters::default()
        }))
    );

    // Live: a tag shows at once.
    let first = faces_of(&api, Filters::default()).unwrap()[0];
    let tag = api
        .tag_faces(TagFacesRequest {
            faces: vec![first],
            tags: vec!["later".into()],
        })
        .unwrap()
        .into_iter()
        .find(|t| t.name == "later")
        .unwrap()
        .id;
    let tagged = smart(
        &api,
        "Tagged",
        RuleOp::And,
        vec![leaf(Filters {
            tags: vec![tag],
            ..Filters::default()
        })],
    )
    .unwrap();
    assert_eq!(tagged.face_count, 1);
    api.untag_faces(UntagFacesRequest {
        faces: vec![first],
        tags: vec![tag],
    })
    .unwrap();
    let listed = api.list_collections().unwrap();
    let count = listed
        .iter()
        .find(|c| c.id == tagged.id)
        .unwrap()
        .face_count;
    assert_eq!(count, 0);

    // Date added: the sample families were added 9 to 15 days before MOCK_NOW.
    let every = faces_of(&api, Filters::default()).unwrap();
    let within = |days| {
        faces_of(
            &api,
            Filters {
                added_within_days: Some(days),
                ..Filters::default()
            },
        )
        .unwrap()
    };
    assert_eq!(within(60), every);
    assert!(within(1).is_empty());

    // Loops and unknown collections.
    let next = CollectionId(tagged.id.0 + 1);
    let itself = smart(&api, "Itself", RuleOp::And, vec![leaf(by(next))]).unwrap_err();
    assert_eq!(itself.code, ErrorCode::InvalidRequest);
    let unknown = smart(
        &api,
        "Unknown",
        RuleOp::And,
        vec![leaf(by(CollectionId(999)))],
    );
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    let both = api.create_collection(CreateCollectionRequest {
        name: "Both".into(),
        parent: None,
        rules: Some(Filters::default()),
        rule_group: Some(RuleGroup::from(Filters::default())),
    });
    assert_eq!(both.unwrap_err().code, ErrorCode::InvalidRequest);
}

#[test]
fn css_values_write_only_what_differs_from_the_defaults() {
    let api = api();
    let css = |face: u32, variations: Vec<AxisValue>, features: Vec<FeatureSetting>| {
        api.css_values(CssValuesRequest {
            face: FaceId(face),
            variations,
            features,
        })
        .expect("css")
    };
    let at = |tag: &str, value: f32| AxisValue {
        tag: tag.into(),
        value,
    };
    let switch = |tag: &str, on: bool| FeatureSetting {
        tag: tag.into(),
        on,
    };
    // Inter (face 1) is variable: wght 100–900, default 400; kern and liga on.
    let got = css(
        1,
        vec![at("wght", 650.0001)],
        vec![
            switch("smcp", true),
            switch("liga", false),
            switch("kern", true),
        ],
    );
    assert_eq!(
        got.variation_settings,
        r#"font-variation-settings: "wght" 650;"#
    );
    assert_eq!(
        got.feature_settings,
        r#"font-feature-settings: "liga" 0, "smcp" 1;"#
    );
    let got = css(1, vec![at("wght", 400.0)], vec![switch("liga", true)]);
    assert_eq!(got.variation_settings, "font-variation-settings: normal;");
    assert_eq!(got.feature_settings, "font-feature-settings: normal;");
    let err = api
        .css_values(CssValuesRequest {
            face: FaceId(999),
            variations: Vec::new(),
            features: Vec::new(),
        })
        .expect_err("unknown face");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn face_coverage_lists_blocks_in_order_and_complete_languages_first() {
    let api = api();
    // JetBrains Mono: Latin and Cyrillic, no Latin Extended-A.
    let got = api.face_coverage(FaceId(6)).unwrap();
    assert_eq!(got.face, FaceId(6));
    let blocks: Vec<(&str, u32)> = got
        .blocks
        .iter()
        .map(|b| (b.name.as_str(), b.assigned))
        .collect();
    assert_eq!(blocks, [("Basic Latin", 128), ("Cyrillic", 256)]);
    assert!(got.blocks.iter().all(|b| b.covered > 0));
    let languages: Vec<(&str, bool)> = got
        .languages
        .iter()
        .map(|l| (l.code.as_str(), l.covered == l.total))
        .collect();
    assert_eq!(
        languages,
        [
            ("en", true),
            ("sr-Cyrl", true),
            ("sr-Latn", false),
            ("pl", false)
        ],
        "complete first, then partial by share"
    );
    assert!(got.languages[1].has_locl);
    assert_eq!(got.languages[1].name, "Serbian (Cyrillic)");
    assert_eq!(
        api.face_coverage(FaceId(999)).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn layout_presets_are_the_six_in_picker_order_and_clamp_keeps_ranges() {
    let api = api();
    let presets = api.layout_presets().expect("presets");
    let ids: Vec<_> = presets.iter().map(|l| l.preset).collect();
    assert_eq!(
        ids,
        [
            LayoutPreset::Book,
            LayoutPreset::Columns,
            LayoutPreset::Magazine,
            LayoutPreset::Quote,
            LayoutPreset::Characters,
            LayoutPreset::Waterfall,
        ]
        .map(Some)
    );
    let waterfall = &presets[5];
    assert!((6..=8).contains(&waterfall.blocks.len()));
    assert!(
        waterfall
            .blocks
            .iter()
            .all(|b| b.overflow == BlockOverflow::Clip)
    );
    for layout in &presets {
        assert_eq!(&api.clamp_layout(layout.clone()).expect("clamp"), layout);
    }
    let mut wild = presets[0].clone();
    wild.blocks[0].size_pt = 1.0;
    wild.blocks[0].line_height = f32::NAN;
    wild.blocks[0].columns = 9;
    let got = api.clamp_layout(wild).expect("clamp");
    assert_eq!(got.blocks[0].size_pt, 8.0);
    assert_eq!(got.blocks[0].line_height, 1.3);
    assert_eq!(got.blocks[0].columns, 3);
    let bounds = api.layout_bounds().expect("bounds");
    assert_eq!(got.blocks[0].size_pt, bounds.size_pt.min);
    assert_eq!(got.blocks[0].line_height, bounds.line_height.default);
    assert_eq!(got.blocks[0].columns, bounds.columns.max);
}

#[test]
fn layout_files_save_duplicate_revert_and_restore_like_the_core() {
    let api = api();
    let r = |id: &str| LayoutRef { id: id.into() };
    let entries = api.list_layouts().expect("list");
    let ids: Vec<_> = entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "book",
            "columns",
            "magazine",
            "quote",
            "characters",
            "waterfall",
            "draft"
        ]
    );
    assert!(entries[6].broken.is_some(), "the mock has one broken file");
    assert_eq!(
        api.get_layout(r("draft")).expect_err("broken").code,
        ErrorCode::InvalidRequest
    );

    let book = api.get_layout(r("book")).expect("book").layout;
    let mut big = book.clone();
    big.blocks[0].size_pt = 5_000.0;
    let saved = api
        .save_layout(SaveLayoutRequest {
            id: "book".into(),
            layout: big.clone(),
        })
        .expect("save");
    assert_eq!(saved.layout.blocks[0].size_pt, 600.0);
    api.save_layout(SaveLayoutRequest {
        id: "book".into(),
        layout: big,
    })
    .expect("same again");
    let history = api.layout_history(r("book")).expect("history");
    assert_eq!(history.len(), 1, "an unchanged save keeps no version");
    assert_eq!(history[0].name.as_deref(), Some("Book"));

    let copy = api
        .duplicate_layout(DuplicateLayoutRequest {
            id: "book".into(),
            name: None,
        })
        .expect("copy");
    assert_eq!(
        (copy.id.as_str(), copy.layout.name.as_str()),
        ("book-copy", "Book copy")
    );
    assert_eq!(
        api.duplicate_layout(DuplicateLayoutRequest {
            id: "book".into(),
            name: Some("BOOK".into())
        })
        .expect_err("taken")
        .code,
        ErrorCode::Conflict
    );

    assert_eq!(
        api.revert_layout(RevertLayoutRequest {
            id: "book".into(),
            to_original: false
        })
        .expect("revert")
        .layout,
        saved.layout
    );
    let reset = api
        .revert_layout(RevertLayoutRequest {
            id: "book".into(),
            to_original: true,
        })
        .expect("reset");
    assert_eq!(reset.layout, book);
    let restored = api
        .restore_layout(RestoreLayoutRequest {
            id: "book".into(),
            number: 2,
        })
        .expect("restore the reset-over version");
    assert_eq!(restored.layout, saved.layout);
    let numbers: Vec<_> = api
        .layout_history(r("book"))
        .expect("history")
        .into_iter()
        .map(|v| v.number)
        .collect();
    assert_eq!(numbers, [3, 2, 1]);
    // A broken file that is no preset has no original to reset to.
    let no_original = api
        .revert_layout(RevertLayoutRequest {
            id: "draft".into(),
            to_original: true,
        })
        .expect_err("draft is no preset");
    assert_eq!(no_original.code, ErrorCode::InvalidRequest);
}

#[test]
fn the_web_filter_follows_declarations_and_explains_each_face() {
    let api = api();
    let web = |ok: bool| {
        let res = api
            .search(SearchRequest {
                filters: Filters {
                    web_ok: Some(ok),
                    ..Filters::default()
                },
                ..SearchRequest::default()
            })
            .unwrap();
        res.families
            .into_iter()
            .map(|h| h.family.name)
            .collect::<Vec<_>>()
    };
    assert!(web(false).contains(&"Legacy Grotesk".to_owned()));
    assert!(web(true).contains(&"Inter".to_owned()));
    let legacy = api.get_face(FaceId(7)).unwrap();
    assert_eq!(legacy.face.family_name, "Legacy Grotesk");
    assert_eq!(
        legacy.license.web_use.map(|w| (w.ok, w.reason)),
        Some((
            false,
            "Embedding restricted, no web license declared".to_owned()
        ))
    );
    api.declare_license(LicenseDeclaration {
        family: legacy.face.family,
        license_type: Some(LicenseType::Web),
        seats: None,
        purchase_date: None,
        receipt: ReceiptChange::Keep,
    })
    .unwrap();
    assert!(web(true).contains(&"Legacy Grotesk".to_owned()));
    assert_eq!(
        api.get_face(FaceId(7))
            .unwrap()
            .license
            .web_use
            .map(|w| w.reason),
        Some("Declared web license".to_owned())
    );
}

/// E14.3: "activated within" keeps a face activated once, active now or not.
#[test]
fn activated_within_days_keeps_deactivated_faces() {
    let api = api();
    let recent = Filters {
        activated_within_days: Some(30),
        ..Filters::default()
    };
    assert_eq!(faces_of(&api, recent.clone()).unwrap(), []);
    api.activate(acts(
        &[3, 4],
        ActivationMode::Session,
        ConflictPolicy::KeepExisting,
    ))
    .unwrap();
    api.deactivate(DeactivateRequest {
        target: Some(Target::Faces {
            faces: vec![FaceId(3)],
        }),
        except: None,
    })
    .unwrap();
    assert_eq!(faces_of(&api, recent).unwrap(), [FaceId(3), FaceId(4)]);
}
