//! Font Manager application API.
//!
//! The only entrance to `fm-core`: typed, versioned request/response types and
//! service traits. The desktop UI, `fm-cli`, `fm-mcp` and the plugin endpoint
//! are thin adapters over this crate. TypeScript types for the UI are generated
//! from it with `pnpm gen:types`.
//!
//! From v1.0 this is a public contract: add fields as `Option<T>` or with
//! `#[serde(default)]`; never remove or rename one.

use serde::{Deserialize, Serialize};

/// The version of the API contract. Bumped on every contract change; a major
/// bump is the only place a breaking change may land.
pub const API_VERSION: ApiVersion = ApiVersion {
    major: 0,
    minor: 1,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_displays_as_semver() {
        assert_eq!(API_VERSION.to_string(), "0.1.0");
    }

    #[test]
    fn api_version_round_trips_through_json() {
        let json = serde_json::to_string(&API_VERSION).unwrap();
        assert_eq!(json, r#"{"major":0,"minor":1,"patch":0}"#);
        let back: ApiVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, API_VERSION);
    }
}
