//! The IPC transport (E29.1, ADR 0016): a per-user named pipe on Windows, a
//! Unix domain socket on macOS and Linux, through the `interprocess` crate.
//!
//! Only the user who runs the app can connect. The pipe has a protected DACL
//! with one entry, the user's SID, and refuses remote clients; the socket is
//! mode `0600` inside a `0700` folder. A stale socket from a crashed run is
//! replaced at [`IpcListener::bind`]; dropping the listener removes the
//! socket. The bytes are the caller's business: this module knows nothing of
//! messages.
//!
//! The name comes from [`AppPaths::ipc_name`](crate::AppPaths::ipc_name).

use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use crate::linux::ipc as imp;
#[cfg(target_os = "macos")]
use crate::macos::ipc as imp;
#[cfg(windows)]
use crate::windows::ipc as imp;

/// The pipe's name prefix on Windows; the user name follows.
pub const PIPE_PREFIX: &str = r"\\.\pipe\org.fontmanager.app-";

/// The socket's file name on macOS and Linux.
pub const SOCKET_FILE: &str = "fm.sock";

/// Where the IPC listener is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IpcName {
    /// A named pipe path (`\\.\pipe\…`), Windows.
    Pipe(String),
    /// A Unix domain socket file, macOS and Linux.
    Socket(PathBuf),
}

impl fmt::Display for IpcName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipe(name) => f.write_str(name),
            Self::Socket(path) => write!(f, "{}", path.display()),
        }
    }
}

/// The pipe name for `user`: characters a pipe name or a log line would
/// trip over become `_`.
pub(crate) fn pipe_name(user: &str) -> String {
    let user: String = user
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let user = if user.is_empty() { "user" } else { &user };
    format!("{PIPE_PREFIX}{user}")
}

/// A listening pipe or socket. Dropping it stops listening and, for a
/// socket, removes the file.
#[derive(Debug)]
pub struct IpcListener {
    inner: imp::Listener,
    name: IpcName,
}

impl IpcListener {
    /// Listens at `name`, private to this user. A socket file left by a
    /// crashed run (nothing answers on it) is replaced; a live one, or a
    /// file at the path that is not a socket, is an error
    /// ([`io::ErrorKind::AddrInUse`], [`io::ErrorKind::AlreadyExists`]).
    pub fn bind(name: &IpcName) -> io::Result<Self> {
        Ok(Self {
            inner: imp::Listener::bind(name)?,
            name: name.clone(),
        })
    }

    /// Waits for the next client.
    pub fn accept(&self) -> io::Result<IpcStream> {
        self.inner.accept().map(IpcStream)
    }

    /// Where this listener is.
    pub fn name(&self) -> &IpcName {
        &self.name
    }

    /// Whether only this user can connect: the pipe's DACL is exactly the
    /// user's SID (Windows); the socket is `0600` and its folder `0700`.
    pub fn is_private(&self) -> io::Result<bool> {
        self.inner.is_private()
    }
}

/// One connection, either end.
#[derive(Debug)]
pub struct IpcStream(imp::Stream);

impl IpcStream {
    /// A handle that ends this connection from another thread: a read
    /// waiting on the stream returns at once, and the client sees the
    /// connection closed. It stays valid after the stream drops.
    pub fn closer(&self) -> io::Result<IpcCloser> {
        imp::closer(&self.0).map(IpcCloser)
    }
}

/// Ends one connection ([`IpcStream::closer`]).
#[derive(Debug)]
pub struct IpcCloser(imp::Closer);

impl IpcCloser {
    /// Closes the connection; closing one already closed does nothing.
    pub fn close(&self) {
        imp::close(&self.0);
    }
}

impl Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// The message of the [`connect`] error when nothing listens at the name.
pub const NOT_RUNNING: &str = "Font Manager is not running";

/// Connects to the listener at `name`. When nothing listens there (the app
/// quit: the pipe is gone, the socket file removed or not answering), the
/// error says [`NOT_RUNNING`] at once and keeps the OS error's kind; a pipe
/// whose instances all stay busy gives up after a short wait.
pub fn connect(name: &IpcName) -> io::Result<IpcStream> {
    imp::connect(name)
        .map(IpcStream)
        .map_err(|e| not_running(name, e))
}

/// `e` with the plain message when it means that nothing listens at `name`.
fn not_running(name: &IpcName, e: io::Error) -> io::Error {
    match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => io::Error::new(
            e.kind(),
            format!("{NOT_RUNNING} (nothing listens at {name}: {e})"),
        ),
        _ => e,
    }
}

/// The error for a name of the other OS family.
#[cfg_attr(not(any(windows, unix)), allow(dead_code))]
pub(crate) fn wrong_kind(name: &IpcName) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("not an IPC name for this OS: {name}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pipe_name_keeps_plain_user_names_and_replaces_the_rest() {
        assert_eq!(pipe_name("ada"), r"\\.\pipe\org.fontmanager.app-ada");
        assert_eq!(
            pipe_name(r"DOMAIN\Ada Lovelace"),
            r"\\.\pipe\org.fontmanager.app-DOMAIN_Ada_Lovelace"
        );
        assert_eq!(
            pipe_name("j.doe-2_x"),
            r"\\.\pipe\org.fontmanager.app-j.doe-2_x"
        );
        assert_eq!(pipe_name(""), r"\\.\pipe\org.fontmanager.app-user");
    }

    #[test]
    fn nothing_listening_reads_as_not_running_and_other_errors_stay() {
        let name = IpcName::Socket(PathBuf::from("/tmp/fm.sock"));
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::ConnectionRefused] {
            let e = not_running(&name, io::Error::from(kind));
            assert_eq!(e.kind(), kind);
            assert!(e.to_string().starts_with(NOT_RUNNING), "{e}");
            assert!(e.to_string().contains("fm.sock"), "{e}");
        }
        let denied = not_running(&name, io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(!denied.to_string().contains(NOT_RUNNING), "{denied}");
    }
}
