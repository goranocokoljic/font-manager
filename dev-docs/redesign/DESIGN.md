# Font Fu — Design North Star

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

1. **Fonts are the UI.** Specimens are the only thing allowed to be loud, and they read as
   ink on paper: black type on a white specimen surface in the primary (light) theme. Chrome
   is quiet, neutral, low-contrast.
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

## Current state — the boards are the target (3 Nov 2026)

The audit of the old UI (1 Nov 2026) is retired. The target is the concept canvas, exported
as 18 PNGs in `dev-docs/design/redesign/` (see its README.md for the board-to-story table).
A UI story is reviewed against its image, not against the previous screenshot.

What the boards leave open — decide from the rules below, not from the images:

- The dark theme of every board (the boards show the light theme only).
- Compact density.
- Narrow windows (the boards are 1440 px wide).

## Design system rules

Establish these in the first iterations, then never violate them. If a rule here turns out
to be wrong, change the rule in this file *and* the code in the same iteration.

- **Tokens in one place.** Grow `src/styles/theme.css` (keep the `--fm-` prefix, the
  `data-theme` and `data-density` switches and the existing contrast tests): colour on three
  surface levels, spacing on a 4 px base, two radii (small, medium), type scale, shadows,
  motion durations, z-index. Components use tokens, never literal values.
- **UI typeface:** one neutral sans with good small sizes, bundled (Inter or equivalent),
  never the system font. Tabular numerals for all counts. UI scale: 11 / 12 / 13 / 15 / 20 px.
- **Theme: paper first.** The default follows the system, and the light theme is the
  primary one: type is judged as it will be read, black on paper. The light specimen
  surface is paper white (not grey, not warm-tinted), specimens are near-black at full
  contrast, and the chrome around them is the quiet thing. Dark is equally finished for
  people who want it, never the showcase. Three elevation levels of surface, not more.
  Borders are hairlines at low alpha, not solid grey lines. No iteration changes the
  default theme.
- **One accent colour, per theme (owner's decision, 3 Nov):** light `--fm-accent: #C56EB2`,
  dark `--fm-accent: #AB4B96`. Used only for selection, active state, focus and the primary
  action. Neither value is a text colour on its own: on white the light accent is 3.4:1, so
  accent-coloured *text* and *links* stay near-black / `--fm-fg`, and anything drawn *on* the
  accent uses a paired token `--fm-on-accent` (light: `#141413`, dark: `#ffffff` — both
  ≥ 4.5:1). Rings, fills, knobs, badges and the pressed state take the accent; words do not.
  As a focus ring the dark accent must still reach 3:1 on every surface, so dark
  `--fm-surface-2` is `#262628`, the lightest value that does.
  Nothing else is coloured. Licence / format / variable badges are monochrome glyphs.
  Status colours (`--fm-danger`, `--fm-warning`) mark problems only — a broken file, an
  unsupported format, a conflict. A good state ("OK", "Open license") is neutral, never
  green; `--fm-success` stays in the token file for the contrast tests but no badge uses
  it. The one exception is Compare's overlay, which needs two data colours
  (`--fm-compare-0/1`) to tell faces apart. (Amended in review 5.)
- **Dialog scrim:** `--fm-scrim`, never a literal. (Added in review 5.)
- **Specimen area:** neutral background, no border; preview colour always follows the theme
  (no override, see Preview options); previews honour the `font-feature-settings` chosen in
  Features.
- **Preview options (board 12, E54.7, E54.8):** one `UiPopover` under the toolbar's options
  button, with a title, a Reset text button and a small live specimen at the top. Three groups,
  each under a small-caps heading and divided by a hairline, in this order:
  **Text** — Sample (`UiSelect`) and Align (`UiSegmentedControl`, three icon segments);
  **Size and spacing** — Size (px), Auto size (`UiSegmentedControl`: Off / x-height / Cap
  height), Line height, Tracking (em); **Guides** — Metric lines and Outlines, each a toggle
  with a one-line description under its label. Size, Line height and Tracking are sliders with
  a `UiNumberField` at the row's end (the unit, if any, after the field); the slider and the
  field show the same value, and the field saves on `commit`. A control that has no effect in
  the current view mode stays in its place, disabled and faded (slider, field and label), with
  one quiet line under it that says why ("No effect while the preview is a single line."); it
  is never hidden, so the popover does not change height between view modes. There are no
  colour controls at all: no Text colour, no Background, no Surface. Preview colours come from
  the theme: every preview draws in the theme's text colour on the theme's surface. The slider
  knob follows the one-accent rule above; the blue knob in the exports is the canvas browser's
  default, not a second colour.
- **First-run setup (board 13, E55):** shown in the main window in place of the library while a
  setup question is open; never a second window. Two panes: the left one (648 px, `--fm-bg`)
  asks, the right one (`--fm-surface`) shows what the answer leads to: the fonts found so far,
  the folder tree an install lands in, what an update check does. Three steps at most (Font
  folders, Online catalogs, Updates) in a step indicator that counts only the steps shown; a
  step that already has an answer is not shown. One primary action per step ("Add folder…"
  while the list is empty, then Continue, Finish on the last); Back is a quiet button; "Skip
  for now" appears only on the folders step while no folder is added, and records the step as
  skipped. Folder and catalog rows are the Sources cards; switches, the select and the radio
  list are the app's own controls. Reopened from Settings it shows every step, the step names
  become links and a close button appears. Its head carries the interface size (E58.10): a
  segmented control of three, each the letter A at its size, on every step. It is in the head
  and not a step, because the user must be able to read the setup itself; a step would come
  too late or delay the first question.
- **Duplicates (board 14, E56):** a full view of its own, like Studio, never a section of
  Problems: duplicates are housekeeping, not faults. Its count is always the neutral count
  style, never the red dot, and the Problems total does not include it. Layout as in Settings:
  a 232 px navigation on `--fm-surface` with "Library" to go back, then Overview, Folders,
  Sets, Plan, History and Hidden copies with their counts; at its foot the languages the user
  works in and the link to the rules. Ways in (amended 26 Nov 2026): the one permanent way in
  is "Duplicates" in the native File menu, on all three systems; the label has no ellipsis
  because it opens a view. There is no row for it in the sidebar: the sidebar is for what is
  used every day, a count that never goes away nags, and a row inside the Sources group
  disappears when the group is folded. The app points to the module when there is something
  to do: one notice after a scan that finds copies (the app's notice, once per scan, never
  opening the module by itself), a quiet "N copies" mark beside a family's style count plus
  one line under Info > Files, the line in the Problems view, and two command palette entries.
  **Overview** leads with the one decision that is safe in bulk (identical copies, primary
  button "Review the plan"), then the levels still to decide, then the top folder pairs.
  **Folders** shows pairs of folders with the share as a hairline bar and "Keep this side" on
  each side; a side that may not be removed (a system folder, a source root, the library
  folder) says why and can only be kept. **Sets** is a queue: the list on the left (320 px),
  one set on the right, one card per copy. A copy card shows a specimen in the copy's own
  face, in a sample of the user's languages; a glyph the copy lacks is the accent tofu marker
  (E43.4), never a fallback. Under it: version, format, year, glyph count, the path in the
  mono face, the user's languages each with a check or "missing", and what the user put on
  that copy. The recommended copy carries the accent ring and a "Keep" badge, and its reason
  is one sentence ("covers Serbian (Cyrillic); the other copies do not"). The other copies get
  a `UiSegmentedControl` with the three outcomes, in these words: the trash of this system
  ("Recycle Bin" on Windows), Hide, Keep. A copy that cannot move (active, in a system folder)
  says why in place of the control. For the whole set: "Keep all" and "Not duplicates"
  (quiet), and one primary action, "Add to plan and go on". Nothing the language guard
  protects is ever preselected for removal. **Compare** puts two copies side by side with the
  differences in this order: languages, localized forms, glyphs (shown as glyphs), features,
  metrics (with "text may reflow"), format. **Plan** is the only place files move from: totals,
  what goes to the trash by folder, what stays and why, the metadata that moves to the kept
  copies, and each conflict as a two-way choice; its primary button names the number of files
  and the trash. **Applying** shows progress, the current file and Cancel. **History** lists
  cleanups as sessions with "Undo all" and per-file undo; **Hidden copies** lists hidden files
  ("Show again") and remembered decisions ("Take back"). **Rules** shows the languages as
  chips, the language guard as a fixed line above the list (always on, not a row), the rules
  in order with a drag handle and a switch, "Path, A to Z" fixed last, and a preview of how
  many recommendations change before Save. The module uses no control the rest of the app
  does not have, and no colour but the accent; a caution (metrics changed, a file that cannot
  come back) is `--fm-warning` only where a file is at risk.
- **Settings (boards 11 and 15, E57):** one shell for all eight sections: a 232 px navigation
  on `--fm-surface` with the title and the sections (icon and name), and beside it the panel:
  the section's heading, one line under it, then its groups, at most 760 px wide (Sources
  keeps its 920). No top bar: the view closes with the x icon button in the panel's top corner
  and with Esc. A section is built from three patterns and nothing else. **Choice cards**
  (`UiChoiceCards`) for one of a few: each card has a title, at most one line, and where it
  helps a small figure; the chosen card carries the accent ring and the filled dot; two columns
  for two choices, three for three short ones, one column when the lines are long (the update
  levels). **Setting rows** (`UiSettingRow` in a `UiSettingGroup`): a card of rows, each with a
  title, at most one line under it, an optional value in the mono face (a path, an address),
  and exactly one control at its end: a `UiSwitch`, a `UiNumberField` with its unit after it,
  a `UiSelect`, or one or two quiet buttons. A group may have a small-caps heading and one
  note above it. **Cards for things**: a catalog is a card like a source card (mark, name, one
  line, count, switch); a theme is a `ThemeCard`; the snippets for AI assistants are one block
  with a segmented control per client, the snippet in the mono face on `--fm-surface` and
  Copy. No native radio button, checkbox or select anywhere; a hint is always the row's or the
  card's one line, never a paragraph between controls. A switch saves at once; a number saves
  on `commit`. A secret (the token) is dots until Show. About opens with the logotype and the
  version in one quiet panel; the state of updates is the title of its row. Nothing here is
  coloured but the accent on the chosen card and the switch that is on, and a failed change
  (the OS refused "Start at login"), which the row says in `--fm-danger`, not a toast.
- **Languages (board 15, E58):** one list, "Languages I work in", kept under Settings >
  Languages and used everywhere. The core answers every list of languages in one order: the
  user's languages in the user's order, then the most used ones not picked
  (`data/charsets/order.json`), then all others from A to Z; while none is picked, the
  system's language leads. No component sorts languages. The section shows the list as rows
  to reorder (handle, number, name, the language's distinctive letters, x to remove) with
  "Add a language", and under it "Then the rest": what completes the first ten, and what is
  behind "Show more". The filter drawer shows the first ten of the order and any chosen
  language, as before; a menu of languages draws a hairline after the user's own. Duplicates
  reads the same list and its module links here; the app's own language is "Language of the
  app" under Appearance, never "Language" alone.
- **Interface size (E58):** three sizes, Default, Large (125%) and Extra large (150%), chosen
  as choice cards under Appearance. A size enlarges everything the app draws except the fonts
  on show: a preview, a specimen, the waterfall, a glyph cell, Compare and the Studio paper
  are drawn at exactly the size the user set, at every interface size. Every such surface
  takes the zoom back through the one shared piece the ADR "Interface size" names; a surface
  that scales a specimen by a rule of its own is a bug. The chrome around a specimen scales
  with the interface (a card's label and actions), so a wall shows fewer columns at a larger
  size. A larger interface is a smaller window: at 150% the smallest window has the room of
  667 x 467, so the inspector, then the sidebar, fold by themselves when there is no room
  and come back when there is, without touching what the user chose by hand. Boards and the
  screenshots of UI stories are at the default size.
- **Narrow windows (E58.6):** every view is usable down to 667 x 467 CSS px (the smallest
  window at 150%): no horizontal scroll bar on the page, no control cut off, a top bar wraps
  its controls onto a second row, and a wide table scrolls inside its own box. The panes fold
  by the one rule in `stores/layout.ts` (`panesShown`): the main pane keeps `MAIN_ROOM`
  (480 px); without it the inspector folds first, then the sidebar. A fold by room is never
  saved: `sidebarOpen` stays the user's choice, the sidebar button shows what is on screen,
  and pressing it without room shows the sidebar for a look until the window changes. The
  shells of Settings, Duplicates and Problems keep their 232 px navigation at every width.
  Below 1000 px (CSS `@media (max-width: 999.98px)`) side-by-side content
  stacks: Compare's Grid shows one face per row, and the Studio's compared panels stack one
  above the other, each with its roles rail above the page. At 1000 px and wider nothing
  changes.
- **Close (E57.8):** a full view or a panel closes with `UiIconButton` (icon `close`) at the
  end of its bar or in its top corner at the end of the line, named for what it closes, with
  Esc in its tooltip where Esc closes it. A text button "Close" in a top bar or a panel head
  is a bug. A dialog's footer keeps text buttons, and a button that ends a flow with another
  word (Cancel, Later, Done) is not a close button.
- **Problems (board 16, E57.9):** the shell of Settings: the navigation lists "All" and the
  groups with their counts, a group with nothing in the muted colour. The panel's heading is
  the count in words, then the problems grouped under small-caps headings, each group with
  one line that says what it is. A problem is a row, never a table row: an icon tile in the
  warning tone, the name with the kind beside it, the sentence fm-api gives, the path in the
  mono face, and one action shown on hover and focus. A sidecar conflict shows both values
  side by side, each with its own button. The status colour is the icon tile's only; the
  empty state ("All good.") is neutral, never green. Duplicates are not problems: one quiet
  line at the foot of the panel counts the fonts with copies and opens the module.
- **Letters (match by letters, board 8, E49):** the letters in the preview field are the
  controls: a letter that has traits carries a hairline underline, a letter with a value
  chosen carries an accent underline, and the field's right edge says what to do ("click an
  underlined letter for its shape"; "R and O narrowed · click a letter to change"). A click
  on a letter opens an ordinary popover under it with the values drawn as glyphs at specimen
  size, the library's count under each ("Straight leg · 312 families") and Any; one choice
  per letter, and choosing again replaces it. A chosen value is an ordinary chip in the
  filter row ("R leg · straight ×"), removed like any other chip, and the count line names
  it ("41 families · Sans serif · R leg straight · O round"). The cards never carry trait
  UI: they show the Letters sample (ROag) in each family's own face and nothing else. A
  family that lacks one of the letters shows the tofu marker in the accent colour in that
  slot (E43.4), never a fallback glyph, and no trait on that letter matches it.
- **Components** (`src/components/ui/`): `UiPopover`, `UiSelect`, `UiDrawer`, `UiIcon` (+ `icons.ts`),
  `UiIconButton`, `UiSegmentedControl`, `UiBadge` (tones quiet / accent / danger / warning)
  and `UiChevron` exist (review 10); `UiSegmentedControl` takes icon or text segments and a `wrap`
  flag, and its track always hugs its segments (iteration 82, review 85), plus `UiFoldButton` with `components/useFolded.ts`
  (review 35) and `UiEmptyState` (review 45); the Settings patterns (E57.3) are `UiChoiceCards`
  (a radio group drawn as cards, one tab stop, arrows move the choice, one to three columns),
  `UiSettingGroup` (a card of rows under an optional small-caps heading and note) and
  `UiSettingRow` (title, one line, an optional mono value, one control that the row labels and
  describes through its `control` slot props, and `error`, a failed change said on the row and
  added to the control's description, E57.4, and `live`, a title that is a state and is
  announced once as a status, E57.7); a local copy of a choice card or a setting row
  is a bug; every Settings section is a component of its own under `components/settings/`,
  its head `SettingsSection` (Sources keeps its own);
  every inspector tab head is `InspectorTabHead`;
  a component that shows one face in its own font holds it with `preview/useHeldFace.ts`, not a
  local acquire/release copy (review 50). Every number field is `UiNumberField`; a caller that
  saves listens to `commit` (Enter, blur, − / +), never to every keystroke or a timer (review 60). Every
  text button is `UiButton` (iteration 61): quiet, an accent tint under `--fm-fg` text while pressed or expanded; its hover
  tint and disabled fade come from the global `button` baseline in `theme.css`, so neither
  `UiButton` nor a caller adds a hover background of its own (a second tint doubles it), and a
  local `.button`/`.toggle` text-button class is a bug (review 65). Every icon button is
  `UiIconButton` under the same baseline rule: no own hover background; a pressed toggle
  (`aria-pressed`) keeps the `--fm-hover` tint, an open one (`aria-expanded`) takes the accent tint; a
  local `.on` class or `.toggle.icon` copy is a bug (review 70). Every badge and every expand/fold twisty uses them; a local `.badge` class, a
  ▸/▾ text glyph or a sidebar heading with its own fold markup is a bug. Text for screen readers only uses the
  global `.sr-only` class in `theme.css`; a local copy of that rule is a bug (review 15). The rest — Button, Toggle, Slider
  (with value field), Chip, Field, Tabs, EmptyState — are built **by the first visible
  change that needs them**, not ahead of time: the Filters drawer brings the Drawer and the
  Chip, the view switcher brings the SegmentedControl, the preview bar brings the Slider and
  the Field. Build once, reuse everywhere; when a later change needs a component that exists,
  it uses it and improves it in place. Grep for native controls before shipping.
- **Icons:** one set (Lucide), 16 px, 1.5 stroke.
- **Row actions (E54.9):** the actions a hovered or focused list row, wall card or table row
  shows (Activate, Compare, Similar and the rest) are `UiIconButton size="lg"`: a **32 px
  target around a 20 px glyph** (the 16 px drawing scaled). They sit in the row's existing
  space — a list row is at least 32 px high, a table row is 32 px — so they never change a
  row's height or move its text. Every other icon button keeps 28 px / 16 px (`sm` 24 px).
- **Tooltips (E54.9):** one primitive, `UiTooltip`, owned by `UiIconButton`; no icon button
  sets a native `title`. It shows the action's name and its shortcut (a quiet `kbd`), comes
  up 300 ms after the pointer rests (at once while another one was up a moment ago) and at
  once on a keyboard focus, and hides on a press, on Esc, on leave and on blur. It sits above
  the button (below without room), never over it, on `--fm-surface-2` with the popover's
  border and shadow, and fades at `--fm-motion-fast`. It is the button's `aria-describedby`.
  Native `title` tooltips were not dependable on row actions: the webview draws them, not
  the app, so whether and when one shows differs per webview (WebView2, WKWebView,
  WebKitGTK) and it needs a long rest of the pointer; the actions are visible only while
  their row is hovered, inside a draggable row; and a keyboard focus never shows one.
- **Similar's icon (E54.9):** a bold "≈" — two waves, 2 px stroke — chosen over two
  overlapping letter outlines, which blur into one shape at 16 px. The old lens with a small
  "≈" read as Search. The same icon is used in list rows, on the family page, in the
  command palette and in the shortcut sheet (`ACTION_ICONS` in `stores/commands.ts`).
- **Motion:** hover/active 120 ms, panel open/close 200 ms, easing `cubic-bezier(.2,0,0,1)`.
  Nothing bounces.
- **Hover:** every hover tint is `--fm-hover` (a low-alpha overlay that reads on any surface).
  A surface token used as a hover is a bug — `--fm-surface-2` is white in light theme, so it
  vanishes on a white pane (review 30).
- **Focus:** a custom ring (accent, 2 px, 2 px offset) on `:focus-visible` only.
- **Shell:** sidebar 232 px, main, inspector 320 px; both side panes collapsible, with
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
opening the app, or feel in the first minute of using it, and open before/after shots of a scene
that shows it. (Amended in review 80: the `list` shot has not moved since review 70 — the first
screen is mature — so "the `list` shot or the scene of the view the change touches" is the test;
a change no scene shows adds that scene first.) Since review 80 list rows carry Activate, Compare
and Similar on hover (6), the Axes instances and the Compare size are text segments (16), and the
Studio lock ends at the text column (17). Next: scrubbing in Grid cards and Quick Look with a
`scrub` scene (14); arrow-key roving in `UiSegmentedControl`, then Compare's mode radios onto it
(16); Layout presets with real contexts (15) — the first new feature in a while.
Every feature adds a scene to `shots.mjs` in the same
iteration; a change that alters what `?face=` shows checks every scene that uses it.

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
4. **App shell (rest).** Toolbar, view switcher and status bar are done (iterations 7, 11).
   Left: sidebar and inspector collapsible with `[` `]`, widths and open state remembered.
   The toolbar sidebar toggle is done (iteration 39); it shows only on the All fonts toolbar,
   not in Table-only or the Studio. The default theme stays `system` (owner's decision:
   fonts are judged on paper, so light is the primary theme — see the design-system rules);
   nothing left here is about the theme.
5. **Sidebar redesign.** Sections with icons, counts right-aligned in tabular figures, smart
   collections as a visual subtype (not a trailing pill), Problems de-duplicated, section
   actions as `UiIconButton`s in the section header, sections collapsible, collections as
   drop targets. Done: `UiIconButton`, header icons, the collection "…" menu, catalog power
   icons, the smart sparkle, Sources row actions on the row (iterations 8, 12, 13, 19).
   Project sets rows export from a hover icon in the count's cell (iteration 22). Every group
   folds with `UiFoldButton` + `useFolded`, remembered (iterations 33–34, review 35).
   Section icons and "All fonts" staying visible when Collections folds are done (iteration
   37). Left: the Problems heading is both a fold and a link to the Problems view; no scene
   for folded groups.

### Information architecture
6. **List rows.** Name and metadata in a quiet 12 px line with tabular counts; licence /
   format as monochrome glyph badges; row actions (Activate, Compare, Tag, Similar) appear on
   hover or focus, right-aligned; selection = accent bar + subtle tint; expanded family shows
   its styles as sub-rows. `UiBadge`, `UiChevron` and the hover "Find similar" icon are done
   (iterations 9, 14, review 10); the chevron sits on the sample line in both densities,
   sub-rows hang off a hairline guide, the pane has no ring (iteration 21, review 25).
   The cursor row is a 2 px bar plus a hairline ring, rows tint on hover (iteration 29,
   review 30). `?face=` selects the list row (iteration 42); revealing a one-style family keeps
   it folded (iteration 46); a one-style family has no chevron and does not open in any view
   (iteration 51, review 55: `aria-expanded` follows the same `single` flag); sub-rows no
   longer repeat "Variable" (iteration 56). Activate, Compare and Similar on hover and on the
   cursor row (iteration 81). Left: a Tag action; Activate shows no on/off state on the button.
7. **Grid cards never truncate.** Done in iteration 16 (the sample ends on a whole word);
   style cards are solid and every card tints on hover (iteration 47); hover or the cursor
   swaps the style count for the weight range, with `grid-hover` and `grid-expanded` scenes
   (iteration 66); the name comes first and is never cut by the count or badge (iteration 77).
   Left: on a narrow card the Variable badge wraps out of sight — a quiet glyph that always fits.
8. **Inspector.** Info, Glyphs, the Features switches and one shared `InspectorTabHead` are
   done (iterations 17, 26–28, 43, reviews 20, 30, 45); each Features switch shows a sample in the
   face with the feature on (iteration 49), beside the same text with it off (iteration 54); the
   Filters drawer names features ("Small capitals", "Stylistic set 1") from a static table of
   registered tags (iteration 53). Left: stylistic sets get a generic sample — a real one is
   **blocked (API)** (no per-set glyph data); the off/on pair has no visible "off"/"on" cue; the
   drawer's names (`chips.featureNames`) and the inspector's (`FeatureInfo.name` from fm-core)
   differ for some tags ("Glyph composition" vs "Glyph composition and decomposition").
9. **Find similar as a mode, not a split.** Results replace the list with a header
   ("Similar to ADLaM Display Regular · Esc to go back"), grouped by family with the closest
   face shown, score as a quiet bar, and the "Why" line showing only what *differs* from the
   source (e.g. "wider, lower contrast") — a line that is identical for every result is
   noise and is dropped. Source family pinned at the top for comparison. Done (iterations
   18, 24). Left: a family row could expand its other styles inline.

### Functional features designers will love
10. **Language & script support as a filter facet.** The per-font coverage data already
   exists. "Supports Serbian (Cyrillic)", "Serbian (Latin)", "Greek", "Vietnamese", … with
   counts. Choosing a language also switches the pangram to that language's sample. Done
   (iteration 36, review 40: chips sit right after Class). Left: per-language counts and
   script grouping are **blocked (API)** — `FmClient` gives no count per language.
11. **"Renders my text" filter (API?).** Type anything — a brand name, a Serbian sentence — and
    filter to fonts that cover every glyph. In every preview, render missing glyphs as a
    visible tofu marker in the accent colour instead of a silent fallback.
12. **Quick Look, finished.** Core done (iteration 32, review 35: full window, arrows move,
    `quicklook` scene). Left: glyph grid and axis sliders in the sheet; size and sample follow
    the current view, not only All fonts. Original brief: `QuickLookSheet` exists; make it the hero: Space opens a
    full-window specimen of the selected family (all styles, waterfall, paragraph, glyphs)
    on the specimen surface; Space or Esc closes; arrow keys move to the next family without
    closing; the sample text and size follow the list's.
13. **Keyboard model.** `↑`/`↓` or `J`/`K` move through the list; Space quick look; `A`
    toggle activation; `C` add to compare; `T` tag; `S` find similar; `1`–`5` switch views;
    `/` or `F` focus search; `[` `]` toggle sidebar / inspector; `?` opens the shortcut
    sheet. Shortcuts shown in tooltips. Done (iteration 31, review 35); the Grid opens and closes
    a family with Alt+→/← (iteration 74), listed in the `?` sheet (review 75). Left: shortcuts in
    the tooltips of the toolbar buttons; a test for J/K and the sheet.
14. **Weight scrubbing.** On variable fonts, drag horizontally on a preview to scrub the
    weight axis; axis value shown as a tooltip; release to keep. Done in `FontPreviewRow`
    (iteration 73; review 75: no text selection while dragging, direction read once per drag).
    Left: Grid cards and Quick Look do not scrub; other axes (wdth, opsz) with a modifier; a
    Playwright drag test with a real pointer; a `scrub` scene (hold the pointer mid-drag).
15. **Context presets in Layout.** Website hero, Mobile UI (iOS sizes), Instagram post,
    Video title and lower third (1920×1080 safe area), Poster. Real layouts with the user's
    sample text.
16. **Compare: overlay mode.** Two faces superimposed, the second in the accent colour at
    50 % opacity, for x-height and width comparison — in addition to side-by-side. The overlay
    existed; the sheet is now centred, quiet and draws at twice the list size, the overlay line
    centred on its stage (iterations 71–72, review 75; `compare`, `compare-overlay` scenes).
    The Axes instances are a text `UiSegmentedControl` (iteration 82) and the sheet has a size
    control, Auto / 48 / 72 / 96 (iteration 83; review 85: the local mode radios share its segment
    height). Left: `UiSegmentedControl` arrow-key roving and a radio role, then the mode radios
    onto it (one look, not two side by side); the size choice is not remembered; Pairings under
    the sheet is a sentence when empty.
17. **Pairing Studio + "Pairs with" (API?).** The studio is where designers will spend real
    time, so it should feel like a canvas, not a form: a paper surface at realistic page scale
    centred in the window with a neutral backdrop; roles instead of blocks (Headline, Deck,
    Body, Caption), each with a font picker that is a proper searchable select showing live
    specimens; a suggestions rail ("Try for Body: …" with the reason — contrast, x-height,
    width, era); lock a role and shuffle the rest; two or more panels side by side to compare
    pairings; export as PNG. `PairingsPanel` already exists in the inspector —
    redesign it as the same suggestions rail with one click to open a pairing in the studio.
    Toolbar collapsed to one row. Done: the block font fields are quiet name tags beside the
    block (iteration 44, review 45); Save/Revert/Duplicate/Reset/History are quiet buttons in the
    panel bar, and the Layout view's bar uses the same quiet buttons (iteration 48, review 50).
    Two bars now: Close, Print and close-panel are icon buttons, the gesture sentence opens
    from the info button (iteration 52, review 55); "Add panel" is a plus icon (iteration 67).
    The Layout view's help is behind an info button and "Lock width" is a `width` icon in its
    head (iteration 76, review 80: the help line aligns with the head). The Studio lock ends at the
    locked text column, from the same tracks as the page (iteration 84, review 85). Left: options
    in their own face; the suggestions rail.
18. **Drag out (API?).** Drag a family from the list to drop its files into Explorer / Finder or
    another app; copy CSS / `font-family` name from the row menu.
19. **Problems & Duplicates triage view (API?).** Grouped, with a "keep best" suggestion and bulk
    resolve.
20. **Collections UX.** Drag families onto sidebar collections; colour per collection; a
    collection header shows a mini specimen strip of its fonts.
21. **Designed empty states.** Typographic, no illustrations, exactly one action that fills
    the state. `UiEmptyState` is done (iteration 41); the sidebar Tags empty is a quiet title
    plus a key chip (iteration 64, review 65). Left: Layout, Catalog and Quick Collection empties
    are still plain text; no scene for "no matches" or "empty library"; an empty collection shows
   "Clear filters" with no filter chosen (`hasFilters` counts the collection — a store fix).
22. **First run (API?).** Detect system font folders, offer to add them, three-step welcome.
23. **Online catalogues (API?)** browsed with the same list UI; install from the row.

### Polish
23a. **Accent swap.** Move `--fm-accent` to the owner's colours (light `#C56EB2`, dark
    `#AB4B96`), add `--fm-on-accent`, and route every place that puts text on the accent
    (primary button, count badge, pressed chip, toast) through it; accent-coloured text and
    links go back to `--fm-fg`. Contrast tests updated to the new pairs; both themes, both
    densities, every scene opened.
23b. **Themes (later).** A small set of named themes the user picks in Settings, Slack-style:
    each is one token set (accent + on-accent + surfaces), light and dark variants; the
    current pair is "Paper". Needs an API default and a settings field — write up as a
    follow-up for the API task; the UI side is a theme picker that previews the specimens.
24. Motion pass: list transitions, panel collapse, selection changes.
25. Custom overlay scrollbars on all platforms.
26. Hover / focus / active audit across every control, and the native-input sweep.
    Done: Settings nav and captions (iteration 38), Features knob and note fields (26, 28),
    `UiNumberField` in the license form, Settings › Advanced and the Preview bar (iterations
    57–59, review 60: saves on `commit`). One quiet `UiButton` for every text button in the
    toolbar, Preview bar, inspector heads (Copy CSS, Reset, Developer copies), Layout and Studio
    bars, Settings, Problems and Conflicts (iterations 61–63, review 65); toolbar toggles on
    `UiIconButton`, Quick Collection and Catalog on `UiButton` (iterations 68–69, review 70).
    Left, in order: the Settings nav shows the focus ring on load (since review 40);
    `type="number"` in BlockProperties, SmartRuleRow, GlyphCompare; the tags `datalist`; the
    bordered dialog buttons (CatalogDownload/Install, LocalApiSettings); the Filters drawer's local
    `.chip`/`.link` buttons; Catalog "Install" not marked as the primary action (Catalog rows and Axes
    done in iterations 78–79; quiet buttons in a column pull back by their own padding, review 80);
    a designed date picker
    (purchase date is a plain text field with a `YYYY-MM-DD` hint); a `preview-options-open`
    scene.
27. Accessibility: contrast ≥ 4.5:1 for UI text in both themes; full keyboard reachability.
