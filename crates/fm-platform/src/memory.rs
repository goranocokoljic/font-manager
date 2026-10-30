//! Giving freed memory back to the OS (E35.7, ADR 0019).
//!
//! A first scan parses every font on more threads than cores and frees what
//! it read when it ends. The allocator keeps part of that freed memory for
//! the next allocation: on Windows the process heap holds about 12 MB of
//! small-block caches after 32 threads parsed, which the app does not use
//! again until the next scan. Each OS has its own call to hand such memory
//! back, in `src/{windows,macos,linux}/memory.rs`.

/// Asks the allocator to return the memory it keeps free to the OS. Cheap,
/// safe to call from any thread at any time; nothing in use moves. Returns
/// `false` when the OS has no such call or refused it, which costs memory,
/// never correctness.
pub fn release_free_memory() -> bool {
    #[cfg(windows)]
    return crate::windows::release_free_memory();
    #[cfg(target_os = "macos")]
    return crate::macos::release_free_memory();
    #[cfg(target_os = "linux")]
    return crate::linux::release_free_memory();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freed_memory_can_be_released_while_other_memory_is_in_use() {
        let kept: Vec<Vec<u8>> = (0..64).map(|i| vec![i as u8; 1024]).collect();
        let freed: Vec<Vec<u8>> = (0..4096).map(|i| vec![i as u8; 256]).collect();
        drop(freed);
        let released = release_free_memory();
        if cfg!(any(
            windows,
            target_os = "macos",
            all(target_os = "linux", target_env = "gnu")
        )) {
            assert!(released, "the OS call is there");
        }
        for (i, block) in kept.iter().enumerate() {
            assert!(block.iter().all(|&b| b == i as u8), "block {i} intact");
        }
    }
}
