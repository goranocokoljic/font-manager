# 0029 — Interface size: the webview's own zoom, and the fonts take it back

Status: Accepted (2026-11-28, E58.4)

## Context

E58 shows the interface at three sizes: 100, 125 and 150 per cent
(`fm_api::settings::InterfaceSize`, E58.5, with its `factor()`). Everything
grows but the fonts the user looks at: a 48 px specimen must stay 48 px on the
screen at every size, or the app lies about the font. Two ways were possible:

- **the webview's own zoom** — Tauri's `WebviewWindow::set_zoom`, which is
  `ICoreWebView2Controller::SetZoomFactor` (WebView2), `WKWebView.pageZoom`
  and `webkit_web_view_set_zoom_level` (WebKitGTK);
- **a CSS zoom on the root** — `html { zoom: z }`.

In both, a surface that shows a font must take the zoom back. E58.4 built a
test build with both ways and measured them.

## Decision

1. **The webview's own zoom.** The app sets the zoom of its window's webview
   to `InterfaceSize::factor()` — `set_zoom`, once the window exists and after
   each page load — and to 1 at the default size. No CSS zoom on the root. The
   zoom hotkeys of the webview stay off (`zoomHotkeysEnabled: false`, Tauri's
   default), so the user's size is the only zoom.
2. **Every surface that shows a font takes the zoom back**: its root element
   gets `zoom: calc(1 / var(--fm-interface-factor))`, inside `@media screen`
   only, so its content is laid out in true pixels. That rule is the class
   `fm-true-size` in `styles/theme.css`, the one shared piece (E58.7); the page
   sets `--fm-interface-factor` on the root before its first paint and on
   `interfaceSizeChanged` (`src/api/interfaceSize.ts`). The surfaces:

   | Surface | Root that takes the zoom back |
   |---|---|
   | A specimen on the wall and in the list (the sample of a row or a card) | the sample element |
   | The family page's hero | the hero sample |
   | The waterfall's 10 to 96 px ladder | the ladder |
   | A glyph cell and the glyph grid | the grid (the cell slider's sizes stay true pixels) |
   | Compare's overlay (HTML and SVG text) | the overlay box, the SVG inside it |
   | The Studio paper | the paper |

   E58.7 put the class on every surface of the app that shows a font: the
   list row's sample (the wall's list and waterfall, the family waterfall,
   the preview strips), the card's live text and thumbnail, the theme cards'
   small wall, the family page's hero, styles and ramp (`FamilySample`), text
   passages, waterfall, glyph grid and specimen, features, pairings and
   metrics drawing, Quick Look, Compare's stage and samples, Similar's rows,
   Compare copies and the Duplicates copy cards, the glyph compare stage, the
   Studio paper, the letters picker's example and the catalog's samples.
   Code that fits a specimen to the room the chrome gives it converts between
   the two spaces in `src/preview/trueSize.ts`.

   The chrome around them (labels, buttons, the row's height, the gaps) grows
   with the interface. A surface inside another that already took the zoom
   back does not take it back again.
3. **Print is laid out at 100 per cent.** The webview's zoom does not reach the
   print pipeline, so the take-back is screen-only; scoped to `@media screen`
   the Studio's PDF is the same at every size.
4. **Nothing else changes.** Popovers, tooltips, the text field menu
   (ADR 0025), the drag ghost (ADR 0017, ADR 0022) and the virtualized lists
   place themselves from `getBoundingClientRect` and pointer events in CSS
   pixels, which stay in one space under the webview's zoom; they need no code
   for the interface size.

## Why — the measurements

**On every system, by the CI matrix** (`crates/fm-platform/tests/interface_size.rs`,
an offscreen webview per OS, `fm_platform::webview_probe`): six surfaces at 125
and 150 per cent, both ways, measured as ink in the pixels the webview drew at a
scale of 1. Each control page without the take-back grew by the zoom (1.245 to
1.505×, CI run 46131007586), so the zoom happened.

| Surface (ink at 100 %, px) | Windows | macOS | Linux |
|---|---|---|---|
| Wall specimen, 40 px | 328 × 37 | 328 × 37 | 327 × 36 |
| Family hero, 96 px | 420 × 86 | 419 × 86 | 422 × 87 |
| Waterfall ladder, 10 lines | 571 × 493 | 571 × 492 | 570 × 492 |
| Glyph cell, 96 px box | 98 × 98 | 98 × 98 | 98 × 98 |
| Compare overlay, 120 px | 325 × 135 | 325 × 135 | 325 × 133 |
| Studio paper, 420 × 594 | 420 × 594 | 420 × 594 | 420 × 594 |
| Difference at 125 and 150 %, both ways | 0 × 0 | 0 × 0 | 0 × 0 |

Each waterfall line was the same at every size too. `innerWidth` of a 1000 px
view was 800 and 667 (666 on WebKit, macOS and Linux) under the webview's zoom, 1000 under the
CSS zoom. Both ways hold the fonts on all three systems.

**In the real app window on Windows** (WebView2, the E58.4 test build; input
through the browser's own input path):

| Check | 100 % | webview 125 % | webview 150 % | CSS 125 % | CSS 150 % |
|---|---|---|---|---|---|
| Tooltip: gap to its button, px | 5.6 | 6.0 | 6.3 | 4.1 | 72.8 |
| Popover: gap below its button / off its edge, px | 9.0 / 5.0 | 8.8 / 4.8 | 8.7 / 4.7 | 83.2 / 63.5 | 186.3 / 146.5 |
| Text field menu: from the pointer, px | 5.0, 9.0 | 4.8, 8.8 | 4.7, 8.7 | 151.3, 18.0 | 328.0, 30.0 |
| Drag ghost: drift from the pointer over 3 moves, px | 0 | 0 | 0 | 54 | 108 |
| Virtualized list: worst gap or overlap while scrolling, px | 0 | 0 | 0 | 0 | 0 |
| Studio PDF: page; text sizes | A4; 13 pt | A4; 13 pt | A4; 13 pt | A4; 13 pt | A4; 13 pt |

(Pixels are CSS pixels; under the webview's zoom one is `z` screen pixels, so a
gap of 5.6 at 100 % and 4.7 at 150 % is the same design gap, grown.) Under the
CSS zoom, `getBoundingClientRect` and the pointer's `clientX` are in different
spaces, so everything placed from one at the other lands off by the zoom; it
would need a correction in every positioning path. Under the webview's zoom
there is one space. Without the `@media screen` scope the Studio's PDF text was
10.40 pt at 125 % and 8.66 pt at 150 % (13 / z): the take-back printed while
the zoom did not.

The webview's zoom also makes `innerWidth` the room the interface really has,
so the panes that fold by themselves (E58.6) and every media query see the
smaller room without code, and `devicePixelRatio` grows with it, so a canvas
is drawn sharp.

## What the system draws, and does not scale

The app does not draw these, so the interface size does not reach them: the
native menus (the app menu on macOS, the tray menu), the tray icon, the
system's dialogs (open, save, print, the folder picker), the window's title
bar, the native drag image of a file drag (ADR 0022), and the native tooltip
of a `title` attribute. The OS's own text size or display scale is what sizes
them.

## What is given up

- The CSS zoom, and with it a size that could differ per pane.
- The webview's zoom is per webview: a second window must set it too. The
  offscreen webviews (the PDF printer, ADR 0013, and the probe) are other
  webviews and stay at 100 %.
- A page that renders a font at a size the user typed is in CSS pixels at 100
  % only inside a surface that takes the zoom back; a new surface that shows a
  font must be added to the table above.

## Open

The same checks by hand in the real app window on macOS and Linux (pointer
placement, drag ghost, list scrolling, text field menu, Studio PDF) are E58.11
(#726), a story for a person. Until it is done, those two rows of evidence are
the CI measurements above only.

## Applied (E58.7)

`fm_app_lib::interface_size`: the factor of the settings is read when the app
is set up, every webview window is zoomed in the setup hook before the main
window is shown, every page that starts or finishes loading is zoomed again
(the help window, a reload), and `Event::InterfaceSizeChanged` zooms every
window at once. The window keeps its size and position. Browser mode (the
mock) has no shell: `applyInterfaceSize` puts a CSS zoom on the root instead,
so the Playwright specs (`e2e/interface-size.spec.ts`) see the same sizes; the
misplacement of popovers under a CSS zoom (above) is a browser-mode limit only.
The screenshots of UI stories stay at the default size.

## Tests

- `crates/fm-platform/tests/interface_size.rs` (all three CI runners, Linux
  under `xvfb-run`): one specimen at each size in the chosen way, and the six
  surfaces both ways, each with its control. It prints every number
  (`interface-size …` lines in the CI log). It has its own `main`
  (`harness = false`), as AppKit needs the main thread.
- `scripts/bench` `cold_start_sized`: the cold start with the interface size
  at 125 % in `config.json`, under the same 1 s budget.
- The test build and the Windows window checks are on the branch
  `E58.4-test-build` (`apps/desktop/e2e-window/`, never merged).
