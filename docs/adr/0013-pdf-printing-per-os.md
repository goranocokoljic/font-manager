# 0013 — PDF printing per OS

Status: Accepted (2026-10-24)

## Context

Story E21.2 adds a PDF specimen: the self-contained HTML page of E21.1,
printed to a paginated A4 PDF. Every OS already has a webview the app ships
with (Tauri, ADR 0001), and each can print a page to a file. Starting an
external browser (headless Chrome) would add a large, unpinned dependency and
start a program the user did not ask for. The webviews are single-threaded
UI objects and differ in how they print, so the printing is platform code.

## Decision

`fm-platform` has one trait, `PdfPrinter`, with one implementation per OS under
`src/{windows,macos,linux}/pdf.rs`. `print_html(html, out)` loads a local HTML
file in an offscreen webview, waits until it stopped loading and
`document.fonts.status` is `loaded`, prints it to `out` as A4 portrait with
12 mm margins, backgrounds on and no header or footer, and returns once the
file is written or the print failed. It uses only the bindings wry (Tauri)
already pulls in: `webview2-com`, `objc2-web-kit` (with `objc2`,
`objc2-foundation`, `objc2-app-kit`, `block2`, `dispatch2`) and `webkit2gtk`
(with `gtk`, whose `glib` and `gio` re-exports it uses, and `javascriptcore-rs`), at the API levels wry uses.

| | Windows | macOS | Linux |
|-|---------|-------|-------|
| Webview | WebView2 (Edge runtime) | WKWebView | WebKitGTK 4.1 |
| Print call | `ICoreWebView2_7::PrintToPdf` with print settings in inches | `printOperationWithPrintInfo:` with a save-to-URL job, run modally for an offscreen window | `WebKitPrintOperation` with GTK's "Print to File" printer, PDF format, output URI |
| Thread | a new STA thread per print, pumping its own messages | the main thread, through the main dispatch queue | the GTK thread, through an idle source on the default main context |
| Window | hidden `STATIC` window, never shown | borderless `NSWindow`, never ordered in | `GtkOffscreenWindow` |
| Profile | its own WebView2 user-data folder, `<cache>/pdf-webview` | WebKit's default | WebKit's default |
| Page size | 8.27 × 11.69 in | 595.28 × 841.89 pt | `iso_a4` |

- **Off the UI thread.** The caller always waits on its own thread. On macOS
  and Linux the webview work runs on the UI thread in small steps (load,
  poll, print), so the UI keeps running between them; a call made on the UI
  thread itself is refused rather than deadlocking. The app's commands call
  the API from worker threads already.
- **Windows' own environment.** A second WebView2 environment on Tauri's data
  folder with other options fails, so the printer keeps its own folder under
  the cache folder (disposable data, ADR 0007).
- **The GTK file printer's name** is translated; the printer looks it up in
  GTK's own catalogue (`gtk30`) rather than hard-coding "Print to File".
- **Injection.** `fm-core` takes the printer as a dependency
  (`CoreApi::with_pdf_printer`). The app passes `fm_platform::pdf::system`.
  Without a printer (the CLI's direct link, which has no UI loop) a PDF is
  `Unsupported`: "PDF export needs the Font Manager app running."
- **Files.** `fm-core` writes the page to a temporary HTML file, has it
  printed to a hidden partial file in the target folder, and renames that over
  the chosen path only when the print succeeded and the file starts with
  `%PDF-`. Both temporary files are removed whatever happens, so a failed
  print leaves nothing at the chosen path and an existing file there as it was.
- **Tests.** `crates/fm-core/tests/specimen_pdf.rs` prints a fixture family on
  every CI runner and checks `%PDF`, the A4 pages and a font program naming
  each face's PostScript name. It has its own `main` (`harness = false`)
  because AppKit needs the process's main thread; `fm_platform::pdf::run_ui_loop`
  runs the OS loop there while the test works on another thread. Linux runs
  the Rust tests under `xvfb-run`.

## Consequences

- PDF output differs slightly per OS (pagination, font subsetting, PDF
  version), as each engine lays out and writes the page itself. The tests
  check the properties, not the bytes.
- Printing needs a display session: on Linux without one GTK does not start
  and the export fails with a message; the app always has one.
- macOS prints through an AppKit modal session for a hidden window; the
  progress and print panels are off, so the user sees nothing but the result.
- WebView2 starts a browser process per print (about a second on a warm
  runtime); printing is a user action, not a hot path.
