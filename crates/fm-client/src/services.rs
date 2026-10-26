//! Every `fm-api` service trait on [`Client`]: each method is one
//! `POST /v1/<method>` (the endpoint's route table, ADR 0015). The method
//! lists mirror the traits, so the compiler refuses a trait method left
//! out; the conformance test calls every route through them.

use std::sync::mpsc::Receiver;

use fm_api::activation::*;
use fm_api::catalog::*;
use fm_api::common::Paged;
use fm_api::comparison::*;
use fm_api::export::*;
use fm_api::layout::*;
use fm_api::library::*;
use fm_api::organization::*;
use fm_api::preview::*;
use fm_api::search::*;
use fm_api::settings::*;
use fm_api::workflows::*;
use fm_api::{
    ApiResult, CollectionId, Event, EventSource, FaceId, FamilyId, JobId, ResolutionId, SetId,
    SourceId, VirtualStyleId,
};

use crate::Client;

/// `impl <Trait> for Client` with one call per method; `()` is the body of
/// a method without a request.
macro_rules! service {
    ($trait:ident { $($name:ident($($arg:ident: $req:ty)?) -> $res:ty;)* }) => {
        impl $trait for Client {
            $(
                fn $name(&self $(, $arg: $req)?) -> ApiResult<$res> {
                    self.call(stringify!($name), &service!(@body $($arg)?))
                }
            )*
        }
    };
    (@body) => { () };
    (@body $arg:ident) => { $arg };
}

service!(LibraryService {
    app_info() -> AppInfo;
    list_sources() -> Vec<Source>;
    add_source(req: AddSourceRequest) -> Source;
    remove_source(source: SourceId) -> ();
    set_source_paused(req: SetSourcePausedRequest) -> Source;
    scan(req: ScanRequest) -> JobId;
    rebuild_index() -> JobId;
    list_families(req: ListFamiliesRequest) -> Paged<FamilySummary>;
    get_family(family: FamilyId) -> Family;
    get_face(face: FaceId) -> FaceDetail;
    list_problems() -> Vec<Problem>;
    list_duplicates() -> Vec<DuplicateGroup>;
    keep_copy(req: KeepCopyRequest) -> Resolution;
    list_resolutions() -> Vec<Resolution>;
    undo_resolution(resolution: ResolutionId) -> Resolution;
    list_sidecar_conflicts() -> Vec<SidecarConflict>;
    resolve_sidecar_conflict(req: ResolveSidecarConflictRequest) -> ();
    list_glyphs(req: GlyphsRequest) -> GlyphPage;
    get_glyph(req: GlyphRequest) -> GlyphDetail;
    resolve_font(req: ResolveFontRequest) -> FontMatches;
});

service!(SearchService {
    search(req: SearchRequest) -> SearchResponse;
    check_coverage(req: CoverageRequest) -> CoverageReport;
    face_coverage(face: FaceId) -> FaceCoverage;
    list_languages() -> Vec<LanguagePreset>;
    find_similar(req: SimilarRequest) -> Vec<SimilarFace>;
    suggest_pairings(req: PairingRequest) -> Vec<PairingSuggestion>;
});

service!(PreviewService {
    render_preview(req: PreviewRequest) -> PreviewImage;
    compare_preview(req: ComparePreviewRequest) -> PreviewImage;
    font_source(face: FaceId) -> FontSource;
    sample_texts(req: SampleTextRequest) -> Vec<SampleText>;
    css_values(req: CssValuesRequest) -> CssValues;
    size_factors(req: SizeFactorRequest) -> Vec<SizeFactor>;
});

service!(ActivationService {
    activate(req: ActivateRequest) -> Vec<ActivationOutcome>;
    deactivate(req: DeactivateRequest) -> Vec<ActivationOutcome>;
    list_active() -> Vec<ActiveFace>;
    conflicts(target: Target) -> Vec<Conflict>;
    list_conflicts() -> Vec<Clash>;
    fonts_needed(req: FontsNeededRequest) -> FontsNeededResponse;
});

service!(OrganizationService {
    list_collections() -> Vec<Collection>;
    create_collection(req: CreateCollectionRequest) -> Collection;
    rename_collection(req: RenameCollectionRequest) -> Collection;
    move_collection(req: MoveCollectionRequest) -> Collection;
    update_collection_rules(req: UpdateCollectionRulesRequest) -> Collection;
    delete_collection(collection: CollectionId) -> ();
    add_to_collection(req: CollectionFacesRequest) -> Collection;
    remove_from_collection(req: CollectionFacesRequest) -> Collection;
    list_tags() -> Vec<Tag>;
    tag_faces(req: TagFacesRequest) -> Vec<Tag>;
    untag_faces(req: UntagFacesRequest) -> ();
    selection_tags(req: SelectionTagsRequest) -> SelectionTags;
    update_user_meta(req: UserMetaUpdate) -> ();
    list_sets() -> Vec<ProjectSet>;
    create_set(req: CreateSetRequest) -> ProjectSet;
    delete_set(set: SetId) -> ();
    export_set(set: SetId) -> SetExport;
    import_set(export: SetExport) -> ImportSetResponse;
    save_set_file(req: SaveSetFileRequest) -> SavedSetFile;
    load_set_file(req: LoadSetFileRequest) -> ImportSetResponse;
    pin_instance(req: PinInstanceRequest) -> VirtualStyle;
    list_virtual_styles(req: VirtualStylesRequest) -> Vec<VirtualStyle>;
    delete_virtual_style(style: VirtualStyleId) -> ();
    update_family_notes(req: FamilyNotesUpdate) -> ();
    declare_license(req: LicenseDeclaration) -> ();
    quick_collection() -> QuickCollection;
    add_to_quick_collection(req: QuickCollectionRequest) -> QuickCollection;
    remove_from_quick_collection(req: QuickCollectionRequest) -> QuickCollection;
    clear_quick_collection() -> ();
});

service!(ExportService {
    export_web(req: ExportWebRequest) -> ExportWebResponse;
    framework_snippet(req: FrameworkSnippetRequest) -> FrameworkSnippet;
    license_inventory(req: LicenseInventoryRequest) -> LicenseInventory;
    export_glyph_svg(req: GlyphSvgRequest) -> String;
    export_specimen(req: SpecimenRequest) -> SpecimenResponse;
    export_collection_specimen(req: CollectionSpecimenRequest) -> SpecimenResponse;
});

service!(WorkflowService {
    pick_fonts_for_brief(req: BriefRequest) -> Shortlist;
    prepare_for_web(req: PrepareForWebRequest) -> PrepareForWebResponse;
});

service!(LayoutService {
    layout_presets() -> Vec<Layout>;
    clamp_layout(layout: Layout) -> Layout;
    layout_bounds() -> LayoutBounds;
    list_layouts() -> Vec<LayoutEntry>;
    get_layout(req: LayoutRef) -> SavedLayout;
    save_layout(req: SaveLayoutRequest) -> SavedLayout;
    duplicate_layout(req: DuplicateLayoutRequest) -> SavedLayout;
    revert_layout(req: RevertLayoutRequest) -> SavedLayout;
    layout_history(req: LayoutRef) -> Vec<LayoutVersion>;
    restore_layout(req: RestoreLayoutRequest) -> SavedLayout;
    layout_width(layout: Layout) -> Option<f32>;
});

service!(ComparisonService {
    list_comparisons() -> Vec<ComparisonEntry>;
    get_comparison(req: ComparisonRef) -> Comparison;
    save_comparison(req: SaveComparisonRequest) -> Comparison;
    rename_comparison(req: RenameComparisonRequest) -> ComparisonEntry;
    delete_comparison(req: ComparisonRef) -> ();
});

service!(CatalogService {
    list_catalogs() -> Vec<Catalog>;
    set_catalog_enabled(req: SetCatalogEnabledRequest) -> Catalog;
    refresh_catalog(req: CatalogRef) -> Catalog;
    list_catalog_families(req: ListCatalogFamiliesRequest) -> Paged<CatalogFamily>;
    catalog_font_source(req: CatalogFontRequest) -> CatalogFontSource;
    download_family(req: DownloadFamilyRequest) -> DownloadFamilyResponse;
    list_catalog_downloads() -> Vec<CatalogDownload>;
    check_updates(req: CheckUpdatesRequest) -> Vec<CatalogUpdate>;
});

/// Not through the macro: a token this client regenerates replaces its own,
/// so the next call still gets in.
impl SettingsService for Client {
    fn get_settings(&self) -> ApiResult<Settings> {
        self.call("get_settings", &())
    }

    fn update_settings(&self, req: SettingsUpdate) -> ApiResult<Settings> {
        self.call("update_settings", &req)
    }

    fn app_folders(&self) -> ApiResult<AppFolders> {
        self.call("app_folders", &())
    }

    fn endpoint_info(&self) -> ApiResult<EndpointInfo> {
        self.call("endpoint_info", &())
    }

    fn regenerate_endpoint_token(&self) -> ApiResult<EndpointInfo> {
        let info: EndpointInfo = self.call("regenerate_endpoint_token", &())?;
        self.set_token(&info.token);
        Ok(info)
    }

    fn mcp_status(&self) -> ApiResult<McpStatus> {
        self.call("mcp_status", &())
    }

    fn set_mcp_enabled(&self, req: McpUpdate) -> ApiResult<McpStatus> {
        self.call("set_mcp_enabled", &req)
    }

    fn set_mcp_grant(&self, req: McpGrantUpdate) -> ApiResult<McpStatus> {
        self.call("set_mcp_grant", &req)
    }

    /// Waits as long as the app waits for the user's answer: the call has
    /// no read timeout.
    fn authorize_mcp_tool(&self, req: McpToolCall) -> ApiResult<()> {
        self.call("authorize_mcp_tool", &req)
    }

    fn answer_mcp_confirmation(&self, req: McpConfirmationAnswer) -> ApiResult<()> {
        self.call("answer_mcp_confirmation", &req)
    }
}

impl EventSource for Client {
    fn subscribe(&self) -> Receiver<Event> {
        crate::events::subscribe(self)
    }
}
