//! Whether a process still runs, by its id (E25.4: a program that finds
//! `endpoint.json` checks that the app which wrote it is still there before
//! it connects). Each OS answers in `src/{windows,macos,linux}/process.rs`.
//!
//! The answer is a hint, never a permission: a process id can be reused, so
//! a caller that gets `true` still has to reach the process (the endpoint's
//! port) before it trusts it.

/// Whether a process with id `pid` runs now. Id 0, and ids above
/// `i32::MAX` (negative on Unix, where they name process groups), are never
/// a running app. When the OS cannot tell, the answer is `true`, so the
/// caller goes on to its own check.
pub fn is_running(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    #[cfg(windows)]
    return crate::windows::is_running(pid);
    #[cfg(target_os = "macos")]
    return crate::macos::is_running(pid);
    #[cfg(target_os = "linux")]
    return crate::linux::is_running(pid);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_runs() {
        assert!(is_running(std::process::id()));
    }

    #[test]
    fn a_process_that_ended_does_not_run() {
        // The test binary itself, asked only to list its tests: it ends at once.
        let exe = std::env::current_exe().expect("test binary");
        let mut child = std::process::Command::new(exe)
            .args(["--list", "--format", "terse"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawn");
        let pid = child.id();
        child.wait().expect("wait");
        assert!(!is_running(pid), "process {pid} ended");
    }

    #[test]
    fn id_zero_and_group_ids_never_run() {
        assert!(!is_running(0));
        assert!(!is_running(u32::MAX));
        assert!(!is_running(i32::MAX as u32 + 1));
    }
}
