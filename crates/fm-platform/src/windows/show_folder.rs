//! Windows: Explorer opens a folder given as its one argument. The path goes
//! as one argument through no shell, so `&` in a folder name is only text.

use std::path::Path;

use crate::browser::OpenCommand;

/// The command that shows `folder`; the caller checked that it is absolute.
pub(crate) fn command(folder: &Path) -> OpenCommand {
    OpenCommand {
        program: "explorer.exe".into(),
        args: vec![folder.as_os_str().to_owned()],
    }
}
