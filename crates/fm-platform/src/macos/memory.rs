//! macOS: `malloc_zone_pressure_relief(NULL, 0)` asks every malloc zone to
//! return as much free memory as it can, as the system does under memory
//! pressure.

use std::ffi::c_void;

unsafe extern "C" {
    /// libSystem, `<malloc/malloc.h>`: the bytes released.
    fn malloc_zone_pressure_relief(zone: *mut c_void, goal: usize) -> usize;
}

pub(crate) fn release_free_memory() -> bool {
    // SAFETY: a null zone means every zone; a goal of 0 means as much as
    // possible. Only free pages are released.
    unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    true
}
