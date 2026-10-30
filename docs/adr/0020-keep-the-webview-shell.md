# 0020 — Keep the webview shell

Status: Accepted (2026-10-30)

## Context

ADR 0001 chose Tauri 2 + Vue 3 for the desktop shell and set the condition under which
a native shell (iced or GPUI) would be revisited: the webview cannot hold 60 fps on a
10k-row list, or it misses the idle memory target (350 MB). Epic E35 was the check.

E35.1 (#200) built the measurement (`scripts/bench/webview.mjs`: a release build, the
reference library of 15,329 faces in 2,544 families, every view scrolled at three paces,
memory of the whole process tree, every animation frame recorded) and measured `main` on
2026-10-29 (`dev-docs/perf/webview-2026-10-29.md`):

- Idle at start: 190 MB in total, within budget. After one scroll through the four
  views: 2,089 MB, and 1,694 MB after 60 s idle. The app process kept 276 MB after a
  first scan. The Grid's JS heap reached 776 MB.
- Wheel pace (one notch per 50 ms): 60 fps in every view. Fast flicks and scrollbar
  drags dropped frames in every view (p95 up to 117 ms).

The experiment that drove the fixes: a forced garbage collection from outside gave most
of the renderer's memory back (List 1,368 → 567 MB, Grid 1,711 → 491 MB). The memory
was garbage the collector got to late, not memory the app needed.

Six stories followed; four changed the code, two were measured and dropped:

| Story | Change | Result |
|---|---|---|
| E35.5 #407 | Grid thumbnails cross Tauri's IPC as raw bytes, not a JSON number array. Found on the way: the content security policy lacked `connect-src` for Tauri's IPC, so every command used the slow message-channel fallback | JS heap after Grid 776 → 19–92 MB; window opens in 1 s instead of 1.3 s |
| E35.7 #411 | The scan's file index lives for one scan request; free heap memory returned per OS (ADR 0019) | App after a first scan 276 → 83 MB (1.37× warm) |
| E35.8 #413 | No face loads, text, size, metrics or thumbnail requests while a view moves faster than a row per frame; rows show their name in the UI font until the view settles | Fling and drag p95 117 → 16.8 ms; memory after scrolling lower in every row |
| #405 | `blob:` allowed in `img-src` | Grid cards render (they were blank) |
| E35.4 #406 | `mimalloc` as global allocator | 276 → 255 MB: dropped, the memory was held, not fragmented |
| E35.6 #408 | `FontFace` from an `ArrayBuffer` instead of `url()` | 565 → 576 MB: dropped, the renderer already frees dropped faces |

Measured on `main` after all of it (`dev-docs/perf/webview-2026-10-30-final.md`;
Windows 11, Ryzen 9 3900X, 2560×1440 at 60 Hz, warm start):

| | 2026-10-29 | 2026-10-30 |
|---|---:|---:|
| Idle at start, total | 190 MB | 195 MB |
| After scrolling every view, total | 2,089 MB | 673 MB (peak 718 after Grid) |
| Idle again after 60 s, total | 1,694 MB | 600 MB |
| App process after a first scan | 276 MB | 83 MB |
| Worst p95 frame, any view, any pace | 117 ms | 16.8 ms |
| Wheel pace frames over 18 ms, worst view | hitches (Waterfall) | 0.3 % |
| Fling or drag frames over 18 ms, worst view | 70 % | 3.9 % |

## Decision

The webview shell stays. ADR 0001's condition for revisiting it is not met: the list
holds 60 fps at every pace in every view on 15,329 faces, and idle memory is within
budget. E35.2 (#201, the iced/GPUI prototype) is not built.

The epic also sets how the webview is kept honest:

1. `scripts/bench/webview.mjs` is the measurement. A story that touches the list, the
   previews, the face loader or the IPC runs it and puts its numbers in a dated file
   under `dev-docs/perf/`. The verdict lines of the script (1 % of frames over 18 ms,
   none over 50 ms, idle under 350 MB) are the bar for wheel pace; fling and drag are
   held to 10 % and 100 ms (E35.8).
2. Large data crosses the IPC as bytes (Tauri's raw response), never as a JSON array
   of numbers. The content security policy lists every scheme the page fetches
   (`ipc:`, `fmfont:`, `blob:` for images); `window_config.rs` has one test per entry
   with its reason.
3. Fonts reach the renderer through the asset protocol and `FontFace` with a `url()`
   source, as before (E35.6). No forced garbage collection, no V8 switches: the
   renderer's caches are the browser's to keep and to drop under memory pressure.
4. Work for a row starts when the view has settled on it, not when the row is
   rendered (`src/list/settle.ts`). A new virtualized view uses the same helper.
5. The app keeps no scan state past the scan request (ADR 0019); the allocator stays
   the system one.

## Consequences

- The ~500–700 MB the renderer keeps after a long scroll is browser cache (font and
  image data), not app state. It is not counted against the 350 MB idle budget, which
  is measured at start; it is reported in every perf note so growth is seen.
- The Table view, which never deferred, is the reference for a view that does little
  per row; Grid thumbnails stay bitmaps from the core rasterizer (one mask per card,
  cheap to recycle), which is the reason the Grid is as smooth as the Table now.
- A native shell is off the table for v1. Reopening it needs a new measurement that
  shows the bar in item 1 missed on the reference library after the fixes above.
- Open on the way: some Grid thumbnails render 8,192 px wide (PR #409 notes); the
  CI bench job does not run the webview bench, which needs a display and the
  reference library, so its numbers are local and go into `dev-docs/perf/` by hand.
