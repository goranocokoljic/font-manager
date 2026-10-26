//! The IPC pipe on Windows (ADR 0016): `\\.\pipe\org.fontmanager.app-<user>`.
//!
//! Every instance is created with a protected DACL of one entry, full access
//! for the user's SID (the same as the private files of ADR 0015), so no
//! other user, not even an administrator, may open it. Remote clients are
//! refused (`PIPE_REJECT_REMOTE_CLIENTS`: interprocess sets it while
//! `accept_remote` is off), and the first instance is created with
//! `FILE_FLAG_FIRST_PIPE_INSTANCE`, so a pipe another program made under
//! this name first is an error, not a server we share. A pipe disappears with
//! its last handle; there is no stale pipe to clean up.

use std::io;
use std::os::windows::io::{AsHandle as _, AsRawHandle, OwnedHandle};
use std::time::Duration;

use interprocess::ConnectWaitMode;
use interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, pipe_mode,
};
use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptorExt as _, BorrowedSecurityDescriptor, SecurityDescriptor,
};
use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT};
use windows::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
use windows::Win32::System::Pipes::DisconnectNamedPipe;

use super::private_file;
use crate::ipc::{IpcName, wrong_kind};

pub(crate) type Stream = DuplexPipeStream<pipe_mode::Bytes>;

/// A duplicate of the server end's handle: disconnecting it ends the pipe
/// instance for every copy.
pub(crate) type Closer = OwnedHandle;

/// How long a client waits while every pipe instance is busy.
const CONNECT_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct Listener(PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>);

impl Listener {
    pub(crate) fn bind(name: &IpcName) -> io::Result<Self> {
        let IpcName::Pipe(path) = name else {
            return Err(wrong_kind(name));
        };
        PipeListenerOptions::new()
            .path(path.as_str())
            .accept_remote(false)
            .security_descriptor(Some(only_user()?))
            .create_duplex::<pipe_mode::Bytes>()
            .map(Self)
    }

    pub(crate) fn accept(&self) -> io::Result<Stream> {
        self.0.accept()
    }

    pub(crate) fn is_private(&self) -> io::Result<bool> {
        let handle = HANDLE(self.0.as_raw_handle());
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // SAFETY: `handle` is the listener's open pipe handle, valid while
        // `self` lives; `sd` receives a LocalAlloc'd descriptor, freed below.
        let status = unsafe {
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                Some(&mut sd),
            )
        };
        status.ok().map_err(io::Error::other)?;
        let actual = private_file::dacl_text(sd);
        // SAFETY: `sd` was allocated by GetSecurityInfo and is freed once.
        let _ = unsafe { LocalFree(Some(HLOCAL(sd.0))) };
        let expected = private_file::expected_dacl_text()?;
        Ok(private_file::only_user(
            &actual?,
            private_file::aces_of(&expected),
        ))
    }
}

pub(crate) fn connect(name: &IpcName) -> io::Result<Stream> {
    let IpcName::Pipe(path) = name else {
        return Err(wrong_kind(name));
    };
    Stream::connect_by_path_with_wait_mode(path.as_str(), ConnectWaitMode::Timeout(CONNECT_WAIT))
}

pub(crate) fn closer(stream: &Stream) -> io::Result<Closer> {
    stream.as_handle().try_clone_to_owned()
}

pub(crate) fn close(closer: &Closer) {
    // SAFETY: `closer` is an open handle this process owns (a duplicate of
    // the pipe instance's handle), valid for the call. On a client end or an
    // instance already disconnected the call fails, which ends nothing.
    let _ = unsafe { DisconnectNamedPipe(HANDLE(closer.as_raw_handle())) };
}

/// `D:P(A;;FA;;;<user SID>)` as a security descriptor interprocess owns.
fn only_user() -> io::Result<SecurityDescriptor> {
    let sd = private_file::parse_sddl(&private_file::only_user_sddl()?)?;
    // SAFETY: `sd` is a valid self-relative descriptor from the conversion
    // in `parse_sddl`; it is only read here, and copied into an absolute
    // descriptor that owns its contents before `sd` is freed.
    let owned = unsafe { BorrowedSecurityDescriptor::from_ptr(sd.0.cast_const()) }.to_owned_sd();
    // SAFETY: allocated by `parse_sddl`; freed once.
    let _ = unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    owned
}
