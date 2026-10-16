//! The error that crosses the API. Errors are structured data, never a bare
//! string: a stable code for programs, a message for people, a detail for
//! developers. `fm-core`'s own error type (E1.7) maps into this one.

use serde::{Deserialize, Serialize};

/// The result of every service method.
pub type ApiResult<T> = Result<T, ApiError>;

/// What went wrong, in a form a program can branch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The id or path does not exist.
    NotFound,
    /// The request is malformed or out of range.
    InvalidRequest,
    /// The request clashes with current state (a name already in use).
    Conflict,
    /// The action is not allowed (a license forbids export, a grant is missing).
    PermissionDenied,
    /// The font or the platform does not support it (Type 1 activation).
    Unsupported,
    /// Another job holds the resource; try again later.
    Busy,
    /// A file or OS call failed.
    Io,
    /// The user or a caller cancelled the work.
    Cancelled,
    /// A bug. The detail says where.
    Internal,
}

/// A failed request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Stable code for programs.
    pub code: ErrorCode,
    /// One sentence for the user, in plain words.
    pub message: String,
    /// Developer detail (the underlying error, a path); not shown by default.
    #[serde(default)]
    pub detail: Option<String>,
}

impl ApiError {
    /// An error with a code and a user-facing message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// Adds the developer detail.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// `what` (for example "face 12") does not exist.
    pub fn not_found(what: impl std::fmt::Display) -> Self {
        Self::new(ErrorCode::NotFound, format!("{what} was not found."))
    }

    /// The request is not valid; `why` says what to change.
    pub fn invalid(why: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, why)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_structured_data() {
        let e = ApiError::not_found("face 7").with_detail("no row 7 in face");
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"code":"notFound","message":"face 7 was not found.","detail":"no row 7 in face"}"#
        );
        assert_eq!(e.to_string(), "face 7 was not found. (no row 7 in face)");
    }

    #[test]
    fn detail_is_optional_on_the_wire() {
        let e: ApiError =
            serde_json::from_str(r#"{"code":"busy","message":"A scan is running."}"#).unwrap();
        assert_eq!(e, ApiError::new(ErrorCode::Busy, "A scan is running."));
    }
}
