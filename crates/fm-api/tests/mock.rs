//! The mock honors the contract: every service reachable through `dyn Api`,
//! per-face outcomes, no deactivation of fonts the app did not register,
//! the license guard, and events instead of polling.

use std::sync::Arc;

use fm_api::activation::{
    ActivateRequest, ActivationMode, Clash, ClashWinner, ConflictPolicy, ConflictWith,
    DeactivateRequest, OutcomeStatus, Target,
};
use fm_api::common::{AxisValue, Embedding, FeatureSetting, FontFormat, Page, Slant};
use fm_api::comparison::{
    CompareMode, ComparisonRef, RenameComparisonRequest, SaveComparisonRequest,
};
use fm_api::export::{
    CollectionSpecimenRequest, ExportWebRequest, Framework, FrameworkSnippetRequest,
    GlyphSvgRequest, SpecimenFormat, SpecimenRequest, Subset, WebFormat,
};
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
    QuickCollectionRequest, ReceiptChange, ReceiptPlace, RenameCollectionRequest, Rule, RuleGroup,
    RuleOp, SelectionTagsRequest, TagFacesRequest, UntagFacesRequest, UpdateCollectionRulesRequest,
    UserMetaUpdate, VirtualStylesRequest,
};
use fm_api::preview::{
    CssValuesRequest, PreviewRequest, SampleFallback, SampleTextRequest, SizeBasis,
    SizeFactorRequest,
};
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
            families: Vec::new(),
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
fn the_quick_collection_keeps_faces_in_the_order_added() {
    let api = api();
    let ids = |q: fm_api::organization::QuickCollection| -> Vec<FaceId> {
        q.faces.iter().map(|f| f.id).collect()
    };
    let req = |faces: &[u32]| QuickCollectionRequest {
        faces: faces.iter().copied().map(FaceId).collect(),
    };
    assert!(api.quick_collection().unwrap().faces.is_empty());
    let got = api.add_to_quick_collection(req(&[6, 1, 6])).unwrap();
    assert_eq!(ids(got), [FaceId(6), FaceId(1)]);
    let got = api.add_to_quick_collection(req(&[2, 1])).unwrap();
    assert_eq!(ids(got), [FaceId(6), FaceId(1), FaceId(2)]);
    let unknown = api.add_to_quick_collection(req(&[3, 999]));
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
    let got = api.remove_from_quick_collection(req(&[1])).unwrap();
    assert_eq!(ids(got), [FaceId(6), FaceId(2)]);
    assert_eq!(ids(api.quick_collection().unwrap()), [FaceId(6), FaceId(2)]);
    api.clear_quick_collection().unwrap();
    assert!(api.quick_collection().unwrap().faces.is_empty());
}

#[test]
fn named_comparisons_are_saved_listed_renamed_reopened_and_deleted() {
    let api = api();
    let save = |name: &str, faces: &[u32]| {
        api.save_comparison(SaveComparisonRequest {
            id: None,
            name: name.into(),
            faces: faces.iter().copied().map(FaceId).collect(),
            mode: CompareMode::Stacked,
            sample_text: "Quartz".into(),
        })
    };
    let code =
        |r: fm_api::ApiResult<fm_api::comparison::Comparison>| r.map(|_| ()).map_err(|e| e.code);
    assert_eq!(code(save("One", &[1])), Err(ErrorCode::InvalidRequest));
    assert_eq!(
        code(save("Five", &[1, 2, 3, 4, 5])),
        Err(ErrorCode::InvalidRequest)
    );
    assert_eq!(code(save(" ", &[1, 2])), Err(ErrorCode::InvalidRequest));
    assert_eq!(code(save("Twice", &[1, 1])), Err(ErrorCode::InvalidRequest));
    assert_eq!(code(save("Unknown", &[1, 999])), Err(ErrorCode::NotFound));
    assert!(api.list_comparisons().unwrap().is_empty());

    let b = save("body", &[6, 1]).unwrap();
    assert_eq!(
        b.faces.iter().map(|f| f.id).collect::<Vec<_>>(),
        [FaceId(6), FaceId(1)]
    );
    assert_eq!(
        (b.mode, b.sample_text.as_str()),
        (CompareMode::Stacked, "Quartz")
    );
    let a = save(" Alpha ", &[1, 2, 6, 4]).unwrap();
    assert_eq!(a.name, "Alpha");
    let names = || -> Vec<String> {
        api.list_comparisons()
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect()
    };
    assert_eq!(names(), ["Alpha", "body"]);
    api.rename_comparison(RenameComparisonRequest {
        id: b.id,
        name: "Aa".into(),
    })
    .unwrap();
    assert_eq!(names(), ["Aa", "Alpha"]);
    assert_eq!(api.get_comparison(ComparisonRef { id: a.id }).unwrap(), a);

    // A face whose source is paused is named as missing; the rest open.
    api.set_source_paused(SetSourcePausedRequest {
        source: SourceId(1),
        paused: true,
    })
    .unwrap();
    let hidden = api.get_comparison(ComparisonRef { id: b.id }).unwrap();
    assert!(hidden.faces.is_empty());
    assert_eq!(hidden.missing.len(), 2);
    assert_eq!(hidden.missing[0].id, FaceId(6));

    api.delete_comparison(ComparisonRef { id: a.id }).unwrap();
    assert_eq!(names(), ["Aa"]);
    assert_eq!(
        api.delete_comparison(ComparisonRef { id: a.id })
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
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
fn the_mock_license_guard_blocks_preview_and_print_as_the_core_does() {
    let mock = Arc::new(MockApi::new());
    // JetBrains Mono becomes preview & print; Legacy Grotesk is restricted.
    mock.set_embedding(FaceId(6), Embedding::PreviewAndPrint)
        .unwrap();
    let api: Arc<dyn Api> = mock.clone();
    let export = |faces: &[u32]| {
        api.export_web(ExportWebRequest {
            faces: faces.iter().copied().map(FaceId).collect(),
            formats: vec![WebFormat::Woff2],
            subset: None,
            out_dir: "/tmp/out".into(),
            css: true,
        })
    };
    let e = export(&[1, 7, 6, 7]).unwrap_err();
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert_eq!(
        e.message,
        "Legacy Grotesk Regular: Embedding restricted, no web license declared; \
         JetBrains Mono Regular: Embedding preview & print only, no web license declared. \
         Declare a web license for the family to export it."
    );
    assert_eq!(e.detail.as_deref(), Some("blocked faces: 7, 6"));
    // Installable and editable faces are not blocked.
    mock.set_embedding(FaceId(3), Embedding::Editable).unwrap();
    assert_eq!(export(&[1, 3]).unwrap().files.len(), 3);

    // A Web declaration for each family lets the same export through; a
    // Desktop one does not.
    let declare = |family: u32, license_type| {
        api.declare_license(LicenseDeclaration {
            family: FamilyId(family),
            license_type: Some(license_type),
            seats: None,
            purchase_date: None,
            receipt: ReceiptChange::Keep,
        })
        .unwrap();
    };
    declare(3, LicenseType::Desktop);
    assert_eq!(export(&[6]).unwrap_err().code, ErrorCode::PermissionDenied);
    declare(3, LicenseType::Web);
    declare(4, LicenseType::Web);
    assert_eq!(export(&[1, 7, 6]).unwrap().files.len(), 4);
}

#[test]
fn export_validates_the_subset_like_the_core() {
    let api = api();
    let with = |subset: Subset| ExportWebRequest {
        faces: vec![FaceId(1)],
        formats: vec![WebFormat::Woff2],
        subset: Some(subset),
        out_dir: "/tmp/out".into(),
        css: false,
    };
    let ok = api
        .export_web(with(Subset::Presets {
            names: vec!["latin".into(), "cyrillic".into()],
        }))
        .unwrap();
    assert_eq!(ok.files, ["/tmp/out/Inter-Regular.woff2"]);
    assert!(
        api.export_web(with(Subset::Unicode {
            ranges: vec!["U+0000-00FF, U+4??".into()],
        }))
        .is_ok()
    );
    assert!(
        api.export_web(with(Subset::Text { text: "Ab".into() }))
            .is_ok()
    );

    let refused = |subset: Subset| {
        let e = api.export_web(with(subset)).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidRequest, "{e:?}");
        e.message
    };
    let m = refused(Subset::Presets {
        names: vec!["elvish".into()],
    });
    assert!(m.contains("\"elvish\"") && m.contains("latin-ext"), "{m}");
    refused(Subset::Presets { names: Vec::new() });
    let m = refused(Subset::Unicode {
        ranges: vec!["U+00FF-0000".into()],
    });
    assert!(m.contains("\"U+00FF-0000\""), "{m}");
    refused(Subset::Unicode { ranges: Vec::new() });
    refused(Subset::Text {
        text: String::new(),
    });
    let m = refused(Subset::Text {
        text: "日本".into(),
    });
    assert!(m.contains("none of the characters"), "{m}");

    // The subset is checked before the faces: a bad one on an unknown face
    // is still InvalidRequest, as in the core.
    let e = api
        .export_web(ExportWebRequest {
            faces: vec![FaceId(999)],
            ..with(Subset::Unicode {
                ranges: vec!["U+ZZ".into()],
            })
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidRequest);
    // prepare_for_web passes the subset through.
    assert_eq!(
        api.prepare_for_web(PrepareForWebRequest {
            family: FamilyId(1),
            subset: Some(Subset::Presets {
                names: vec!["elvish".into()]
            }),
            out_dir: "/tmp/web".into(),
        })
        .unwrap_err()
        .code,
        ErrorCode::InvalidRequest
    );
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
fn pairings_follow_the_rules_one_face_per_other_family() {
    use fm_api::search::PairingRequest;
    let api = api();
    let got = api
        .suggest_pairings(PairingRequest {
            face: FaceId(1),
            limit: 10,
        })
        .unwrap();
    // Inter (sans): Source Serif 4 by contrast first, through its Regular;
    // never Inter's own Italic.
    let first = &got[0];
    assert_eq!(first.face.id, FaceId(3));
    assert_eq!(
        first.reasons,
        ["contrast: sans serif with serif", "similar x-height"]
    );
    assert!((first.score - 0.5).abs() < 1e-6);
    assert!(got.iter().all(|s| s.face.family != FamilyId(1)));
    let families: Vec<FamilyId> = got.iter().map(|s| s.face.family).collect();
    assert_eq!(families, [FamilyId(2), FamilyId(3), FamilyId(4)]);
    assert!(
        got[1..]
            .iter()
            .all(|s| s.reasons == ["similar x-height"] && s.score < first.score)
    );

    let one = api
        .suggest_pairings(PairingRequest {
            face: FaceId(1),
            limit: 1,
        })
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(
        api.suggest_pairings(PairingRequest {
            face: FaceId(99),
            limit: 5
        })
        .unwrap_err()
        .code,
        ErrorCode::NotFound
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
        families: Vec::new(),
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
            families: Vec::new(),
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
fn layout_presets_start_locked_at_a_readable_width_that_follows_the_body_block() {
    let api = api();
    for p in api.layout_presets().expect("presets") {
        assert!(p.width_locked, "{}", p.name);
        assert_eq!(p.width_px, None, "{}", p.name);
        assert!(api.layout_width(p).expect("width").is_some());
    }
    let book = api.layout_presets().expect("presets").remove(0);
    // Book's body is 11 pt: 33 em at 4/3 px per point.
    let w = api
        .layout_width(book.clone())
        .expect("width")
        .expect("locked");
    assert!((w - 11.0 * 96.0 / 72.0 * 33.0).abs() < 0.01, "{w}");
    let mut chosen = book.clone();
    chosen.width_px = Some(900.0);
    assert_eq!(
        api.layout_width(chosen.clone()).expect("width"),
        Some(900.0)
    );
    chosen.width_locked = false;
    assert_eq!(api.layout_width(chosen).expect("width"), None);
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

/// E16.2: the mock answers auto size factors like the core: one per face,
/// in order, the target over the face's metric in em; an unknown face fails.
#[test]
fn size_factors_answer_each_face_in_order() {
    let api = api();
    let faces = vec![FaceId(2), FaceId(1)];
    let got = api
        .size_factors(SizeFactorRequest {
            faces: faces.clone(),
            basis: SizeBasis::XHeight,
        })
        .expect("factors");
    assert_eq!(got.iter().map(|f| f.face).collect::<Vec<_>>(), faces);
    for f in &got {
        let m = api.get_face(f.face).expect("face").metrics;
        let em = f32::from(m.x_height.expect("x-height")) / f32::from(m.units_per_em);
        assert!((f.factor - 0.5 / em).abs() < 1e-6, "{f:?}");
    }
    let cap = api
        .size_factors(SizeFactorRequest {
            faces: vec![FaceId(1)],
            basis: SizeBasis::CapHeight,
        })
        .expect("cap factors");
    assert!((cap[0].factor - 0.7 / 0.72).abs() < 1e-6, "{cap:?}");
    let err = api
        .size_factors(SizeFactorRequest {
            faces: vec![FaceId(999_999)],
            basis: SizeBasis::XHeight,
        })
        .expect_err("unknown");
    assert_eq!(err.code, ErrorCode::NotFound);
}

#[test]
fn tagging_a_family_tags_its_faces_and_the_selection_shows_mixed_tags() {
    let api = api();
    let family = api
        .list_families(ListFamiliesRequest::default())
        .unwrap()
        .items[0]
        .id;
    let members = api
        .selection_tags(SelectionTagsRequest {
            faces: vec![],
            families: vec![family],
        })
        .unwrap()
        .faces;
    assert!(
        members.len() > 1,
        "the first sample family has several styles"
    );
    let outside = (1..100).map(FaceId).find(|f| !members.contains(f)).unwrap();

    let made = api
        .tag_faces(TagFacesRequest {
            faces: vec![],
            tags: vec!["Body".into()],
            families: vec![family],
        })
        .unwrap();
    assert_eq!(made[0].face_count as usize, members.len());

    let got = api
        .selection_tags(SelectionTagsRequest {
            faces: vec![outside],
            families: vec![family],
        })
        .unwrap();
    assert_eq!(got.faces.len(), members.len() + 1);
    assert_eq!(got.tags.len(), 1);
    assert_eq!(got.tags[0].face_count as usize, members.len());
    assert!(
        !got.tags[0].all,
        "the outside face does not carry it: mixed"
    );

    let unknown = api.tag_faces(TagFacesRequest {
        faces: vec![],
        tags: vec!["Body".into()],
        families: vec![FamilyId(9999)],
    });
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
}

/// E14.1: the mock refuses the rules fm-core refuses, updates a smart
/// collection's rule, applies a draft rule to search and counts foundries.
#[test]
fn rule_editor_calls_behave_like_the_core() {
    let api = api();
    let leaf = |filters: Filters| Rule::Filters { filters };
    let empty = smart(&api, "Empty", RuleOp::And, vec![]).unwrap_err();
    assert_eq!(empty.code, ErrorCode::InvalidRequest);
    let upside_down = smart(
        &api,
        "Upside down",
        RuleOp::Or,
        vec![leaf(Filters {
            weight: Some(WeightRange { min: 900, max: 100 }),
            ..Filters::default()
        })],
    )
    .unwrap_err();
    assert_eq!(upside_down.code, ErrorCode::InvalidRequest);
    assert!(api.list_collections().unwrap().is_empty());

    let italic = Filters {
        slant: Some(Slant::Italic),
        ..Filters::default()
    };
    let c = smart(&api, "Italic", RuleOp::And, vec![leaf(italic.clone())]).unwrap();
    let draft = RuleGroup {
        op: RuleOp::Or,
        rules: vec![
            leaf(italic.clone()),
            leaf(Filters {
                variable: Some(true),
                ..Filters::default()
            }),
        ],
    };
    let found = api
        .search(SearchRequest {
            rule: Some(draft.clone()),
            page: Page {
                offset: 0,
                limit: 1000,
            },
            ..SearchRequest::default()
        })
        .unwrap();
    let draft_faces: usize = found.families.iter().map(|h| h.faces.len()).sum();
    let updated = api
        .update_collection_rules(UpdateCollectionRulesRequest {
            collection: c.id,
            rule_group: draft.clone(),
        })
        .unwrap();
    assert_eq!(updated.rule_group, Some(draft));
    assert_eq!(updated.face_count as usize, draft_faces);
    assert!(draft_faces > 0);

    let refused = api
        .update_collection_rules(UpdateCollectionRulesRequest {
            collection: c.id,
            rule_group: RuleGroup {
                op: RuleOp::And,
                rules: vec![Rule::Group {
                    group: RuleGroup {
                        op: RuleOp::Or,
                        rules: vec![],
                    },
                }],
            },
        })
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::InvalidRequest);
    assert_eq!(
        api.list_collections().unwrap()[0].rule_group,
        updated.rule_group
    );
    let manual = api
        .create_collection(CreateCollectionRequest {
            name: "Manual".into(),
            parent: None,
            rules: None,
            rule_group: None,
        })
        .unwrap();
    let not_smart = api
        .update_collection_rules(UpdateCollectionRulesRequest {
            collection: manual.id,
            rule_group: RuleGroup::from(italic),
        })
        .unwrap_err();
    assert_eq!(not_smart.code, ErrorCode::InvalidRequest);

    let foundries = api
        .search(SearchRequest::default())
        .unwrap()
        .counts
        .unwrap()
        .foundry;
    assert!(!foundries.is_empty());
    let mut sorted = foundries.clone();
    sorted.sort_by(|a, b| a.value.cmp(&b.value));
    assert_eq!(foundries, sorted);
}

#[test]
fn every_mock_language_carries_its_pangram() {
    let languages = api().list_languages().expect("languages");
    assert!(!languages.is_empty());
    for l in &languages {
        assert!(
            l.pangram.as_deref().is_some_and(|p| !p.is_empty()),
            "{}",
            l.code
        );
    }
    let sr = languages
        .iter()
        .find(|l| l.code == "sr-Cyrl")
        .expect("sr-Cyrl");
    assert!(sr.pangram.as_deref().is_some_and(|p| p.contains('џ')));
}

#[test]
fn a_language_preset_without_pangram_still_deserializes() {
    let p: fm_api::search::LanguagePreset =
        serde_json::from_str(r#"{"code":"en","name":"English","script":"Latn"}"#)
            .expect("old shape");
    assert_eq!(p.pangram, None);
}

#[test]
fn export_css_follows_the_core_rules() {
    let api = api();
    let req = |faces: Vec<u32>, subset: Option<Subset>| ExportWebRequest {
        faces: faces.into_iter().map(FaceId).collect(),
        formats: vec![WebFormat::Woff, WebFormat::Woff2],
        subset,
        out_dir: "/tmp/out".into(),
        css: true,
    };
    let css = api
        .export_web(req(vec![1, 2, 3], None))
        .unwrap()
        .css
        .unwrap();
    assert_eq!(css, MOCK_CSS_SNAPSHOT, "{css}");
    let subset = api
        .export_web(req(vec![1], Some(Subset::Text { text: "BA".into() })))
        .unwrap()
        .css
        .unwrap();
    assert!(
        subset.contains("  unicode-range: U+0020, U+0041-0042;\n"),
        "{subset}"
    );
}

/// The same blocks `fm_core::web_css` writes for these faces.
const MOCK_CSS_SNAPSHOT: &str = r#"@font-face {
  font-family: "Inter";
  font-style: normal;
  font-weight: 100 900;
  font-display: swap;
  src: url("Inter-Regular.woff2") format("woff2"), url("Inter-Regular.woff") format("woff");
}

@font-face {
  font-family: "Inter";
  font-style: italic;
  font-weight: 100 900;
  font-display: swap;
  src: url("Inter-Italic.woff2") format("woff2"), url("Inter-Italic.woff") format("woff");
}

@font-face {
  font-family: "Source Serif 4";
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url("SourceSerif4-Regular.woff2") format("woff2"), url("SourceSerif4-Regular.woff") format("woff");
}
"#;

#[allow(clippy::expect_used)] // test setup: a failure fails the test
fn snippet(api: &Arc<dyn Api>, framework: Framework, faces: Vec<FaceId>) -> String {
    api.framework_snippet(FrameworkSnippetRequest {
        faces,
        framework,
        fonts_path: None,
    })
    .expect("snippet")
    .text
}

#[test]
fn framework_snippets_follow_the_core_shape_with_the_family_class_fallback() {
    let api = api();
    let faces: Vec<FaceId> = [FamilyId(1), FamilyId(2)]
        .into_iter()
        .flat_map(|f| api.get_family(f).unwrap().faces.into_iter().map(|x| x.id))
        .collect();
    let tailwind = snippet(&api, Framework::Tailwind, faces.clone());
    assert!(
        tailwind.starts_with("// tailwind.config.js\nexport default {\n"),
        "{tailwind}"
    );
    assert!(
        tailwind.contains("        'inter': ['\"Inter\"', 'sans-serif'],\n"),
        "{tailwind}"
    );
    assert!(
        tailwind.contains("        'source-serif-4': ['\"Source Serif 4\"', 'serif'],\n"),
        "{tailwind}"
    );
    let css = snippet(&api, Framework::CssVariables, faces.clone());
    assert!(
        css.starts_with(":root {\n  --font-inter: \"Inter\", sans-serif;\n"),
        "{css}"
    );
    assert!(
        css.contains("  --font-source-serif-4: \"Source Serif 4\", serif;\n"),
        "{css}"
    );
    let next = snippet(&api, Framework::NextLocalFont, faces.clone());
    assert!(
        next.starts_with("import localFont from 'next/font/local';\n"),
        "{next}"
    );
    assert!(
        next.contains("export const fontInter = localFont({"),
        "{next}"
    );
    assert!(
        next.contains("export const fontSourceSerif4 = localFont({"),
        "{next}"
    );
    assert_eq!(
        next.matches("{ path: './fonts/").count(),
        faces.len(),
        "{next}"
    );
    assert!(
        next.contains("  variable: '--font-source-serif-4',\n  display: 'swap',\n"),
        "{next}"
    );
}

#[test]
fn a_framework_snippet_needs_a_known_face() {
    let api = api();
    let req = |faces| FrameworkSnippetRequest {
        faces,
        framework: Framework::Tailwind,
        fonts_path: None,
    };
    let empty = api.framework_snippet(req(Vec::new())).unwrap_err();
    assert_eq!(empty.code, ErrorCode::InvalidRequest);
    let unknown = api.framework_snippet(req(vec![FaceId(999)])).unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound);
}

#[test]
fn a_collection_specimen_checks_like_the_core_and_returns_the_path() {
    let api = api();
    let req = |collection, format| CollectionSpecimenRequest {
        collection,
        format,
        out_path: "/tmp/Client.pdf".into(),
    };
    let collection = |name: &str, rules| {
        api.create_collection(CreateCollectionRequest {
            rule_group: None,
            name: name.into(),
            parent: None,
            rules,
        })
        .unwrap()
        .id
    };
    let code = |r: CollectionSpecimenRequest| api.export_collection_specimen(r).unwrap_err().code;
    let manual = collection("Client", None);
    assert_eq!(
        code(req(manual, SpecimenFormat::Pdf)),
        ErrorCode::InvalidRequest,
        "an empty collection"
    );
    assert_eq!(
        code(req(CollectionId(999), SpecimenFormat::Html)),
        ErrorCode::NotFound
    );
    api.add_to_collection(CollectionFacesRequest {
        collection: manual,
        faces: vec![FaceId(1), FaceId(3)],
        families: vec![],
    })
    .unwrap();
    let res = api
        .export_collection_specimen(req(manual, SpecimenFormat::Pdf))
        .unwrap();
    assert_eq!(res.path, "/tmp/Client.pdf");
    assert_eq!(
        code(CollectionSpecimenRequest {
            out_path: " ".into(),
            ..req(manual, SpecimenFormat::Html)
        }),
        ErrorCode::InvalidRequest
    );
    // A smart collection goes through its rule.
    let smart = collection(
        "Variable",
        Some(Filters {
            variable: Some(true),
            ..Default::default()
        }),
    );
    api.export_collection_specimen(req(smart, SpecimenFormat::Html))
        .unwrap();
    // The restricted face refuses the whole collection, by name.
    api.add_to_collection(CollectionFacesRequest {
        collection: manual,
        faces: vec![FaceId(7)],
        families: vec![],
    })
    .unwrap();
    let e = api
        .export_collection_specimen(req(manual, SpecimenFormat::Html))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert!(
        e.message
            .starts_with("Legacy Grotesk Regular: Embedding restricted"),
        "{e:?}"
    );
    assert_eq!(e.detail.as_deref(), Some("blocked faces: 7"));
}

#[test]
fn a_specimen_checks_like_the_core_and_returns_the_path() {
    let api = api();
    let req = |family, format| SpecimenRequest {
        family,
        format,
        out_path: "/tmp/Inter specimen.html".into(),
    };
    let inter = api.get_face(FaceId(1)).unwrap().face.family;
    let res = api
        .export_specimen(req(inter, SpecimenFormat::Html))
        .unwrap();
    assert_eq!(res.path, "/tmp/Inter specimen.html");
    // The app prints a PDF (ADR 0013); the mock answers as the app does.
    let pdf = api
        .export_specimen(SpecimenRequest {
            out_path: "/tmp/Inter specimen.pdf".into(),
            ..req(inter, SpecimenFormat::Pdf)
        })
        .unwrap();
    assert_eq!(pdf.path, "/tmp/Inter specimen.pdf");
    let code = |r: SpecimenRequest| api.export_specimen(r).unwrap_err().code;
    assert_eq!(
        code(req(FamilyId(999), SpecimenFormat::Html)),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(SpecimenRequest {
            out_path: " ".into(),
            ..req(inter, SpecimenFormat::Html)
        }),
        ErrorCode::InvalidRequest
    );
    // The restricted face with no license declared is refused, by name.
    let legacy = api.get_face(FaceId(7)).unwrap();
    assert_eq!(legacy.license.embedding, Embedding::Restricted);
    let e = api
        .export_specimen(req(legacy.face.family, SpecimenFormat::Html))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert!(
        e.message
            .starts_with("Legacy Grotesk Regular: Embedding restricted"),
        "{e:?}"
    );
    assert_eq!(e.detail.as_deref(), Some("blocked faces: 7"));
    api.declare_license(LicenseDeclaration {
        family: legacy.face.family,
        license_type: Some(LicenseType::Desktop),
        seats: None,
        purchase_date: None,
        receipt: ReceiptChange::Keep,
    })
    .unwrap();
    api.export_specimen(req(legacy.face.family, SpecimenFormat::Html))
        .unwrap();
}

#[test]
fn catalogs_are_off_until_turned_on_and_list_from_the_refreshed_cache() {
    use fm_api::catalog::{
        CatalogId, CatalogRef, ListCatalogFamiliesRequest, SetCatalogEnabledRequest,
    };
    let api: Arc<dyn Api> = Arc::new(MockApi::new());
    let id = CatalogId::GoogleFonts;
    let catalogs = api.list_catalogs().expect("catalogs");
    let ids: Vec<CatalogId> = catalogs.iter().map(|c| c.id).collect();
    assert_eq!(ids, [CatalogId::GoogleFonts, CatalogId::Fontshare]);
    assert_eq!(catalogs[1].name, "Fontshare");
    assert!(catalogs.iter().all(|c| !c.enabled), "off by default");
    assert_eq!(catalogs[0].refreshed_at, None);
    assert_eq!(
        api.refresh_catalog(CatalogRef { id })
            .expect_err("off")
            .code,
        ErrorCode::Conflict
    );
    let list = |query: &str| {
        api.list_catalog_families(ListCatalogFamiliesRequest {
            id,
            query: query.into(),
            page: Page::default(),
        })
        .expect("families")
    };
    assert_eq!(list("").total, 0);

    api.set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    assert_eq!(list("").total, 0, "never refreshed");
    let refreshed = api.refresh_catalog(CatalogRef { id }).expect("refresh");
    assert_eq!(refreshed.family_count, 4);
    assert!(refreshed.refreshed_at.is_some());
    let lato = list("LAT");
    assert_eq!(lato.total, 1);
    assert_eq!(lato.items[0].name, "Lato");

    let off = api
        .set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: false })
        .expect("off");
    assert_eq!(off.family_count, 4, "cache kept");
    assert_eq!(list("").total, 0, "an off catalog lists nothing");
}

#[test]
fn a_mock_download_goes_into_a_watched_source_and_is_listed() {
    use fm_api::catalog::{
        CatalogFileKind, CatalogId, CatalogRef, DownloadFamilyRequest, SetCatalogEnabledRequest,
    };
    let api: Arc<dyn Api> = Arc::new(MockApi::new());
    let id = CatalogId::GoogleFonts;
    let req = |family: &str, kind, source| DownloadFamilyRequest {
        id,
        family: family.to_owned(),
        kind,
        source: SourceId(source),
    };
    let e = api
        .download_family(req("Lato", CatalogFileKind::Static, 1))
        .expect_err("off");
    assert_eq!(e.code, ErrorCode::Conflict);
    api.set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    let e = api
        .download_family(req("Lato", CatalogFileKind::Static, 1))
        .expect_err("never refreshed");
    assert_eq!(e.code, ErrorCode::NotFound);
    api.refresh_catalog(CatalogRef { id }).expect("refresh");
    let e = api
        .download_family(req("Lato", CatalogFileKind::Static, 2))
        .expect_err("the system fonts");
    assert_eq!(e.code, ErrorCode::InvalidRequest);

    let rx = api.subscribe();
    let got = api
        .download_family(req("Inter", CatalogFileKind::Variable, 1))
        .expect("download");
    assert_eq!(got.download.kind, CatalogFileKind::Variable);
    let names: Vec<&str> = got.download.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["Inter[wght].ttf"]);
    assert!(got.download.folder.ends_with("/Inter"));
    let events: Vec<Event> = rx.try_iter().collect();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::ScanFinished { job, .. } if *job == got.scan)),
        "the rescan: {events:?}"
    );
    let lato = api
        .download_family(req("Lato", CatalogFileKind::Variable, 1))
        .expect("static only");
    assert_eq!(
        lato.download.kind,
        CatalogFileKind::Static,
        "no variable font"
    );
    assert_eq!(lato.download.files.len(), 5);
    let listed = api.list_catalog_downloads().expect("downloads");
    let families: Vec<&str> = listed.iter().map(|d| d.family.as_str()).collect();
    assert_eq!(families, ["Lato", "Inter"], "newest first");
}

#[test]
fn a_catalog_font_source_needs_the_catalog_on_and_the_family_listed() {
    use fm_api::catalog::{CatalogFontRequest, CatalogId, CatalogRef, SetCatalogEnabledRequest};
    let api: Arc<dyn Api> = Arc::new(MockApi::new());
    let id = CatalogId::GoogleFonts;
    let source = |family: &str| {
        api.catalog_font_source(CatalogFontRequest {
            id,
            family: family.into(),
        })
    };
    assert_eq!(source("Lato").expect_err("off").code, ErrorCode::Conflict);
    api.set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    assert_eq!(
        source("Lato").expect_err("not refreshed").code,
        ErrorCode::NotFound
    );
    api.refresh_catalog(CatalogRef { id }).expect("refresh");
    let lato = source("Lato").expect("listed");
    assert_eq!(lato.family, "Lato");
    assert!(lato.path.ends_with(".ttf"));
    assert_eq!(
        source("Nope Sans").expect_err("unlisted").code,
        ErrorCode::NotFound
    );
}

#[test]
fn the_mock_update_check_follows_the_rules_of_the_core() {
    use fm_api::catalog::{
        CatalogId, CatalogUpdateStatus, CheckUpdatesRequest, SetCatalogEnabledRequest,
    };
    let mock = MockApi::new();
    let id = CatalogId::GoogleFonts;
    // The mock families last changed on 2026-09-01.
    mock.seed_catalog_download(id, "Lato", "2026-09-01");
    mock.seed_catalog_download(id, "Inter", "2026-03-01");
    mock.seed_catalog_download(id, "Retired Sans", "2025-01-01");
    let api: Arc<dyn Api> = Arc::new(mock);
    assert!(
        api.check_updates(CheckUpdatesRequest::default())
            .expect("off")
            .is_empty(),
        "an off catalog is skipped"
    );
    api.set_catalog_enabled(SetCatalogEnabledRequest { id, enabled: true })
        .expect("on");
    assert!(
        api.check_updates(CheckUpdatesRequest { cached_only: true })
            .expect("never refreshed")
            .is_empty()
    );
    let updates = api
        .check_updates(CheckUpdatesRequest::default())
        .expect("check");
    let found: Vec<(&str, CatalogUpdateStatus)> = updates
        .iter()
        .map(|u| (u.family.as_str(), u.status))
        .collect();
    assert_eq!(
        found,
        [
            ("Inter", CatalogUpdateStatus::UpdateAvailable),
            ("Retired Sans", CatalogUpdateStatus::NoLongerListed),
        ]
    );
    assert_eq!(updates[0].available_version.as_deref(), Some("2026-09-01"));
}
