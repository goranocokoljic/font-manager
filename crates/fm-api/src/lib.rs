//! Font Manager application API.
//!
//! The only entrance to `fm-core`: typed, versioned request/response types and
//! service traits. The desktop UI, `fm-cli`, `fm-mcp` and the plugin endpoint
//! are thin adapters over this crate. TypeScript types for the UI are generated
//! from it with `pnpm gen:types`.
//!
//! From v1.0 this is a public contract: add fields as `Option<T>` or with
//! `#[serde(default)]`; never remove or rename one.
//!
//! Shape of the contract (ADR 0006):
//! - one trait per service ([`LibraryService`], [`SearchService`],
//!   [`PreviewService`], [`ActivationService`], [`OrganizationService`],
//!   [`ExportService`], [`WorkflowService`], [`SettingsService`],
//!   [`LayoutService`], [`ComparisonService`], [`CatalogService`]); [`Api`]
//!   is all of them plus
//!   [`EventSource`];
//! - every method takes one request value and returns [`ApiResult`];
//! - methods are synchronous; long work (a scan) returns a [`JobId`] at once
//!   and reports through [`Event`]s;
//! - [`mock::MockApi`] implements every trait on in-memory sample data.

pub mod activation;
pub mod catalog;
pub mod common;
pub mod comparison;
pub mod error;
pub mod event;
pub mod export;
pub mod ids;
pub mod layout;
pub mod library;
pub mod mock;
pub mod organization;
pub mod preview;
pub mod search;
pub mod settings;
pub mod update;
pub mod version;
pub mod workflows;

pub use activation::ActivationService;
pub use catalog::CatalogService;
pub use comparison::ComparisonService;
pub use error::{ApiError, ApiResult, ErrorCode};
pub use event::{Event, EventSource};
pub use export::ExportService;
pub use ids::{
    CollectionId, ComparisonId, FaceId, FamilyId, JobId, ResolutionId, SetId, SidecarConflictId,
    SourceId, TagId, VirtualStyleId,
};
pub use layout::LayoutService;
pub use library::LibraryService;
pub use organization::OrganizationService;
pub use preview::PreviewService;
pub use search::SearchService;
pub use settings::SettingsService;
pub use version::{
    DEPRECATION_HEADER, Deprecation, NEGOTIATED_MAJOR, VERSION_HEADER, VersionInfo,
    check_client_version, check_server_version, deprecations, deprecations_in,
};
pub use workflows::WorkflowService;

use serde::{Deserialize, Serialize};

/// The version of the API contract. Bumped on every contract change; a major
/// bump is the only place a breaking change may land, and it must move
/// [`NEGOTIATED_MAJOR`] too (ADR 0015).
pub const API_VERSION: ApiVersion = ApiVersion {
    major: 0,
    minor: 23,
    patch: 0,
};

/// A semantic version of the API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ApiVersion {
    /// Incompatible contract changes.
    pub major: u32,
    /// Additive changes.
    pub minor: u32,
    /// Fixes with no contract change.
    pub patch: u32,
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Everything an adapter needs: every service plus the event stream.
///
/// Implemented automatically for any type that implements all of them, so an
/// adapter can hold `Arc<dyn Api>` and not care whether it is the core or the mock.
pub trait Api:
    LibraryService
    + SearchService
    + PreviewService
    + ActivationService
    + OrganizationService
    + ExportService
    + WorkflowService
    + SettingsService
    + LayoutService
    + ComparisonService
    + CatalogService
    + EventSource
    + Send
    + Sync
{
}

impl<T> Api for T where
    T: LibraryService
        + SearchService
        + PreviewService
        + ActivationService
        + OrganizationService
        + ExportService
        + WorkflowService
        + SettingsService
        + LayoutService
        + ComparisonService
        + CatalogService
        + EventSource
        + Send
        + Sync
{
}

/// Every type that crosses the API, registered for TypeScript generation.
///
/// A new request, response or event type must be added here, or the UI will
/// not see it (types only reached through another type are picked up anyway).
pub fn types() -> specta::Types {
    macro_rules! register {
        ($($t:ty),* $(,)?) => { specta::Types::default()$(.register::<$t>())* };
    }
    register!(
        ApiVersion,
        ApiError,
        version::VersionInfo,
        version::Deprecation,
        Event,
        // library
        library::AppInfo,
        library::Source,
        library::AddSourceRequest,
        library::SetSourcePausedRequest,
        library::ScanRequest,
        library::ListFamiliesRequest,
        common::Paged<library::FamilySummary>,
        library::FamilySummary,
        library::Family,
        library::FaceDetail,
        library::Problem,
        library::DuplicateGroup,
        library::KeepCopyRequest,
        library::Resolution,
        library::SidecarConflict,
        library::SidecarField,
        library::ConflictSide,
        library::ResolveSidecarConflictRequest,
        library::GlyphsRequest,
        library::GlyphPage,
        library::GlyphInfo,
        library::GlyphBlock,
        library::GlyphRequest,
        library::GlyphDetail,
        library::ResolveFontRequest,
        library::FontMatches,
        // search
        search::SearchRequest,
        search::SearchResponse,
        search::CoverageRequest,
        search::CoverageReport,
        search::BlockCoverage,
        search::FaceLanguageCoverage,
        search::FaceCoverage,
        search::LanguagePreset,
        search::SimilarRequest,
        search::SimilarFace,
        search::PairingRequest,
        search::PairingSuggestion,
        // preview
        preview::PreviewRequest,
        preview::PreviewImage,
        preview::ComparePreviewRequest,
        preview::FontSource,
        preview::SampleTextRequest,
        preview::SampleText,
        preview::CssValuesRequest,
        preview::CssValues,
        preview::SizeBasis,
        preview::SizeFactorRequest,
        preview::SizeFactor,
        // activation
        activation::ActivateRequest,
        activation::DeactivateRequest,
        activation::ActivationOutcome,
        activation::ActiveFace,
        activation::Conflict,
        activation::Clash,
        activation::FontsNeededRequest,
        activation::FontsNeededResponse,
        activation::DocumentClosedRequest,
        activation::DocumentClosedResponse,
        // organization
        organization::Collection,
        organization::CreateCollectionRequest,
        organization::RenameCollectionRequest,
        organization::MoveCollectionRequest,
        organization::UpdateCollectionRulesRequest,
        organization::CollectionFacesRequest,
        organization::Tag,
        organization::TagFacesRequest,
        organization::UntagFacesRequest,
        organization::SelectionTagsRequest,
        organization::SelectionTags,
        organization::SelectionTag,
        organization::UserMetaUpdate,
        organization::FamilyNotesUpdate,
        organization::LicenseDeclaration,
        organization::ReceiptChange,
        organization::ReceiptPlace,
        organization::ProjectSet,
        organization::CreateSetRequest,
        organization::SetExport,
        organization::ImportSetResponse,
        organization::SaveSetFileRequest,
        organization::SavedSetFile,
        organization::LoadSetFileRequest,
        organization::PinInstanceRequest,
        organization::VirtualStyle,
        organization::VirtualStylesRequest,
        organization::QuickCollection,
        organization::QuickCollectionRequest,
        // export
        export::ExportWebRequest,
        export::ExportWebResponse,
        export::LicenseInventoryRequest,
        export::LicenseInventory,
        export::GlyphSvgRequest,
        export::Framework,
        export::FrameworkSnippetRequest,
        export::FrameworkSnippet,
        export::SpecimenFormat,
        export::SpecimenRequest,
        export::CollectionSpecimenRequest,
        export::SpecimenResponse,
        // workflows
        workflows::BriefRequest,
        workflows::Shortlist,
        workflows::PrepareForWebRequest,
        workflows::PrepareForWebResponse,
        workflows::DocumentFontsRequest,
        workflows::DocumentFonts,
        // settings
        settings::Settings,
        settings::SettingsUpdate,
        settings::AppFolders,
        settings::EndpointInfo,
        settings::McpClient,
        settings::McpSnippet,
        settings::McpStatus,
        settings::McpUpdate,
        settings::McpGrant,
        settings::McpGrantUpdate,
        settings::McpToolCall,
        settings::McpConfirmationAnswer,
        // updates
        update::UpdateKind,
        update::AvailableUpdate,
        update::UpdateCheck,
        update::UpdateSupport,
        update::InstallUpdateRequest,
        // layout
        layout::Layout,
        layout::TextBlock,
        layout::LayoutPreset,
        layout::BlockAlign,
        layout::BlockOverflow,
        layout::BlockRange,
        layout::ColumnRange,
        layout::LayoutBounds,
        layout::LayoutEntry,
        layout::SavedLayout,
        layout::LayoutRef,
        layout::SaveLayoutRequest,
        layout::DuplicateLayoutRequest,
        layout::RevertLayoutRequest,
        layout::LayoutVersion,
        layout::RestoreLayoutRequest,
        // comparisons
        comparison::CompareMode,
        comparison::ComparisonEntry,
        comparison::MissingFace,
        comparison::Comparison,
        comparison::SaveComparisonRequest,
        comparison::ComparisonRef,
        comparison::RenameComparisonRequest,
        // catalogs
        catalog::CatalogId,
        catalog::Catalog,
        catalog::CatalogStyle,
        catalog::CatalogFamily,
        catalog::SetCatalogEnabledRequest,
        catalog::CatalogRef,
        catalog::ListCatalogFamiliesRequest,
        catalog::CatalogFontRequest,
        catalog::CatalogFontSource,
        catalog::CatalogFileKind,
        catalog::DownloadFamilyRequest,
        catalog::CatalogDownloadFile,
        catalog::CatalogDownload,
        catalog::DownloadFamilyResponse,
        catalog::CatalogUpdateStatus,
        catalog::CatalogUpdate,
        catalog::CheckUpdatesRequest,
        common::Paged<catalog::CatalogFamily>,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_displays_as_semver() {
        assert_eq!(API_VERSION.to_string(), "0.23.0");
    }

    #[test]
    fn api_version_round_trips_through_json() {
        let json = serde_json::to_string(&API_VERSION).unwrap();
        assert_eq!(json, r#"{"major":0,"minor":23,"patch":0}"#);
        let back: ApiVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, API_VERSION);
    }
}
