//! The IPC socket on macOS (ADR 0016): `fm.sock` in the data folder.
//!
//! The steps are Linux's (`crate::linux::ipc`): a `0700` folder, a stale
//! socket replaced, the file removed when the listener drops. Only the mode
//! differs: macOS ignores a mode set on a socket before `bind()`, so the
//! file is made `0600` right after it; the `0700` folder already keeps
//! every other user out in between.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;

use crate::ipc::IpcName;
use crate::linux::ipc::{Listener as UnixListener, SOCKET_MODE, bind_with};
pub(crate) use crate::linux::ipc::{Stream, connect};

#[derive(Debug)]
pub(crate) struct Listener(UnixListener);

impl Listener {
    pub(crate) fn bind(name: &IpcName) -> io::Result<Self> {
        bind_with(
            name,
            |options| options,
            |path| fs::set_permissions(path, fs::Permissions::from_mode(SOCKET_MODE)),
        )
        .map(Self)
    }

    pub(crate) fn accept(&self) -> io::Result<Stream> {
        self.0.accept()
    }

    pub(crate) fn is_private(&self) -> io::Result<bool> {
        self.0.is_private()
    }
}
