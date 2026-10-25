//! Version negotiation and deprecation notes (ADR 0015, "Versions").
//!
//! A client names the contract it was built against in the
//! [`VERSION_HEADER`]; [`check_client_version`] serves every client of the
//! same major and refuses any other major with one sentence that says what
//! to update. Within a major the contract only grows (ADR 0006), so an
//! older client misses nothing it knows and a newer one sends fields this
//! build ignores. A field on its way out is listed in [`deprecations`]
//! until the next major removes it; [`deprecations_in`] finds the ones an
//! answer contains, so the endpoint can name them in [`DEPRECATION_HEADER`].
//!
//! The endpoint, the CLI and the MCP server all negotiate through these
//! functions, so they follow the same version.

use std::collections::BTreeSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{API_VERSION, ApiError, ApiResult, ApiVersion, ErrorCode};

/// The header a client sends with the contract it speaks and every answer
/// carries with the contract this build serves (`major.minor.patch`).
pub const VERSION_HEADER: &str = "Fm-Api-Version";

/// The header an answer carries once per deprecated field it contains.
pub const DEPRECATION_HEADER: &str = "Fm-Deprecation";

/// The only major [`check_client_version`] serves. Kept apart from
/// [`API_VERSION`] on purpose: a major bump is a breaking change, so whoever
/// makes it must come here, read the rules in ADR 0015 and move this too. A
/// test fails until they do.
pub const NEGOTIATED_MAJOR: u32 = 0;

/// What `GET /v1/version` answers: enough for a client to decide whether it
/// can talk to this app before it holds a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// The app version (semver).
    pub app_version: String,
    /// The API contract this build serves.
    pub api_version: ApiVersion,
    /// The fields on their way out, oldest first.
    pub deprecations: Vec<Deprecation>,
}

/// A field that still works but will be removed in the next major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Deprecation {
    /// The type that holds the field, by its Rust (and TypeScript) name,
    /// e.g. `AppInfo`.
    pub type_name: String,
    /// The field by its wire name (camelCase), e.g. `platformNotes`.
    pub field: String,
    /// The contract version that deprecated it.
    pub since: ApiVersion,
    /// One plain sentence: what to use instead.
    pub note: String,
}

impl Deprecation {
    /// The value of one [`DEPRECATION_HEADER`]:
    /// `<Type>.<field> (since <version>): <note>`.
    pub fn header_value(&self) -> String {
        format!(
            "{}.{} (since {}): {}",
            self.type_name, self.field, self.since, self.note
        )
    }
}

/// The deprecated fields of this contract. Empty today: nothing has been
/// deprecated since the contract started.
pub fn deprecations() -> Vec<Deprecation> {
    Vec::new()
}

/// The entries of `list` that the answer `json` of type `T` contains: the
/// entry's type is `T` or reachable from it, and its field is a key of an
/// object somewhere in `json` (a field the answer leaves out is not sent,
/// so it is not named).
pub fn deprecations_in<'a, T: specta::Type>(
    list: &'a [Deprecation],
    json: &serde_json::Value,
) -> Vec<&'a Deprecation> {
    if list.is_empty() {
        return Vec::new();
    }
    let types = specta::Types::default().register::<T>();
    let reachable: BTreeSet<&str> = types.into_unsorted_iter().map(|t| &*t.name).collect();
    list.iter()
        .filter(|d| reachable.contains(d.type_name.as_str()) && has_key(json, &d.field))
        .collect()
}

fn has_key(json: &serde_json::Value, key: &str) -> bool {
    match json {
        serde_json::Value::Object(map) => {
            map.contains_key(key) || map.values().any(|v| has_key(v, key))
        }
        serde_json::Value::Array(items) => items.iter().any(|v| has_key(v, key)),
        _ => false,
    }
}

/// Serves a client that speaks `client`, or refuses it with
/// [`ErrorCode::Unsupported`] and one sentence naming both versions and
/// what to update. Any minor and patch of the same major is served: an
/// older client asks for less, and the fields a newer one adds are ignored.
pub fn check_client_version(client: ApiVersion) -> ApiResult<()> {
    negotiate(NEGOTIATED_MAJOR, API_VERSION, client)
}

/// The client side of [`check_client_version`]: a client built against
/// [`API_VERSION`] talks to a server that serves `served` (its
/// [`VERSION_HEADER`]), or refuses it with [`ErrorCode::Unsupported`] and the
/// same sentence the server would send. So a client meets a major mismatch
/// plainly even when the server does not check the header.
pub fn check_server_version(served: ApiVersion) -> ApiResult<()> {
    negotiate(served.major, served, API_VERSION)
}

/// [`check_client_version`] for a build that serves `major` as `served`.
fn negotiate(major: u32, served: ApiVersion, client: ApiVersion) -> ApiResult<()> {
    if client.major == major {
        return Ok(());
    }
    let update = if client.major > major {
        format!(
            "update Font Manager to a version that serves API {}.x",
            client.major
        )
    } else {
        format!("update the client to a version built for API {major}.x")
    };
    Err(ApiError::new(
        ErrorCode::Unsupported,
        format!("The client speaks API {client} but Font Manager serves API {served}: {update}."),
    ))
}

/// Why a version string could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseVersionError(String);

impl std::fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} is not a version of the form major.minor.patch",
            self.0
        )
    }
}

impl std::error::Error for ParseVersionError {}

impl FromStr for ApiVersion {
    type Err = ParseVersionError;

    /// `major.minor.patch`, three decimal numbers; spaces around it are
    /// allowed, nothing else.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = || ParseVersionError(s.to_owned());
        let mut parts = s.trim().split('.');
        let mut next = || -> Result<u32, ParseVersionError> {
            let part = parts.next().ok_or_else(error)?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(error());
            }
            part.parse().map_err(|_| error())
        };
        let version = ApiVersion {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return Err(error());
        }
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Paged;
    use crate::library::{AppInfo, FamilySummary};

    fn v(major: u32, minor: u32, patch: u32) -> ApiVersion {
        ApiVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn negotiated_major_follows_api_version() {
        assert_eq!(
            API_VERSION.major, NEGOTIATED_MAJOR,
            "API_VERSION's major changed: update NEGOTIATED_MAJOR and the rules in ADR 0015"
        );
        assert!(check_client_version(API_VERSION).is_ok());
    }

    #[test]
    fn versions_parse_from_major_minor_patch() {
        assert_eq!("0.22.0".parse(), Ok(v(0, 22, 0)));
        assert_eq!(" 1.2.3 ".parse(), Ok(v(1, 2, 3)));
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1..3",
            "a.b.c",
            "1.2.-3",
            "+1.2.3",
            "1.2.3-rc1",
        ] {
            assert!(bad.parse::<ApiVersion>().is_err(), "{bad:?}");
        }
        assert_eq!(
            "x".parse::<ApiVersion>().unwrap_err().to_string(),
            r#""x" is not a version of the form major.minor.patch"#
        );
    }

    #[test]
    fn the_same_major_is_served_at_any_minor_or_patch() {
        let major = NEGOTIATED_MAJOR;
        for client in [
            v(major, 0, 0),
            v(major, API_VERSION.minor + 5, 9),
            API_VERSION,
        ] {
            assert!(check_client_version(client).is_ok(), "{client}");
        }
    }

    #[test]
    fn a_newer_major_is_told_to_update_the_app() {
        let client = v(NEGOTIATED_MAJOR + 1, 0, 0);
        let e = check_client_version(client).unwrap_err();
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert_eq!(
            e.message,
            format!(
                "The client speaks API {client} but Font Manager serves API {API_VERSION}: \
                 update Font Manager to a version that serves API {}.x.",
                client.major
            )
        );
    }

    #[test]
    fn an_older_major_is_told_to_update_the_client() {
        // NEGOTIATED_MAJOR is 0 today, so no client can be older yet: check
        // the rule as a build that serves 2.1.0 would apply it.
        let client = v(1, 9, 0);
        let e = negotiate(2, v(2, 1, 0), client).unwrap_err();
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert_eq!(
            e.message,
            "The client speaks API 1.9.0 but Font Manager serves API 2.1.0: update the client to a version built for API 2.x."
        );
        assert!(negotiate(2, v(2, 1, 0), v(2, 0, 7)).is_ok());
    }

    #[test]
    fn a_client_checks_the_server_by_the_same_rule() {
        assert!(check_server_version(API_VERSION).is_ok());
        assert!(check_server_version(v(API_VERSION.major, API_VERSION.minor + 3, 1)).is_ok());
        let newer = v(API_VERSION.major + 1, 0, 0);
        let e = check_server_version(newer).unwrap_err();
        assert_eq!(e.code, ErrorCode::Unsupported);
        assert_eq!(
            e.message,
            format!(
                "The client speaks API {API_VERSION} but Font Manager serves API {newer}: update the client to a version built for API {}.x.",
                newer.major
            )
        );
    }

    #[test]
    fn deprecations_is_empty_today() {
        assert!(deprecations().is_empty());
    }

    #[test]
    fn every_deprecation_fits_in_a_header_and_names_a_contract_type() {
        // A header value is visible ASCII and spaces; a note outside that
        // would be dropped from the answer.
        let names: BTreeSet<String> = crate::types()
            .into_unsorted_iter()
            .map(|t| t.name.to_string())
            .collect();
        for d in deprecations() {
            let value = d.header_value();
            assert!(
                value.bytes().all(|b| b == b' ' || b.is_ascii_graphic()),
                "{value}"
            );
            assert!(names.contains(&d.type_name), "{}", d.type_name);
            // The next major removes it, so it cannot be older than this one.
            assert_eq!(d.since.major, API_VERSION.major, "{value}");
        }
    }

    fn entry(type_name: &str, field: &str) -> Deprecation {
        Deprecation {
            type_name: type_name.into(),
            field: field.into(),
            since: v(0, 22, 0),
            note: "Use something else.".into(),
        }
    }

    #[test]
    fn a_deprecated_field_is_found_in_its_type_and_in_types_that_hold_it() {
        let list = [entry("AppInfo", "platformNotes")];
        let json = serde_json::json!({ "name": "x", "platformNotes": [] });
        assert_eq!(deprecations_in::<AppInfo>(&list, &json), vec![&list[0]]);
        // Nested: a page of summaries reaches FamilySummary, not AppInfo.
        let list = [entry("FamilySummary", "faceCount")];
        let page = serde_json::json!({ "items": [{ "faceCount": 2 }], "total": 1 });
        assert_eq!(
            deprecations_in::<Paged<FamilySummary>>(&list, &page),
            vec![&list[0]]
        );
    }

    #[test]
    fn a_field_of_another_type_or_left_out_is_not_named() {
        let list = [entry("AppInfo", "platformNotes")];
        // Same key, but the answer's type cannot hold an AppInfo.
        let json = serde_json::json!({ "platformNotes": [] });
        assert!(deprecations_in::<Paged<FamilySummary>>(&list, &json).is_empty());
        // The right type, but the field is not in the answer.
        let json = serde_json::json!({ "name": "x" });
        assert!(deprecations_in::<AppInfo>(&list, &json).is_empty());
    }

    #[test]
    fn the_header_value_names_the_field_and_its_note() {
        assert_eq!(
            entry("AppInfo", "platformNotes").header_value(),
            "AppInfo.platformNotes (since 0.22.0): Use something else."
        );
    }
}
