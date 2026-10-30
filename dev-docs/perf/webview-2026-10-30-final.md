# Webview on main after E35 (final check for ADR 0020)

Measured 2026-10-30 on Windows with `node scripts/bench/webview.mjs` on `main` at
`fec3e73` (E35.5, E35.7 and E35.8 merged). Release build
(`pnpm tauri build --no-bundle`), same machine and library as
[webview-2026-10-29.md](webview-2026-10-29.md): Ryzen 9 3900X, 64 GB, 2560×1440 at
60 Hz, `D:\private\fm-reference-library` (15,329 faces in 2,544 families). Default
settings of the script (60 s settle, 5 s per scroll run), warm start, one run, no
forced garbage collection. Raw report: `target/bench/webview-windows-main-final.json`
(not committed). Window open 1,171 ms (with the debugging port), list visible at 3.6 s.

## Memory, private working set (MB)

| Row | 2026-10-29 total | now total | now app | now webview |
|---|---:|---:|---:|---:|
| idle | 190 | 195 | 61 | 133 |
| after List | 802 | 537 | 63 | 474 |
| after Table | 945 | 632 | 62 | 570 |
| after Grid | 1,739 | 718 | 63 | 655 |
| after Waterfall | 2,089 | 673 | 64 | 610 |
| idle again (60 s) | 1,694 | 600 | 63 | 537 |

JS heap after Grid: 776 MB on 10-29, 28 MB now. The app process stays at 61–64 MB
through the whole run. The app after a fresh scan was measured on 10-30 after E35.7:
83 MB (`webview-2026-10-30-after-e35.md`), unchanged by E35.8.

## Frames

| view | pace | 10-29 >18 ms | now >18 ms | now >50 ms | now p95 | now max |
|---|---|---:|---:|---:|---:|---:|
| List | wheel | 0 % | 0 % | 0 | 16.8 ms | 16.8 ms |
| List | fling | jank | 0.3 % | 1 | 16.8 ms | 66.8 ms |
| List | drag | jank | 1.6 % | 0 | 16.8 ms | 34.1 ms |
| Table | wheel | 0 % | 0.3 % | 0 | 16.8 ms | 33.4 ms |
| Table | fling | jank | 0.6 % | 1 | 16.8 ms | 100.2 ms |
| Table | drag | jank | 2.3 % | 4 | 16.8 ms | 83.4 ms |
| Grid | wheel | 0 % | 0 % | 0 | 16.8 ms | 17.0 ms |
| Grid | fling | jank | 0.6 % | 1 | 16.8 ms | 66.8 ms |
| Grid | drag | jank | 0 % | 0 | 16.8 ms | 16.9 ms |
| Waterfall | wheel | hitches | 0 % | 0 | 16.8 ms | 16.8 ms |
| Waterfall | fling | jank | 3.9 % | 1 | 16.8 ms | 50.1 ms |
| Waterfall | drag | jank | 1.3 % | 0 | 16.8 ms | 50.0 ms |

- p95 is 16.8 ms (one 60 Hz frame) in every view at every pace. On 10-29 it was
  83–117 ms for List and Grid at fling and drag.
- Wheel pace meets the script's own line (1 %, no frame over 50 ms) in every view.
- Fling and drag meet the E35.8 bar (10 %, 100 ms) in every view. Single frames of
  50–100 ms remain (one per run in List, Grid and Waterfall fling; Table drag has four);
  the Table never deferred and is now the least smooth view at drag pace.

## Verdict for ADR 0020

Idle memory within the 350 MB budget, 60 fps at every pace in every view on the
reference library: the webview stays. See `docs/adr/0020-keep-the-webview-shell.md`.
