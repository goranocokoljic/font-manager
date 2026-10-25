//! The exit codes of `fm`: fixed, and listed in `fm --help`.

use fm_api::ErrorCode;

/// What `fm` exits with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// 0: the command did what it was asked.
    Success = 0,
    /// 1: an unexpected error (internal, I/O, cancelled).
    Unexpected = 1,
    /// 2: the command line is not valid, or the request is not.
    Usage = 2,
    /// 3: a font, a face or another thing named is not there.
    NotFound = 3,
    /// 4: refused (permission denied, conflict, busy, unsupported).
    Refused = 4,
    /// 5: a check ran and found problems.
    Problems = 5,
}

/// The exit codes as `fm --help` lists them.
pub const EXIT_CODES: &str = "\
Exit codes:
  0  success
  1  unexpected error (internal, I/O, cancelled)
  2  usage error or invalid request
  3  not found
  4  refused (permission denied, conflict, busy, unsupported)
  5  a check found problems";

impl Exit {
    /// The code a failed API call exits with.
    pub fn of(code: ErrorCode) -> Self {
        match code {
            ErrorCode::Internal | ErrorCode::Io | ErrorCode::Cancelled => Self::Unexpected,
            ErrorCode::InvalidRequest => Self::Usage,
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::PermissionDenied
            | ErrorCode::Conflict
            | ErrorCode::Busy
            | ErrorCode::Unsupported => Self::Refused,
        }
    }

    /// The process exit code.
    pub fn code(self) -> u8 {
        self as u8
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(exit: Exit) -> Self {
        Self::from(exit.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_code_has_its_exit_code() {
        let cases = [
            (ErrorCode::Internal, 1),
            (ErrorCode::Io, 1),
            (ErrorCode::Cancelled, 1),
            (ErrorCode::InvalidRequest, 2),
            (ErrorCode::NotFound, 3),
            (ErrorCode::PermissionDenied, 4),
            (ErrorCode::Conflict, 4),
            (ErrorCode::Busy, 4),
            (ErrorCode::Unsupported, 4),
        ];
        for (code, exit) in cases {
            // The match makes a new ErrorCode a compile error here too.
            match code {
                ErrorCode::NotFound
                | ErrorCode::InvalidRequest
                | ErrorCode::Conflict
                | ErrorCode::PermissionDenied
                | ErrorCode::Unsupported
                | ErrorCode::Busy
                | ErrorCode::Io
                | ErrorCode::Cancelled
                | ErrorCode::Internal => {}
            }
            assert_eq!(Exit::of(code).code(), exit, "{code:?}");
        }
    }

    #[test]
    fn the_help_lists_every_exit_code() {
        let all = [
            Exit::Success,
            Exit::Unexpected,
            Exit::Usage,
            Exit::NotFound,
            Exit::Refused,
            Exit::Problems,
        ];
        for (i, exit) in all.iter().enumerate() {
            assert_eq!(usize::from(exit.code()), i);
            assert!(
                EXIT_CODES.contains(&format!("\n  {i}  ")),
                "{i} is not listed"
            );
        }
    }
}
