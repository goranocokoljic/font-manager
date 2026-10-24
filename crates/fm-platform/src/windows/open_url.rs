//! Windows: the URL protocol handler opens an address in the default browser.
//! `rundll32 url.dll,FileProtocolHandler` takes the address as one argument
//! and goes through no shell, unlike `cmd /c start`, which reads `&` in a
//! query string as a command separator.

use crate::browser::OpenCommand;

/// The command that opens `url`; the caller checked it with `check_https`.
pub(crate) fn command(url: &str) -> OpenCommand {
    OpenCommand {
        program: "rundll32.exe".into(),
        args: vec!["url.dll,FileProtocolHandler".into(), url.into()],
    }
}
