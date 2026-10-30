# 0019 — Memory after a scan

Status: Accepted (2026-10-30)

## Context

E35.1 measured the app process at 276 MB after a first scan of the reference library
(15,329 faces) and 60 s idle, against 61 MB on a warm start. E35.4 tried another
allocator (`mimalloc`); it moved the number by 8 % and was dropped (#406). E35.7
measured what holds the memory (`dev-docs/perf/app-memory-2026-10-30.md`):

1. The scan's file index (`FileIndex`): every face with its names, features, code
   points and fingerprint, about 175 MB for the reference library, kept by the API
   until the app quits so the next scan can skip unchanged files.
2. The allocator's free memory: after 32 threads parsed fonts and freed the results,
   the Windows process heap keeps about 12 MB of small-block caches. `HeapCompact`
   does not release them.

## Decision

- The file index lives for one scan request. The scan thread drops it after the
  request's last job, before that job's last event. The next scan loads it back from
  the database (`Index::load_files`, 0.5 s for the reference library), as the first
  scan after a start always did.
- Then the core calls `fm_platform::memory::release_free_memory()`, which asks the
  allocator to return its free memory to the OS. One implementation per OS, no new
  crate:

| OS | Call |
|----|------|
| Windows | `HeapSetInformation(NULL, HeapOptimizeResources)` (Windows 8.1+; decommits the low-fragmentation heap's caches too) |
| macOS | `malloc_zone_pressure_relief(NULL, 0)` (every zone, as much as possible) |
| Linux | glibc `malloc_trim(0)`; no call on another C library (the memory stays) |

## Consequences

- Every scan request pays one `load_files`; the `incremental_update` bench always
  measured it, so its number does not move.
- The call only returns free pages; nothing in use moves, and a failure costs memory,
  never correctness. It runs once per scan request, not on a timer.
- The allocator stays the system one (E35.4).
