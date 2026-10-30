# What the app holds after a scan (E35.7)

Measured 2026-10-30 on Windows, same machine and library as
[webview-2026-10-29.md](webview-2026-10-29.md): Ryzen 9 3900X (24 threads), 64 GB,
`D:\private\fm-reference-library` (15,329 faces in 2,544 families). Release builds
(`pnpm tauri build --no-bundle`) on base `45f18c4`. App memory is the private
working set of `fm-app.exe`, column "app" of `scripts/bench/webview.mjs`; "fresh" is
`--fresh` (the first scan included), "warm" is a start on the index the fresh run
left. The raw reports are `target/bench/webview-{main,e357,e357b}-*.json` (not
committed). Decision: [ADR 0019](../../docs/adr/0019-memory-after-a-scan.md).

## Holders

| # | Holder | How measured | Held after the first scan |
|---|--------|--------------|--------------------------:|
| 1 | The scan's file index (`FileIndex` in `CoreApi`): every face with names, features, code points, fingerprint, kept until quit | App, fresh: `main` against the index dropped after the scan; counter: the index loaded back (`Index::load_files`) and summed per field | **183 MB** (276.5 → 93.2). Counter: 175 MB, of it code points 103 MB, features 12 MB, names 9 MB, the face structs 9 MB (584 B each) |
| 2 | Free memory the Windows heap keeps after 32 scan threads parsed fonts (low-fragmentation heap caches) | App, fresh: 1 against 1 + `HeapSetInformation(HeapOptimizeResources)`; core alone: 32 threads parse the fontshare folder and free everything | **9–16 MB** (93.2 → 84.5 / 77.3). Core alone: 16.8 MB → 7.6 MB after the call; `HeapCompact` released nothing (92.8 MB) |
| — | Not holders | Core alone, measured the same way | The language presets loaded by the scan (< 0.1 MB); reading files on 32 threads without parsing (+0.7 MB); the search cache (built on a warm start too) |

The core alone, without the webview (a probe over `CoreApi`, not committed): warm
start and first page 52 MB; after a fresh scan and the first page 74 MB with holder 1
dropped, 58 MB with holder 2 released as well. What is left (5–20 MB in the app) is
not one holder: the scan's own allocations spread over the heap while the UI
listed and searched during it.

## Result

Private working set of the app process, 60 s idle after the list settled (MB):

| Build | Fresh (first scan) | Warm | Fresh / warm |
|-------|-------------------:|-----:|-------------:|
| `main` (E35.1 had 276 / 61) | 276.5 | 61 (E35.1) | 4.5× |
| File index dropped after the scan | 93.2 | 60.5 | 1.54× |
| Dropped + free heap memory returned (this story), run 1 | **84.5** | 61.4 | **1.38×** |
| Same, run 2 | **77.3** | 62.7 | **1.23×** |

Criterion: within 1.5× of the warm value. **Met.**

After scrolling every view and 60 s idle again the app holds 102–116 MB fresh and
62–65 MB warm (`main` fresh: 290 MB); that difference is not measured further. The
webview's idle numbers did not change (150–157 MB fresh in every build).

`scripts/bench.sh`, this branch against the last `main` run on this machine
(`target/bench/bench-base2.json`): idle_memory 16.3 / 16.5 MB, first_scan 3,285 /
3,592 ms, incremental_update 828 / 833 ms, search 27.9 / 30.0 ms,
preview_cache_hit 0.2 / 0.2 ms; every budget met. The core benches do not run the
API's scan thread, so they measure the same code; `incremental_update` always
loaded the file index from the index, as every scan request of the app now does
(0.5 s for the reference library, the cost the first scan after a start always paid).
