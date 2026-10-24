//! macOS: `/usr/bin/open` hands an address to Launch Services, which opens it
//! in the default browser.

use crate::browser::OpenCommand;

/// The command that opens `url`; the caller checked it with `check_https`.
pub(crate) fn command(url: &str) -> OpenCommand {
    OpenCommand {
        program: "/usr/bin/open".into(),
        args: vec![url.into()],
    }
}
