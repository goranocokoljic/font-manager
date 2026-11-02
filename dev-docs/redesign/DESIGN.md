# Font Manager — Design North Star

This file is the single source of truth for every UI/UX iteration. Read it fully before
changing anything. When a decision here conflicts with what is currently in the code, this
file wins.

## Who it is for

Graphic designers, UI/UX designers and video artists. People who live in Figma, Illustrator,
After Effects, Premiere and DaVinci. They judge software by taste in the first three seconds
and by speed every day after. They notice optical sizing, inconsistent corner radii, a hover
state that is 100 ms too slow, and a system font in the sidebar. They forgive missing
features; they do not forgive a tool that looks like it was built by someone who does not
care about type.

## The bar

Beat Typeface (macOS). Not "as good as" — this should be the font manager designers
screenshot and send to a friend. Amazing to look at *and* extremely functional. Every pixel
that is not a font is either neutral or gone.

## References — steal the idea, not the pixels

- **Typeface (macOS)** — preview-first list, body-text previews, detail list, font matching,
  "making font management fun". The baseline to beat.
- **Figma's font picker, Linear** — density that still breathes. Hairline separators, 4/8 px
  grid, 13 px UI type, keyboard-first, one accent colour.
- **Raycast** — the command palette as the main way to do things; instant filtering.
- **Google Fonts / Fontshare (web)** — "type your own text" as the hero control; filters as a
  quiet side panel, not a wall.
- **Fontbase, RightFont** — collections sidebar, online catalogues, drag-out.

## Principles (ranked — when two conflict, the higher one wins)

1. **Fonts are the UI.** Specimens are the only thing allowed to be loud. Chrome is quiet,
   neutral, low-contrast.
2. **Zero browser defaults.** No native `<select>`, checkbox, radio, range slider, button or
   scrollbar look anywhere. Every control is a designed component.
3. **Keyboard-first, mouse-complete.** Everything reachable from the keyboard; everything
   discoverable with the mouse.
4. **Progressive disclosure.** Power is one click or keystroke away, never on screen by
   default. Raw data (OpenType tags, hashes, file paths) lives behind an expander.
5. **Two surfaces, one design.** Dark and light are both first-class. Every token has both
   values; nothing is "dark-only".
6. **Instant.** Filtering, view switching, previews — nothing visibly loads. Virtualise
   everything that lists fonts.
7. **Honest rendering.** Never silently fall back to a system font for glyphs the font does
   not have. Show the gap.
8. **Both densities.** A gallery mode (big specimens, few per screen) and a tool mode (tight
   rows, 30+ families visible) — switchable, remembered, and each one properly designed.

## Current state — audit from screenshots and a first read of the code (1 Nov 2026)

### Keep (strong functional skeleton)
Five views (List, Table, Grid, Waterfall, Layout), the inspector with Info / Axes /
Features / Glyphs / Developer, the weight-distribution glyph in Table, the Layout studio with
presets and drag-to-resize blocks, per-font Unicode-block and language coverage, Compare,
Find similar, Collections and smart collections, Project sets, online catalogues, the
command palette, Problems/Duplicates detection.

### Fix
- **A seed of a design system, not a design system.** `styles/theme.css` has ten colour
  tokens (`--fm-bg`, `--fm-surface`, `--fm-accent`…), a light/dark switch via `data-theme`
  and a density switch via `data-density` — keep all three mechanisms. But there is no
  spacing, radius, type, shadow or motion scale; the UI font is `system-ui`; fourteen
  components still render a native `<select>`; buttons, checkboxes and sliders are browser
  defaults; borders are solid grey lines. Every element has the same visual weight, so
  nothing has hierarchy.
- **The filter wall.** Three-plus rows of chips above the list, including ~70 raw OpenType
  feature tags (`cv01`…`cv32`, `ss01`…). It eats ~150 px above the fold and is developer data
  shown to designers.
- **The preview toolbar.** Fourteen controls in one row (Sample, Language, Size, Auto size,
  Line height, Tracking, Metrics, Outline, Start/Center/End, Text, Background, Surface,
  Reset). The one control that matters most — the sample text itself — is not even a text
  field.
- **Grid cards truncate the pangram** ("The quick br"). A card must show something that
  fits.
- **List row metadata** (name, style count, licence badge) is tiny, and the badges look like
  buttons. Row actions ("Similar") appear as a stray button.
- **Sidebar** is a plain text list: no icons, no rhythm, "Smart" rendered as a pill after
  the label, a "Smart collections" header with nothing under it, "Problems" appearing twice
  (a smart collection with 0 and a section with 69), buttons (Rescan / Pause / Remove,
  New / New smart, Import set…) inline in the list.
- **Inspector › Info** is a raw key/value dump; the file hash and path sit at the same weight
  as Designer and Foundry. **Features** tab is a single toggle. **Glyphs** tab has the right
  content with a horizontal scrollbar inside the grid.
- **Find similar** is a strong feature with a weak surface. Results open as a split pane
  that halves the list; every result carries the same "Why:" sentence (same classification,
  matching x-height, similar width…), so the explanation says nothing; results are
  individual faces, so three Myriad Pro variants fill three rows; the score is a number
  with no visual weight.
- **Pairing Studio** opens as a separate full-window mode but uses a fraction of it: a
  narrow magazine column floats in ~80 % empty white. It is a pairing tool showing one font,
  with no suggestions and no second face in sight; the font choice per block is a small
  "Panel font" dropdown in the gutter, which is the single most important control on the
  screen and looks like a form field. Four stacked toolbar rows of text buttons (Add panel /
  Close, Layout select / Print, Save / Revert / Duplicate / Reset / History, family name /
  Lock width). Instructions live in a sentence at the bottom.
- **Empty states** are a sentence of grey text.
- **Status bar** permanently shows app, API and platform version strings.
- **Language filtering is invisible.** Per-font language coverage exists (Glyphs tab) but
  there is no way to filter the library by language or script. The "Language" selector in
  the preview bar only changes the sample.

## Design system rules

Establish these in the first iterations, then never violate them. If a rule here turns out
to be wrong, change the rule in this file *and* the code in the same iteration.

- **Tokens in one place.** Grow `src/styles/theme.css` (keep the `--fm-` prefix, the
  `data-theme` and `data-density` switches and the existing contrast tests): colour on three
  surface levels, spacing on a 4 px base, two radii (small, medium), type scale, shadows,
  motion durations, z-index. Components use tokens, never literal values.
- **UI typeface:** one neutral sans with good small sizes, bundled (Inter or equivalent),
  never the system font. Tabular numerals for all counts. UI scale: 11 / 12 / 13 / 15 / 20 px.
- **Theme:** dark by default on first launch; light equally finished. Three elevation levels
  of surface, not more. Borders are hairlines at low alpha, not solid grey lines.
- **One accent colour,** used only for selection, active state, focus and the primary action.
  Nothing else is coloured. Licence / format / variable badges are monochrome glyphs.
- **Specimen area:** neutral background, no border; preview colour follows the theme unless
  the user overrides it; previews honour the `font-feature-settings` chosen in Features.
- **Components** (`src/components/ui/`): `UiPopover` and `UiSelect` exist (iteration 3).
  The rest — Button, IconButton (with tooltip), Toggle, SegmentedControl, Slider (with value
  field), Chip, Badge, Drawer, Field, Tabs, EmptyState — are built **by the first visible
  change that needs them**, not ahead of time: the Filters drawer brings the Drawer and the
  Chip, the view switcher brings the SegmentedControl, the preview bar brings the Slider and
  the Field. Build once, reuse everywhere; when a later change needs a component that exists,
  it uses it and improves it in place. Grep for native controls before shipping.
- **Icons:** one set (Lucide), 16 px, 1.5 stroke.
- **Motion:** hover/active 120 ms, panel open/close 200 ms, easing `cubic-bezier(.2,0,0,1)`.
  Nothing bounces.
- **Focus:** a custom ring (accent, 2 px, 2 px offset) on `:focus-visible` only.
- **Shell:** sidebar 240 px, main, inspector 320 px; both side panes collapsible, with
  keyboard shortcuts, widths and open state remembered.
- **Density:** the existing `data-density` switch (Comfortable / Compact) swaps the spacing
  token set, not per-component overrides.
- **Localisation and direction:** every string goes through `i18n` (`t(...)`), never a
  literal; layout uses logical properties (`margin-inline-start`, not `margin-left`) so the
  RTL chrome keeps working — `styles/logical.test.ts` enforces this.
- **Cross-platform:** no Windows-only or macOS-only assumptions in CSS (scrollbars, fonts,
  title bar). Custom overlay scrollbars everywhere.

## Must not break

- **`CLAUDE.md`.** Its rules apply to every iteration, above all rule 1: no logic in the
  frontend. If a visual change needs a fact about fonts the API does not give, it is blocked.
- **The Playwright suite** (`pnpm test:e2e`, browser mode on the mock). It is the loop's
  safety net. A test may be *updated* when the UI it checks legitimately changed (a renamed
  button, a moved control); it is never deleted or skipped. The axe checks in `a11y.spec.ts`
  stay green in both themes. Vitest and `pnpm lint:js` stay green.
- All five views and everything they can do today, plus Quick Look (`QuickLookSheet`),
  Pairings (`PairingsPanel`), the Studio, Quick Collection, document fonts, palettes, i18n
  and the RTL chrome.
- Virtual-list performance with 1000+ families and 5000+ styles.
- Activation, collections, smart collections, tags, project sets, compare, find similar,
  command palette, problems/duplicates.
- The API boundary: `apps/desktop/src/api/` is the only place that knows about Tauri or
  the mock. UI iterations change views, components and stores; they never edit `api/`,
  `mock.ts`, generated types or Rust. An item that needs new data from `FmClient` is
  blocked — it gets written up as a follow-up, in terms of the interface, for a separate
  API task.
- Existing keyboard shortcuts (Ctrl+K, Ctrl+, , T).

## Opportunity backlog

Roughly ordered by impact. The iteration loop picks from here first, lowest unfinished
number that is not blocked. It may invent something not on this list when it is clearly
better, and must log why. Tick items in `ITERATIONS.md`, not here; the review pass
re-orders this list.

Items marked **(API?)** probably need something `FmClient` does not expose yet. The loop
builds whatever part is possible with today's data and logs the rest as a follow-up —
it does not extend the mock or the store to fake it.

### Foundation — done enough (iterations 2–3)
1. ~~Design tokens + theme engine + bundled UI font + ui/ Select.~~ Landed: the token file
   (`theme.css`), Inter, drawn baselines for every native control, `UiPopover`, `UiSelect`
   replacing all 22 native selects. **The gate is lifted.** The remaining `ui/` components
   are folded into the items that first need them (see the design-system rules). Do not
   spend another iteration on foundation work with no visible result.

### The three-second test (applies to every item from here on)
Every iteration must change something a designer would see in the first three seconds of
opening the app, or feel in the first minute of using it. If the iteration's "after"
screenshot of `list` is not visibly different from its "before", the iteration picked the
wrong thing. Items 2–9 are ordered by how much of the first screen they change.

### First screen
2. **Filters panel instead of the wall.** A `Filters · 2` button opens a left drawer with
   grouped facets: Classification, Weight / Width / Slant, Variable, Language & script,
   Format, Licence, OpenType features (human-named and grouped — "Small caps", "Tabular
   figures", "Ligatures", "Stylistic sets", "Case-sensitive forms"), Problems. Active filters
   appear as removable chips in one row above the list. Raw feature tags live only inside an
   "All features" expander in the drawer. Brings `UiDrawer` and `UiChip`.
3. **Preview bar redesign.** The sample text is a real text field, front and centre, with
   presets (Pangram, Alphabet, Numerals, Paragraph, saved texts) and per-language samples.
   Size slider plus the three or four most-used controls inline; everything else (tracking,
   line height, alignment, metrics, outline, colours) in a Preview options popover. State
   remembered per view. Brings `UiField`, `UiSlider`, `UiToggle`.
4. **Dark by default + the app shell.** First launch opens dark (Settings keeps system /
   light / dark). Sidebar / main / inspector as three panes with hairline dividers,
   collapsible and remembered; the view switcher as a `UiSegmentedControl`; status bar
   reduced to live state (active fonts, problems, `Ctrl+K` hint), version strings move to
   About.
5. **Sidebar redesign.** Sections with icons, counts right-aligned in tabular figures, smart
   collections as a visual subtype (not a trailing pill), Problems de-duplicated, section
   actions as `UiIconButton`s in the section header, sections collapsible, collections as
   drop targets. Brings `UiIconButton` with tooltip.

### Information architecture
6. **List rows.** Name and metadata in a quiet 12 px line with tabular counts; licence /
   format as monochrome glyph badges; row actions (Activate, Compare, Tag, Similar) appear on
   hover or focus, right-aligned; selection = accent bar + subtle tint; expanded family shows
   its styles as sub-rows.
7. **Grid cards never truncate.** Fit the sample automatically (a short word at large size,
   or auto-size the pangram to the card width); footer with name and style count; hover
   reveals the weight range.
8. **Inspector.** Info grouped and prioritised (Designer, Foundry, Licence, Styles first;
   file, hash, IDs under a "Technical" expander). Glyphs grid without inner scrollbars.
   Features with human names and a live preview of each switch.
9. **Find similar as a mode, not a split.** Results replace the list with a header
   ("Similar to ADLaM Display Regular · Esc to go back"), grouped by family with the closest
   face shown, score as a quiet bar, and the "Why" line showing only what *differs* from the
   source (e.g. "wider, lower contrast") — a line that is identical for every result is
   noise and is dropped. Source family pinned at the top for comparison.

### Functional features designers will love
10. **Language & script support as a filter facet.** The per-font coverage data already
   exists. "Supports Serbian (Cyrillic)", "Serbian (Latin)", "Greek", "Vietnamese", … with
   counts. Choosing a language also switches the pangram to that language's sample.
11. **"Renders my text" filter (API?).** Type anything — a brand name, a Serbian sentence — and
    filter to fonts that cover every glyph. In every preview, render missing glyphs as a
    visible tofu marker in the accent colour instead of a silent fallback.
12. **Quick Look, finished.** `QuickLookSheet` exists; make it the hero: Space opens a
    full-window specimen of the selected family (all styles, waterfall, paragraph, glyphs)
    on the specimen surface; Space or Esc closes; arrow keys move to the next family without
    closing; the sample text and size follow the list's.
13. **Keyboard model.** `↑`/`↓` or `J`/`K` move through the list; Space quick look; `A`
    toggle activation; `C` add to compare; `T` tag; `S` find similar; `1`–`5` switch views;
    `/` or `F` focus search; `[` `]` toggle sidebar / inspector; `?` opens the shortcut
    sheet. Shortcuts shown in tooltips.
14. **Weight scrubbing.** On variable fonts, drag horizontally on a preview to scrub the
    weight axis; axis value shown as a tooltip; release to keep.
15. **Context presets in Layout.** Website hero, Mobile UI (iOS sizes), Instagram post,
    Video title and lower third (1920×1080 safe area), Poster. Real layouts with the user's
    sample text.
16. **Compare: overlay mode.** Two faces superimposed, the second in the accent colour at
    50 % opacity, for x-height and width comparison — in addition to side-by-side.
17. **Pairing Studio + "Pairs with" (API?).** The studio is where designers will spend real
    time, so it should feel like a canvas, not a form: a paper surface at realistic page scale
    centred in the window with a neutral backdrop; roles instead of blocks (Headline, Deck,
    Body, Caption), each with a font picker that is a proper searchable select showing live
    specimens; a suggestions rail ("Try for Body: …" with the reason — contrast, x-height,
    width, era); lock a role and shuffle the rest; two or more panels side by side to compare
    pairings; export as PNG. `PairingsPanel` already exists in the inspector —
    redesign it as the same suggestions rail with one click to open a pairing in the studio.
    Toolbar collapsed to one row.
18. **Drag out (API?).** Drag a family from the list to drop its files into Explorer / Finder or
    another app; copy CSS / `font-family` name from the row menu.
19. **Problems & Duplicates triage view (API?).** Grouped, with a "keep best" suggestion and bulk
    resolve.
20. **Collections UX.** Drag families onto sidebar collections; colour per collection; a
    collection header shows a mini specimen strip of its fonts.
21. **Designed empty states.** Typographic, no illustrations, exactly one action that fills
    the state.
22. **First run (API?).** Detect system font folders, offer to add them, three-step welcome.
23. **Online catalogues (API?)** browsed with the same list UI; install from the row.

### Polish
24. Motion pass: list transitions, panel collapse, selection changes.
25. Custom overlay scrollbars on all platforms.
26. Hover / focus / active audit across every control.
27. Accessibility: contrast ≥ 4.5:1 for UI text in both themes; full keyboard reachability.
