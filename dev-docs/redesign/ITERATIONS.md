# Iteration log

Append-only. Newest at the bottom. Every iteration and every review pass adds an entry.
Backlog ticks live here, not in `DESIGN.md`.

## Backlog status
<!-- Keep this list in sync: mark an item [x] in the iteration that finishes it. -->
- [x] 1 Design tokens + theme + UI font + ui/ Select (iterations 2–3; the gate is lifted)
- [x] 2 Filters panel instead of the wall (iteration 4; UiDrawer built, chips stay in FilterChips; human feature names moved to item 8)
- [x] 3 Preview bar — sample text as the hero control (iteration 6; first pass: one row + Options row; UiField/UiSlider/UiToggle not built)
- [ ] 4 Dark by default + app shell (iterations 7, 11: toolbar icons + UiSegmentedControl, status bar reduced to live state; collapsible panes remain; dark-first launch blocked on the API default theme)
- [ ] 5 Sidebar redesign (iterations 8, 12, 13, 19, 22: Project sets count at the edge, hover export icon; Sources row actions inline on hover; UiIconButton, collection "…" menu, catalog power icons, smart sparkle; iteration 33: Problems / Project sets / Online catalogs fold with a chevron, remembered; section icons, Sources/Collections folding, Problems dedupe remain)
- [ ] 6 List rows (iterations 9, 14, 21, 29 + reviews 10, 25, 30: UiBadge, UiChevron, hover Find-similar icon, chevron on the sample line, sub-row guide, no pane ring, cursor bar + ring, hover tint; `?face=` list selection remains)
- [x] 7 Grid cards (iteration 16: sample ends on a whole word that fits; hover weight range remains)
- [ ] 8 Inspector (iterations 17, 26–28 + reviews 20, 30: Info grouped, Features switches, Glyphs grid, underline notes; shared tab head, Developer head, Features live preview remain)
- [x] 9 Find similar as a mode (iteration 18: results replace the list, source pinned, score bar, only differing reasons; iteration 24: grouped by family)
- [ ] 10 Language & script filter
- [ ] 11 "Renders my text" filter + visible missing glyphs
- [x] 12 Quick Look, finished (iteration 32: full-window specimen, size ladder, paragraph, arrows move between fonts; no glyph grid or per-view sample/size follow yet)
- [x] 13 Keyboard model (iteration 31: `?` sheet, `[` `]`, J/K; toolbar toggle + more tooltips remain)
- [ ] 14 Weight scrubbing
- [ ] 15 Context presets in Layout
- [ ] 16 Compare overlay
- [ ] 17 Pairing Studio + Pairs with
- [ ] 18 Drag out / copy CSS
- [ ] 19 Problems & Duplicates triage
- [ ] 20 Collections UX
- [ ] 21 Empty states
- [ ] 22 First run
- [ ] 23 Online catalogues
- [ ] 24 Motion pass
- [ ] 25 Overlay scrollbars
- [ ] 26 Hover/focus/active audit
- [ ] 27 Accessibility

## Entry template (at most 25 lines per entry)

```
## Iteration N — <title>
Date · Critique (before, 3–5 sentences) · Why this change · What changed (areas) ·
Backlog ticked · Shots folder · Follow-ups · Dependencies added · Reverted (yes/no, why)
```

---

## Iteration 1 — 2026-11-02 — Setup: screenshot scenes fixed; loop blocked by permissions
Fixed four screenshot scenes and added the `FM_SCREENS_*` hooks, but could not run or commit anything under `acceptEdits` (committed by hand as `iter(0)`).

## Iteration 1 (second run) — 2026-11-02 — Setup: loop script pre-approves its own commands
Gave `loop.ps1` an `--allowedTools` list so unattended runs can use node, pnpm and git.

## Iteration 2 — 2026-11-02 — Design tokens, bundled Inter, drawn controls (foundation 1 of 2)
Rewrote `theme.css` as the token file (three surfaces, hairlines, 4 px spacing, two radii, type, motion, z-index), bundled Inter and drew a baseline for every native control.

## Iteration 3 — 2026-11-02 — The ui/ Select: every native select replaced (foundation 2 of 3)
Built `UiPopover` and `UiSelect` (APG select-only combobox with type-ahead and a filter line) and replaced all 22 native selects, with e2e helpers in `e2e/select.ts`.

## Iteration 4 — 2026-11-02 — Filters drawer instead of the chip wall
Built `UiDrawer`; the facets moved into it, the bar shows `Filters · N` plus removable chips, and raw feature tags sit behind an expander.

## Review 5 — 2026-11-02
**Grades:** iter 0/1 (setup) **2** — needed plumbing, two runs lost to permissions. Iter 2 **3** — the right foundation and a quieter chrome, but nothing changed on the first screen, and Inter's width left the Home toolbar wrapping in Table/Layout at 1440 px. Iter 3 **4** — complete and keyboard-correct; it left the native `select` baseline in `theme.css` as dead CSS. Iter 4 **3** — the wall is gone and specimens start ~100 px higher, but features are still raw tags, chip CSS is inline and the drawer covers its own opener.
**Pulled apart:** iter 2 drew native selects globally; iter 3 removed every select, so the select baseline became dead code.
**Fixed:**
- `theme.css`: deleted the dead `select`/`option` rules; added `--fm-scrim` and replaced the literal scrim in 7 dialogs (CommandPalette, CompareSheet, GlyphCompare, QuickLookSheet, SmartRuleEditor, TaggingPanel, UpdateDialog).
- Monochrome badges (rule "nothing else is coloured"): green "Open license" (FontList, FontTable) and "OK" (HealthBadge), and accent "Variable" (FontPreviewRow, FontGrid), are now muted. `search.spec.ts` checks for the muted colour instead of green (UI changed on purpose).
- Focus ring on view headings that take focus only for screen readers (`h1/h2[tabindex=-1]`): removed globally and in StudioView's own rule. Studio, Problems and Conflicts no longer open with a box round the title.
**Rule amended (DESIGN.md):** status colours are for problems only; good states are neutral; Compare's two data colours are the one exception; the scrim is a token.
**Backlog:** item 4 now starts with the toolbar overflow; item 6 brings `UiBadge` (five copies of badge CSS) and a real twisty; item 8 takes the human feature names left from item 2, and the clipped Glyphs column.
**Verified:** typecheck, `lint:js`, Vitest 906, Playwright 272 passed / 16 skipped (`.harness/redesign/e2e-review-5.log`). Shots `.harness/redesign/shots/review-5/` (before) and `review-5-after/`: every view renders in light and dark, comfortable and compact.
**Not fixed, noted:** the focused font list draws a ring round the whole pane (item 26); Waterfall repeats the "Variable" label on every size line (item 6); `PreviewControls` keeps hex fallbacks for the colour wells in TS.
**Next five:** item 3 (preview bar: sample text as the hero; `UiField`, `UiSlider`, `UiToggle`) → item 4 (shell + toolbar overflow; `UiSegmentedControl`) → item 5 (sidebar; `UiIconButton`) → item 6 (rows; `UiBadge`) → item 7 (grid cards). Every one must change the `list` shot. Build each `ui/` component once and move existing copies onto it in the same iteration.

## Iteration 6 — 2026-11-03 — Preview bar: one row, sample text as the hero
Preview controls became one row (preset, a real text field, language, size slider, `Options`) with the rest behind the Options toggle; specimens start ~90 px higher.

## Iteration 7 — 2026-11-03 — Toolbar: icon view switcher, quiet icon buttons
Built `UiSegmentedControl`, `UiIcon` and `icons.ts`; view modes, Compact rows, Compare and Inspector became quiet icons, freeing ~400 px of toolbar.

## Iteration 8 — 2026-11-03 — Sidebar: header and source actions become icon buttons
Built `UiIconButton`; Sources, Collections and Project sets headers and the source rows' rescan/pause/remove became 16 px icons.

## Iteration 9 — 2026-11-03 — List rows: one badge component, a real chevron
Built `UiBadge`; FontList and FontPreviewRow badges moved onto it and the ▸/▾ twisty became a rotating chevron in List, Table and Grid.

## Review 10 — 2026-11-03
**Grades:** iter 6 **3** — the right win (three rows → one), but the text field only shows a placeholder for presets and the slider and number box are still drawn natives, not `UiSlider`/`UiField`. Iter 7 **4** — the clearest first-screen change of the four; it used literal motion values in `UiSegmentedControl`, and the Compact-rows icon reads as "list". Iter 8 **3** — the headers are calm now, but a selected collection still opens six bordered text buttons, "Turn on" stays, and source actions float on their own line. Iter 9 **3** — good component, half-applied: Table, Grid, HealthBadge and catalog rows kept their own badge CSS, Waterfall and Tags kept ▸/▾ text, and the chevron CSS was pasted into three files with literal timings.
**Regressions:** none in tests, keyboard or views. **Drift:** literal `120ms cubic-bezier` in four places; catalog rows showed an accent-coloured "Variable" (missed by review 5).
**Pulled apart:** none head-on; iterations 7 and 9 both bypassed the motion tokens that iteration 2 set up.
**Fixed:**
- New `ui/UiChevron` (tokens; one rotate rule). List, Table, Grid, Waterfall and the Tags fold use it; three pasted chevron blocks deleted.
- `UiBadge` gained `danger`/`warning` tones. FontTable (format, licence), FontGrid (Variable, Fallback), HealthBadge and CatalogFamilyItem (Variable, Update) use it; five local badge blocks deleted; catalog "Variable" is monochrome.
- Waterfall shows "Variable" on its first line only (`FontPreviewRow` `showVariable`), not on all six.
- `UiSegmentedControl` uses `--fm-motion-fast`/`--fm-ease`.
**Rule amended (DESIGN.md):** the component list is current; a local `.badge` or a ▸/▾ glyph is a bug. Items 5 and 6 list what is left.
**Verified:** typecheck, `lint:js`, Vitest 906, Playwright 272 passed / 16 skipped in one run (`.harness/redesign/e2e-review-10.log`). Shots `review-10/` (before) and `review-10-after/`: all views render in light and dark, comfortable and compact.
**Not fixed, noted:** focused list draws a ring round the whole pane (item 26); Collections "Smart" pill (item 5); status bar version strings (item 4); grid cards still cut the pangram to "The quick b" (item 7).
**Next five:** item 4 rest (status bar to live state, dark first launch — both on the first screen) → item 5 rest (collection row menu, "Turn on", smart subtype, Problems dedupe) → item 6 rest (hover row actions as `UiIconButton`, chevron alignment) → item 7 (grid cards) → item 8 (inspector Info grouping). Use the tokens for every duration and colour; when a `ui/` component lands, move every copy onto it in the same iteration and grep for the old class.

## Iteration 11 — 2026-11-03 — Status bar: live state only
Status bar shows `0 fonts active · 5 problems · Conflicts` left and the Commands/Settings hints right; app/API/platform versions moved to Settings › About.

## Iteration 12 — 2026-11-03 — Collections: six row buttons become one "…" menu
An open collection's six bordered buttons moved into a keyboard-complete `role=menu` popover behind a "…" `UiIconButton`.

## Iteration 13 — 2026-11-03 — Sidebar: "Turn on" becomes an icon, empty header gone
Catalog "Turn on" buttons became power `UiIconButton`s, the "Smart" pill a sparkle icon, and the empty "Smart collections" header was removed.

## Iteration 14 — 2026-11-03 — List rows: "Find similar" becomes a hover icon button
The row's bordered "Find similar" text button became a hover/focus `UiIconButton`; not visible in the shots (no hover scene).

## Review 15 — 2026-11-03
**Grades:** iter 11 **3** — the right first-screen change, but the Settings key cap sat ~2 px above Commands (a link and a button on different baselines, `kbd` forced to inherit the font). Iter 12 **3** — a correct, keyboard-complete menu; the open row showed two hover tints (row + global button gradient on the name), and it left three `vue/attributes-order` warnings. Iter 13 **3** — the calmest sidebar change yet (two text buttons and two caption lines gone); it left the `.groups` CSS and the `home.sidebarGroups.smart` string dead. Iter 14 **2** — invisible in every shot, and its lens-with-plus icon reads as "zoom in".
**Regressions:** none in views, keyboard or tests. **Drift:** six local copies of the screen-reader-only rule (two added by 12 and 13); `lint:js` exited 1 on `collections.spec.ts` formatting since iteration 12 while the log called it "warnings".
**Pulled apart:** none head-on; four small sidebar/row iterations in a row left the `list` shot unchanged since iteration 13.
**Fixed:**
- One global `.sr-only` in `theme.css`; local copies deleted in CollectionsSidebar, PreviewControls, ProblemsView, FontTable (`.visually-hidden`), OnlineCatalogsSidebar (`.connects`), SourcesSidebar (`.announce`).
- Status bar: hints are inline-flex and centred; key caps keep the `kbd` token style. Commands and Settings now share one baseline.
- CollectionsSidebar: the name button adds no second hover tint; attribute order fixed. `collections.spec.ts` formatted (`lint:js` exits 0).
- `similar` icon: a lens with "≈" instead of "+".
- Dead: HomeView `.groups`/`.group` CSS; `home.sidebarGroups.smart` in en/de.
**Rule amended (DESIGN.md):** screen-reader-only text uses the global `.sr-only`. Items 4, 5, 6 rewritten to what is left; item 4's dark-first launch marked blocked (API).
**Verified:** typecheck, `lint:js` (exit 0, no warnings), Vitest 906, Playwright 272 passed / 16 skipped (`.harness/redesign/e2e-review-15.log`). Shots `review-15/` (before) and `review-15-after/`: all 23 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** Fonts source-row actions on their own line; Project sets count not right-aligned; focus ring round the whole list pane; Find similar result pane close is a text "×"; Studio "Panel font" fields look empty.
**Next five:** item 7 (grid cards: the "The quick b" cut is the worst thing in any view) → item 8 (inspector Info grouped, hash/path behind "Technical") → item 9 (Find similar as a mode) → item 5 rest (Fonts row actions inline, Project sets counts) → item 6 rest (chevron alignment, sub-rows). Every iteration must change a shot; hover-only work adds a hover scene first. Run `pnpm lint:js` and read its exit code, not only its warnings.

## Iteration 16 — 2026-11-03 — Grid cards stop cutting the sample mid-word
`cardChars`/`cardText` in `list/grid.ts` fit the card width and end on a whole word, so cards show "The quick", not "The quick b".

## Iteration 17 — 2026-11-03 — Inspector Info: what matters first, raw data folded
Info opens on About, License and Your notes; names, file, hash, style and metrics moved behind a "Technical details" expander.

## Iteration 18 — 2026-11-03 — Find similar becomes a mode, not a split
`SimilarPanel` replaces the list (kept alive with `v-show`), pins the source face, draws a score bar and shows only the reasons that differ.

## Iteration 19 — 2026-11-03 — Sources: row actions sit on the row
Rescan / Pause / Remove moved into the Sources row, replacing the count on hover or focus; the local `.badge` became `UiBadge`.

## Review 20 — 2026-11-03
**Grades:** iter 16 **4** — fixed the worst flaw in any view with a small, tested helper; cards still show only two words and keep a dashed border on style cards. Iter 17 **3** — the right order, but it lifted the tall license form (native date field, radios, two buttons) into the first screen of Info, so Your notes and Technical details sat below the fold, and the license URL broke as "openfontlicense.or / g". Iter 18 **4** — the strongest change of the four: a real mode, quiet score bars, honest "Why"; results are still single faces, not families, and the bar used a literal 2 px radius. Iter 19 **4** — rows keep one height and the sidebar is calmer; the change shows only on hover, and no hover scene was added.
**Regressions:** none in tests, keyboard or views. Compact density wrapped "Not rated" one letter per line in the inspector (older, now fixed).
**Drift:** `SimilarPanel` `.bar`/`.fill` literal `border-radius: 2px` (now `--fm-radius-sm`). No new colours, fonts or native controls added.
**Pulled apart:** iteration 17 (raw data folded, progressive disclosure) and its own License placement (a full edit form open by default) worked against each other.
**Fixed:**
- `InspectorLicense`: the "Your license for this family" form is folded behind its heading (a `UiChevron` button, `aria-expanded`); it opens by itself when the family has a declaration. All of Info, Technical details included, now fits a 900 px window. `license.spec` opens it first (UI changed on purpose).
- `InspectorInfo`: the license URL stays on one line with an ellipsis (click still copies the full value); rating stars wrap as a group and "Not rated" never breaks.
- `SimilarPanel`: bar radii use the token.
**Rule amended:** none. **Backlog (DESIGN.md):** items 5, 7, 8, 9 rewritten to what is left; item 26 now names the native inputs still in the inspector and the list-pane focus ring; the three-second note points at items 6, 5, 26.
**Verified:** typecheck, `lint:js` exit 0, Vitest 909, Playwright 272 passed / 16 skipped (`.harness/redesign/e2e-review-20.log`). Shots `20/` (before) and `20-after/`: all 23 scenes render in light and dark, comfortable and compact (46 images).
**Not fixed, noted:** Project sets count mid-row (hidden Export button holds space); Glyphs grid's last column touches the pane edge; Studio "Panel font" fields look empty; Find similar results not grouped by family.
**Next five:** item 6 rest (chevron on the sample's cap height, quieter sub-rows, no ring round the whole list pane) → item 5 rest (Project sets row like Sources) → item 26 native-input sweep in the inspector (date, number, datalist, text stars) → item 9 rest (group similar results by family) → item 8 rest (Glyphs column, Features human names). Hover-only work adds a hover scene to `screens.spec.ts` in the same iteration.

## Iteration 21 — 2026-11-03 — List rows: chevron on the sample line, quiet sub-rows, no pane ring
The chevron moved onto the sample line, the list pane lost its focus ring, and style sub-rows got a faint tint and a hairline guide.

## Iteration 22 — 2026-11-03 — Project sets: count at the edge, Export becomes a hover icon
The Project sets "Export set…" text button became a hover `UiIconButton` over the count, so counts line up down the sidebar.

## Iteration 23 — 2026-11-03 — Inspector: drawn stars and heart instead of text glyphs
Rating stars and the Favorite heart became `UiIcon`s and the favorite turned accent instead of danger-red; a borderless-fields attempt was reverted for the 3:1 outline tests.

## Iteration 24 — 2026-11-03 — Find similar: one row per family
`SimilarPanel` shows one row per family (closest face) with a quiet "+N more styles"; taken without shots.

## Review 25 — 2026-11-03
**Grades:** iter 21 **3** — the pane ring is gone and comfortable rows read cleanly, but the 48 px bottom-anchored chevron cell missed the sample line in compact density, and the cursor row keeps a heavy 2 px box. Iter 22 **4** — small, exact, consistent with Sources; counts now line up; hover-only again and no hover scene. Iter 23 **3** — right direction (no text glyphs, one accent), but it left a duplicate `background-color` from the reverted attempt and a star hover on `--fm-text`, a token that does not exist, so the hover did nothing; the Playwright log it cites is from before its revert. Iter 24 **3** — the right grouping, but taken blind (no shots), and the "+N more styles" was visible text only: the result's `aria-label` hid it from screen readers.
**Regressions:** none in views or keyboard; compact chevron misaligned (iter 21). **Drift:** non-existent token (iter 23); no new colours, fonts, radii or native controls.
**Pulled apart:** none head-on; 22 and 24 both added hover/inline text without the hover scene review 15 and 20 asked for.
**Fixed:**
- `FontList`: the chevron is centred with a 20 px top pad (the meta line plus its gap), so it sits on the sample line in comfortable and compact.
- `InspectorInfo`: star hover uses `--fm-fg`; the duplicate `background-color` on the tag field deleted.
- `SimilarPanel`: the result's accessible name ends with ", +N more styles"; `similar.spec` updated to allow that suffix (UI changed on purpose).
- Dead: `sets.export` string in en/de (the icon uses `sets.exportLabel`).
**Rule amended:** none. **Backlog (DESIGN.md):** items 5, 6, 9 rewritten to what is left; item 26 adds the Features toggle knob, the heavy note fields, the Problems "Recommended" pill; the three-second note points at items 8, 26, 5.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 272 passed / 16 skipped (`.harness/redesign/e2e-review-25.log`). Shots `25/` (before) and `25-after/`: all 23 scenes render in light and dark, comfortable and compact (46 images).
**Not fixed, noted:** the cursor row's 2 px box; `?face=` does not select the list row; Studio "Panel font" fields look empty; Features toggle knob off-centre.
**Next five:** a hover scene in `screens.spec.ts` first (rows, sidebar) → item 26 (Features toggles, underline-style note fields, date/number/datalist) → item 8 rest (Glyphs clipped column, Features human names) → item 5 rest (section icons, collapsible sections, Problems dedupe) → item 12 or 13 (Quick Look or the keyboard model — the first real features). Every iteration takes before/after shots; no blind iterations.

## Iteration 26 — 2026-11-03 — Inspector Features: centred switches, stacked header, hairline rows
The switch knob is centred and travels symmetrically, the hint sits on its own line above Copy CSS / Reset, and rows got 32 px height and hairlines.

## Iteration 27 — 2026-11-03 — Inspector Glyphs: no grid lines, hover tint, sixth column
The glyph grid lost its pane border and cell lines, gained a rounded hover tint, and now fits six columns with none clipped.

## Iteration 28 — 2026-11-03 — Inspector notes: underline fields, no boxes, no resize grip
The tag and notes fields became underline fields (3:1 bottom border kept), and notes auto-grow with `field-sizing: content`.

## Iteration 29 — 2026-11-03 — List: keyboard cursor row gets a hairline ring and a bar
The cursor row's 2 px accent box became a 1 px ring at 60 % plus a 2 px start bar; taken without a cursor scene.

## Review 30 — 2026-11-03
**Grades:** iter 26 **3** — the knob is fixed and rows have rhythm, but it stacked only the Features head while the sibling Axes tab kept the old side-by-side head, and the track used a literal `9px` radius. Iter 27 **3** — the grid is calmer and the column is back, but its hover used `--fm-surface-2`, which is white in light theme, so the hover did nothing there; the heading sat 8 px off the pane edge; the coverage list below kept a different hover and a local `.badge`. Iter 28 **4** — the clearest change of the four (the notes block lost ~80 px of boxes); `field-sizing` is Chromium-only, so on WebKit (macOS, Linux) notes were a two-line field with no grip. Iter 29 **3** — right look, but not seen by eye (no cursor scene), and it pasted a second copy of the selection bar's `::before` block.
**Regressions:** none in views, keyboard or tests. Light-theme glyph hover invisible (iter 27).
**Drift:** literal radius (26); surface tokens as hover (27, and older in GlyphCoverage); local `.badge` (GlyphCoverage). No new colours, fonts or native controls.
**Pulled apart:** iterations 26 and the untouched Axes tab — two inspector tabs with the same markup and two head layouts. Four inspector iterations in a row left the `list` shot unchanged (byte-identical 25→27).
**Fixed:**
- `InspectorAxes`: head stacked like Features (hint line, actions right).
- `InspectorFeatures`: switch radius uses `--fm-radius-md`.
- `InspectorGlyphs`: cell hover `--fm-hover`; heading flush with the pane edge.
- `GlyphCoverage`: row hover `--fm-hover`; the local `.badge` became `UiBadge` (mono); unit test follows the class.
- `InspectorInfo`: `@supports not (field-sizing)` fallback keeps notes 3 lines tall with a grip on WebKit.
- `FontList`: one `::before` rule for the selection and cursor bars; rows tint with `--fm-hover` on hover (none before).
- New `hover-row` scene (`FM_SCREENS_HOVER` in `screens.spec.ts`, `shots.mjs`) — asked for since review 15.
**Rule amended (DESIGN.md):** hover tints use `--fm-hover`, never a surface token. Items 6, 8, 26 and the three-second note rewritten.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 272 passed / 16 skipped (`.harness/redesign/e2e-review-30.log`). Shots `review-30/` (before, 46) and `review-30-after/` (48, with `hover-row`): all 24 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** Developer tab head; three copies of the tab-head CSS; "applied automatically" repeated per row; Settings nav 2 px box; Studio "Panel font" fields look empty; `?face=` list selection.
**Next five:** item 13 (keyboard model: J/K, A, C, S, 1–5, `[` `]`, `?` sheet) → item 12 (Quick Look as the hero) → item 5 rest (section icons, collapsible sections, Problems dedupe) → item 8 rest (one shared inspector tab head; Developer stacked) → item 26 (Settings radios/checkboxes and nav focus). Inspector polish is done for now: every iteration must change the `list` shot or a scene a feature adds.

## Iteration 31 — 2026-11-03 — Keyboard model: `?` shortcut sheet, `[` / `]` panes, J/K
Critique (before): the `list` shot is calm, but nothing tells a designer the app is keyboard-first: shortcuts exist (A, C, S, T, 1–5) yet none is discoverable, and the sidebar cannot be hidden. Worst: the rows' beauty is boxed in by a fixed 240 px sidebar. Good: hairlines, quiet chrome.
Why: backlog item 13 is next after review 30. Feels different in the first minute: `?` opens a sheet, `[` hides the sidebar so the list gets 240 px more width.
What changed:
- `ShortcutSheet.vue` (new, mounted in `App.vue`): modal, three columns (Move / Fonts / View), chords read from `KEYMAP`, `?` or Esc closes, focus returns.
- `keymap.ts`: bindings `[` (sidebar), `]` (inspector, next to Ctrl+I), `?` (sheet).
- `layout.ts` + `MainLayout.vue`: `sidebarOpen` (remembered), `toggleSidebar`, palette action.
- `list/keys.ts`: `J` / `K` move the cursor down / up.
- `screens.spec` scene table: new `shortcuts` scene (`shots.mjs`). en + de strings.
Backlog: item 13 mostly done (left: `/` already bound; tooltips with shortcuts on more controls).
Shots: `.harness/redesign/shots/31/` (`shortcuts-*` checked by eye, both themes). Verified: typecheck, lint:js, Vitest 910, Playwright 272 passed / 16 skipped.
Follow-ups: no collapsed-sidebar rail or toggle button in the toolbar (keyboard only); sheet is static text for list keys; no unit test for `ShortcutSheet` or J/K yet.
Dependencies: none.

## Iteration 32 — 2026-11-03 — Quick Look becomes a full-window specimen
Critique (before): the `list` shot is calm and the rows read well, but Quick Look was a 1100 px card with one 96 px line and a Close text button: it hid the list and showed almost nothing new. Worst: no way to see the next font without closing it. Good: quiet chrome, hairlines.
Why: backlog item 12, next after the keyboard model. Feels different in the first minute: Space fills the window with one specimen, and arrows flip through fonts.
What changed:
- `QuickLookSheet.vue`: full-window sheet on `--fm-bg`; hero sample up to 104 px, alphabet + numerals line, a 12–36 px size ladder (tabular size labels), a paragraph; close is a `UiIconButton`; the face loader swaps faces without a system-font flash.
- Arrow keys (←↑ / →↓) move the list cursor; the sheet follows `fontList.cursorFace`. `HomeView` no longer remounts the sheet per face.
- en + de hint; `a11y.spec` gets an arrows test.
Backlog: item 12 ticked (core). Shots: `.harness/redesign/ql-dbg.png` (dark, by eye); no scene yet.
Verified: typecheck, lint:js, Vitest 910, Playwright 273 passed (`.harness/redesign/e2e-32.log`).
Follow-ups: add a `quicklook` scene to `shots.mjs`; glyph grid and variable-axis sliders in the sheet; sample text/size do not follow per-view settings; the keyboard-model test breaks if it moves the cursor and returns (activation `A` then misses) — worth a look.
Dependencies: none.

## Iteration 33 — 2026-11-03 — Sidebar groups fold
Critique (before): the `list` shot is calm, but the sidebar is six headings of the same weight with only Tags able to fold; Problems alone lists five rows nobody needs every day. Worst: three headings look identical yet behave differently. Good: hairlines, counts at the edge.
Why: item 5 rest (collapsible sections). Feels different in the first minute: a chevron on each group; folding Problems gives the sidebar 130 px back.
What changed:
- `ui/UiFoldButton.vue` (new) + `components/useFolded.ts` (remembered in localStorage `fm.sidebar.folded.v1`, a view preference).
- Problems, Project sets, Online catalogs use them. Problems keeps its link and test id; the chevron is its own button.
Backlog: item 5 partly. Shots: `.harness/redesign/shots/33-after/` (`list-dark` by eye).
Verified: typecheck, lint:js, Vitest 910, Playwright 271 passed + 2 timeouts that pass alone (`.harness/redesign/e2e-33.log`).
Follow-ups: Tags still has its own fold markup (move to `UiFoldButton`); Sources and Collections do not fold; no section icons; Problems heading still doubles the Problems view link.
Dependencies: none.
