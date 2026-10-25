//! Linux: a process runs while `/proc/<pid>` exists. Without a mounted
//! `/proc` the answer is `true`, and the caller decides by its own check.

use std::path::Path;

pub(crate) fn is_running(pid: u32) -> bool {
    let proc = Path::new("/proc");
    if !proc.join("self").exists() {
        return true;
    }
    proc.join(pid.to_string()).exists()
}
