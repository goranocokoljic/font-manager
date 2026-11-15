//! Linux: `xdg-open` on a folder opens it in the desktop's file manager; in a
//! Flatpak it goes through the OpenURI portal.

use std::path::Path;

use crate::browser::OpenCommand;

/// The command that shows `folder`; the caller checked that it is absolute.
pub(crate) fn command(folder: &Path) -> OpenCommand {
    OpenCommand {
        program: "xdg-open".into(),
        args: vec![folder.as_os_str().to_owned()],
    }
}
