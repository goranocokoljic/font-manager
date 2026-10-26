//! The IPC socket on Linux (ADR 0016): `fm.sock` in
//! `$XDG_RUNTIME_DIR/font-manager/`, or in the data folder.
//!
//! The folder is made `0700` before the socket is bound, and the socket is
//! `0600` from the start: interprocess sets the mode on the socket before
//! `bind()`. A socket file nothing answers on is a crashed run's and is
//! replaced; one that answers is another running app, and a file there that
//! is not a socket is never touched. Dropping the listener removes the file.
//!
//! macOS shares this code (`crate::macos::ipc`) except for how the mode is
//! set.

use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};

use crate::ipc::{IpcName, wrong_kind};

pub(crate) type Stream = interprocess::local_socket::Stream;

/// The socket file's mode.
pub(crate) const SOCKET_MODE: u32 = 0o600;
/// The mode of the folder that holds it.
pub(crate) const FOLDER_MODE: u32 = 0o700;

#[derive(Debug)]
pub(crate) struct Listener {
    inner: interprocess::local_socket::Listener,
    path: PathBuf,
}

/// Binds the socket at `name` after the folder and stale-file steps;
/// `options` sets how the socket gets its mode on this OS, and `after`
/// runs once it is bound.
pub(crate) fn bind_with(
    name: &IpcName,
    options: impl FnOnce(ListenerOptions<'_>) -> ListenerOptions<'_>,
    after: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<Listener> {
    let IpcName::Socket(path) = name else {
        return Err(wrong_kind(name));
    };
    let dir = path.parent().ok_or_else(|| wrong_kind(name))?;
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(FOLDER_MODE))?;
    clear_stale(path)?;
    let fs_name = path.as_path().to_fs_name::<GenericFilePath>()?;
    let inner = options(ListenerOptions::new().name(fs_name).reclaim_name(true)).create_sync()?;
    let listener = Listener {
        inner,
        path: path.clone(),
    };
    // On an error the listener drops here and removes the socket.
    after(path)?;
    Ok(listener)
}

/// Removes a socket file nothing answers on; refuses a live one and
/// anything that is not a socket.
fn clear_stale(path: &Path) -> io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if !meta.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} exists and is not a socket", path.display()),
        ));
    }
    if connect_path(path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!("another Font Manager listens on {}", path.display()),
        ));
    }
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

impl Listener {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) fn bind(name: &IpcName) -> io::Result<Self> {
        bind_with(name, set_mode_before_bind, |_| Ok(()))
    }

    pub(crate) fn accept(&self) -> io::Result<Stream> {
        self.inner.accept()
    }

    pub(crate) fn is_private(&self) -> io::Result<bool> {
        let mode = |p: &Path| fs::symlink_metadata(p).map(|m| m.permissions().mode() & 0o777);
        let dir = self.path.parent().unwrap_or(Path::new("/"));
        Ok(mode(&self.path)? == SOCKET_MODE && mode(dir)? == FOLDER_MODE)
    }
}

#[cfg(target_os = "linux")]
fn set_mode_before_bind(options: ListenerOptions<'_>) -> ListenerOptions<'_> {
    use interprocess::os::unix::local_socket::ListenerOptionsExt as _;
    options.mode(SOCKET_MODE)
}

#[cfg(not(target_os = "linux"))]
fn set_mode_before_bind(options: ListenerOptions<'_>) -> ListenerOptions<'_> {
    options
}

pub(crate) fn connect(name: &IpcName) -> io::Result<Stream> {
    let IpcName::Socket(path) = name else {
        return Err(wrong_kind(name));
    };
    connect_path(path)
}

fn connect_path(path: &Path) -> io::Result<Stream> {
    Stream::connect(path.to_fs_name::<GenericFilePath>()?)
}
