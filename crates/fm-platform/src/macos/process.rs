//! macOS: a process runs when `kill -0 <pid>` (no signal sent, only the
//! check) succeeds. `/proc` does not exist on macOS, and the signal check
//! needs no FFI this way.

use std::process::{Command, Stdio};

pub(crate) fn is_running(pid: u32) -> bool {
    match Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        // The check could not run: let the caller decide by its own check.
        Err(_) => true,
    }
}
