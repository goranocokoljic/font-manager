//! Linux: glibc's `malloc_trim(0)` returns the free memory of every arena,
//! not only the top of the main heap. Another C library (musl) has no such
//! call; there the memory stays with the process.

#[cfg(target_env = "gnu")]
pub(crate) fn release_free_memory() -> bool {
    unsafe extern "C" {
        /// glibc, `<malloc.h>`: 1 when memory was returned, 0 when none was.
        fn malloc_trim(pad: usize) -> std::ffi::c_int;
    }
    // SAFETY: malloc_trim takes a plain value and only releases free pages.
    unsafe { malloc_trim(0) };
    true
}

#[cfg(not(target_env = "gnu"))]
pub(crate) fn release_free_memory() -> bool {
    false
}
