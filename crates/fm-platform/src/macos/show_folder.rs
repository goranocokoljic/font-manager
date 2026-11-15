//! macOS: `/usr/bin/open` on a folder opens it in Finder.

use std::path::Path;

use crate::browser::OpenCommand;

/// The command that shows `folder`; the caller checked that it is absolute.
pub(crate) fn command(folder: &Path) -> OpenCommand {
    OpenCommand {
        program: "/usr/bin/open".into(),
        args: vec![folder.as_os_str().to_owned()],
    }
}
