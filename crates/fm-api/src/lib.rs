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
//!   [`ExportService`], [`WorkflowService`], [`SettingsService`]); [`Api`] is all of them plus
//!   [`EventSource`];
//! - every method takes one request value and returns [`ApiResult`];
//! - methods are synchronous; long work (a scan) returns a [`JobId`] at once
//!   and reports through [`Event`]s;
//! - [`mock::MockApi`] implements every trait on in-memory sample data.

pub mod activation;
pub mod common;
pub mod error;
pub mod event;
pub mod export;
pub mod ids;
pub mod library;
pub mod mock;
pub mod organization;
pub mod preview;
pub mod search;
pub mod settings;
pub mod workflows;

pub use activation::ActivationService;
pub use error::{ApiError, ApiResult, ErrorCode};
pub use event::{Event, EventSource};
pub use export::ExportService;
pub use ids::{CollectionId, FaceId, FamilyId, JobId, SetId, SourceId, TagId};
pub use library::LibraryService;
pub use organization::OrganizationService;
pub use preview::PreviewService;
pub use search::SearchService;
pub use settings::SettingsService;
pub use workflows::WorkflowService;

use serde::{Deserialize, Serialize};

/// The version of the API contract. Bumped on every contract change; a major
/// bump is the only place a breaking change may land.
pub const API_VERSION: ApiVersion = ApiVersion {
    major: 0,
    minor: 5,
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
        // search
        search::SearchRequest,
        search::SearchResponse,
        search::CoverageRequest,
        search::CoverageReport,
        search::LanguagePreset,
        search::SimilarRequest,
        search::SimilarFace,
        // preview
        preview::PreviewRequest,
        preview::PreviewImage,
        preview::FontSource,
        // activation
        activation::ActivateRequest,
        activation::DeactivateRequest,
        activation::ActivationOutcome,
        activation::ActiveFace,
        activation::Conflict,
        // organization
        organization::Collection,
        organization::CreateCollectionRequest,
        organization::RenameCollectionRequest,
        organization::CollectionFacesRequest,
        organization::Tag,
        organization::TagFacesRequest,
        organization::UntagFacesRequest,
        organization::UserMetaUpdate,
        organization::ProjectSet,
        organization::CreateSetRequest,
        organization::SetExport,
        organization::ImportSetResponse,
        // export
        export::ExportWebRequest,
        export::ExportWebResponse,
        export::LicenseInventoryRequest,
        export::LicenseInventory,
        // workflows
        workflows::BriefRequest,
        workflows::Shortlist,
        workflows::PrepareForWebRequest,
        workflows::PrepareForWebResponse,
        // settings
        settings::Settings,
        settings::SettingsUpdate,
        settings::AppFolders,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_displays_as_semver() {
        assert_eq!(API_VERSION.to_string(), "0.5.0");
    }

    #[test]
    fn api_version_round_trips_through_json() {
        let json = serde_json::to_string(&API_VERSION).unwrap();
        assert_eq!(json, r#"{"major":0,"minor":5,"patch":0}"#);
        let back: ApiVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, API_VERSION);
    }
}
