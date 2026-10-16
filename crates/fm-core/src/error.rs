//! The one error type of the core.
//!
//! Every fallible core function returns [`FmResult`]. An [`FmError`] carries a
//! stable [`ErrorCode`], one plain sentence for the user and, on the side, the
//! developer detail (the underlying error, a path). It converts losslessly into
//! [`fm_api::ApiError`], so errors cross the API as structured data and never
//! as a bare string.

use std::fmt;
use std::io;
use std::path::Path;

use fm_api::{ApiError, ErrorCode};

/// The result of every fallible core function.
pub type FmResult<T> = Result<T, FmError>;

/// A failure in the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmError {
    code: ErrorCode,
    message: String,
    detail: Option<String>,
}

impl FmError {
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
    pub fn not_found(what: impl fmt::Display) -> Self {
        Self::new(ErrorCode::NotFound, format!("{what} was not found."))
    }

    /// The request is not valid; `why` says what to change.
    pub fn invalid(why: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, why)
    }

    /// The font or the platform does not support the action.
    pub fn unsupported(why: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, why)
    }

    /// A bug. `detail` says where; the user sees a generic sentence.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::Internal,
            "Something went wrong inside Font Manager.",
        )
        .with_detail(detail)
    }

    /// A file or OS call on `path` failed. `message` is the sentence for the
    /// user; the path and the OS error go into the detail. A missing file maps
    /// to [`ErrorCode::NotFound`] and a refused one to
    /// [`ErrorCode::PermissionDenied`], so a program can tell them apart.
    pub fn io(message: impl Into<String>, path: &Path, err: &io::Error) -> Self {
        Self::new(io_code(err), message).with_detail(format!("{}: {err}", path.display()))
    }

    /// The stable code for programs.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// The sentence for the user.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The developer detail, if any.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

fn io_code(err: &io::Error) -> ErrorCode {
    match err.kind() {
        io::ErrorKind::NotFound => ErrorCode::NotFound,
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            ErrorCode::PermissionDenied
        }
        io::ErrorKind::Interrupted => ErrorCode::Cancelled,
        _ => ErrorCode::Io,
    }
}

impl fmt::Display for FmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl std::error::Error for FmError {}

impl From<FmError> for ApiError {
    fn from(e: FmError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            detail: e.detail,
        }
    }
}

impl From<ApiError> for FmError {
    fn from(e: ApiError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            detail: e.detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosses_the_api_as_structured_data() {
        let e = FmError::not_found("face 7").with_detail("no row 7 in face");
        let api: ApiError = e.into();
        let json = serde_json::to_value(&api).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "notFound",
                "message": "face 7 was not found.",
                "detail": "no row 7 in face",
            })
        );
    }

    #[test]
    fn round_trips_through_api_error_without_loss() {
        let e = FmError::unsupported("Type 1 fonts cannot be activated.").with_detail("x.pfb");
        let back = FmError::from(ApiError::from(e.clone()));
        assert_eq!(back, e);
        assert_eq!(back.code(), ErrorCode::Unsupported);
        assert_eq!(back.message(), "Type 1 fonts cannot be activated.");
        assert_eq!(back.detail(), Some("x.pfb"));
    }

    #[test]
    fn message_is_for_people_detail_is_for_developers() {
        let e = FmError::internal("index.rs:42: row without a family");
        assert_eq!(e.message(), "Something went wrong inside Font Manager.");
        assert!(!e.message().contains("index.rs"));
        assert_eq!(e.detail(), Some("index.rs:42: row without a family"));
        assert_eq!(
            e.to_string(),
            "Something went wrong inside Font Manager. (index.rs:42: row without a family)"
        );
    }

    #[test]
    fn io_errors_keep_their_kind_and_put_the_path_in_the_detail() {
        let path = Path::new("fonts").join("ünï.ttf");
        let cases = [
            (io::ErrorKind::NotFound, ErrorCode::NotFound),
            (io::ErrorKind::PermissionDenied, ErrorCode::PermissionDenied),
            (io::ErrorKind::Interrupted, ErrorCode::Cancelled),
            (io::ErrorKind::InvalidData, ErrorCode::Io),
        ];
        for (kind, code) in cases {
            let os = io::Error::new(kind, "os says no");
            let e = FmError::io("The font could not be read.", &path, &os);
            assert_eq!(e.code(), code, "{kind:?}");
            assert_eq!(e.message(), "The font could not be read.");
            let detail = e.detail().unwrap();
            assert!(detail.contains("ünï.ttf"), "{detail}");
            assert!(detail.contains("os says no"), "{detail}");
        }
    }

    #[test]
    fn detail_is_absent_until_added() {
        let api = ApiError::from(FmError::invalid("The name is empty."));
        assert_eq!(api.detail, None);
        assert_eq!(api.code, ErrorCode::InvalidRequest);
    }
}
