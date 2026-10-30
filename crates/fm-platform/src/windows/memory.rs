//! Windows: `HeapSetInformation(HeapOptimizeResources)` decommits the free
//! memory of every heap of the process, the low-fragmentation heap's caches
//! included (`HeapCompact` leaves those). Windows 8.1 and later.

use std::ffi::c_void;

use windows::Win32::System::Memory::{HeapOptimizeResources, HeapSetInformation};

/// `HEAP_OPTIMIZE_RESOURCES_INFORMATION`, which the `windows` crate does not
/// bind.
#[repr(C)]
struct OptimizeResources {
    version: u32,
    flags: u32,
}

/// `HEAP_OPTIMIZE_RESOURCES_CURRENT_VERSION`.
const CURRENT_VERSION: u32 = 1;

pub(crate) fn release_free_memory() -> bool {
    let info = OptimizeResources {
        version: CURRENT_VERSION,
        flags: 0,
    };
    // SAFETY: no heap handle means every heap of the process; `info` is the
    // documented structure, outlives the call, and its size is passed.
    unsafe {
        HeapSetInformation(
            None,
            HeapOptimizeResources,
            Some((&raw const info).cast::<c_void>()),
            size_of::<OptimizeResources>(),
        )
    }
    .is_ok()
}
