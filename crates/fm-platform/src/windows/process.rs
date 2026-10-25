//! Windows: a process runs while `OpenProcess` finds it and its exit code is
//! still `STILL_ACTIVE`.

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, STILL_ACTIVE};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

pub(crate) fn is_running(pid: u32) -> bool {
    // SAFETY: OpenProcess takes plain values and returns an owned handle or
    // an error; the handle is closed below.
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(handle) => handle,
        // Another user's (or a protected) process: it exists.
        Err(e) => return e.code() == ERROR_ACCESS_DENIED.to_hresult(),
    };
    let mut code = 0u32;
    // SAFETY: `handle` is a valid process handle opened with query rights;
    // `code` outlives the call.
    let read = unsafe { GetExitCodeProcess(handle, &mut code) };
    // SAFETY: `handle` came from OpenProcess and is closed once.
    let _ = unsafe { CloseHandle(handle) };
    match read {
        Ok(()) => code == STILL_ACTIVE.0 as u32,
        Err(_) => true,
    }
}
