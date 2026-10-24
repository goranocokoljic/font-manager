//! Linux: `xdg-open` opens an address in the desktop's default browser; in a
//! Flatpak it goes through the OpenURI portal.

use crate::browser::OpenCommand;

/// The command that opens `url`; the caller checked it with `check_https`.
pub(crate) fn command(url: &str) -> OpenCommand {
    OpenCommand {
        program: "xdg-open".into(),
        args: vec![url.into()],
    }
}
