# Webview after the E35 memory fixes (E35.5, E35.7)

Measured 2026-10-30 on Windows with `scripts/bench/webview.mjs` on `main` at
`e51ef2d` (E35.5 #409 and E35.7 #412 merged; E35.4 mimalloc and E35.6
ArrayBuffer faces were measured, did not help, and were dropped: #406, #408).
Release build (`pnpm tauri build --no-bundle`), same machine and library as
[webview-2026-10-29.md](webview-2026-10-29.md): Ryzen 9 3900X, 64 GB,
2560×1440 at 60 Hz, `D:\private\fm-reference-library` (15,329 faces in 2,544
families). Default settings of the script (60 s settle, 5 s per scroll run),
no forced garbage collection. Raw reports:
`target/bench/webview-windows-main-{warm,fresh}.json` (not committed).

## Memory, private working set (MB)

Warm start (index already built), one run:

| Row | 2026-10-29 total | now total | now app | now webview |
|---|---:|---:|---:|---:|
| idle | 190 | 200 | 61 | 139 |
| after List | 802 | 773 | 81 | 692 |
| after Table | 945 | 888 | 74 | 814 |
| after Grid | 1,739 | 1,041 | 74 | 967 |
| after Waterfall | 2,089 | 1,095 | 72 | 1,023 |
| idle again (60 s) | 1,694 | 746 | 64 | 682 |

Fresh start (`--fresh`: first scan of the library in the run), one run:

| Row | 2026-10-29 total | now total | now app | now webview |
|---|---:|---:|---:|---:|
| idle after the scan | 418 (app 276) | 232 | 83 | 149 |
| after Waterfall | — | 1,307 | 111 | 1,196 |
| idle again (60 s) | — | 835 | 108 | 726 |

- JS heap after Grid: 776 MB on 2026-10-29, 92 MB (warm) / 20 MB (fresh) now.
- App after a fresh scan: 276 MB → 83 MB, 1.37× the warm 61 MB (E35.7 target 1.5×).
- Idle at start is within the 350 MB budget. After scrolling every view once,
  the renderer keeps 680–730 MB; the browser's own font and image caches hold
  it (the GC experiment in webview-2026-10-29.md found a ~500 MB floor).

## Frames

| view | pace | 10-29 >18 ms | now warm >18 ms | now warm >50 ms | now fresh >18 ms |
|---|---|---:|---:|---:|---:|
| List | wheel | 0 % | 0 % | 0 | 0 % |
| List | fling | jank | 47 % | 21 | 45 % |
| List | drag | jank | 33 % | 16 | 62 % |
| Table | wheel | 0 % | 0.3 % | 0 | 0.3 % |
| Table | fling | jank | 0.6 % | 1 | 0.3 % |
| Table | drag | jank | 3.8 % | 3 | 4.5 % |
| Grid | wheel | 0 % | 0.3 % | 0 | 0 % |
| Grid | fling | jank | 43 % | 36 | 37 % |
| Grid | drag | jank | 0.3 % | 0 | 2.6 % |
| Waterfall | wheel | hitches | 0 % | 0 | 0 % |
| Waterfall | fling | jank | 2.9 % | 0 | 4.3 % |
| Waterfall | drag | jank | 70 % | 42 | 54 % |

- Wheel pace (one notch per 50 ms, how a hand scrolls) holds 60 fps in every
  view. Waterfall's wheel hitches from 10-29 are gone.
- Table is now within the frame budget at every pace.
- Fling (two notches per frame) still janks in List and Grid, drag (the whole
  list in 5 s) in List and Waterfall: p95 83–117 ms, up to 250 ms. The cost is
  face loads and layout for rows that enter and leave the viewport within a
  few frames, not memory.
- Window open: 1,348 ms warm (includes the remote-debugging start), list
  visible at 3.3 s; fresh run: list visible at 18.0 s including the 15,329-face
  scan.

## What this says for E35.3

- Memory: the targets that the fixes aimed at are met (app after scan, Grid
  heap). What stays is renderer cache, released under memory pressure.
- Scroll: 60 fps at hand pace everywhere. Fast flicks and drags still drop
  frames in List, Grid and Waterfall; a fix would skip face loads while the
  list moves faster than a row per frame and show the family name in a
  fallback font meanwhile (not a shell change).
- Seen on the way, not fixed: some Grid thumbnails render 8,192 px wide
  (PR #409 notes).
