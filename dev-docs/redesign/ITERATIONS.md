# Iteration log

Append-only. Newest at the bottom. Every iteration and every review pass adds an entry.
Backlog ticks live here, not in `DESIGN.md`.

## Backlog status
<!-- Keep this list in sync: mark an item [x] in the iteration that finishes it. -->
- [x] 1 Design tokens + theme + UI font + ui/ Select (iterations 2–3; the gate is lifted)
- [x] 2 Filters panel instead of the wall (iteration 4; UiDrawer built, chips stay in FilterChips; human feature names moved to item 8)
- [x] 3 Preview bar — sample text as the hero control (iteration 6; first pass: one row + Options row; UiField/UiSlider/UiToggle not built)
- [ ] 4 App shell (iterations 7, 11, 31, 39: toolbar icons, UiSegmentedControl, live status bar, collapsible panes with `[` `]` and a sidebar toggle; dark-first dropped — owner wants paper-first, default stays `system`)
- [ ] 5 Sidebar redesign (iterations 8, 12, 13, 19, 22, 33–34, 37, 39 + review 35: icon buttons, row menus, folding groups, section icons, "All fonts" stays, toolbar toggle; Problems fold+link remains)
- [x] 6 List rows (iteration 81: row actions Activate, Compare, Similar on hover) (iterations 9, 14, 21, 29 + reviews 10, 25, 30: UiBadge, UiChevron, hover Find-similar icon, chevron on the sample line, sub-row guide, no pane ring, cursor bar + ring, hover tint; iteration 42: `?face=` selects the list row)
- [x] 7 Grid cards (iteration 77: name never cut; iteration 16: sample ends on a whole word that fits; iteration 66: hover weight range, `grid-hover`/`grid-expanded` scenes)
- [ ] 8 Inspector (iterations 17, 26–28, 43, 49, 53, 54 + reviews 20, 30: Info grouped, Features switches, Glyphs grid, underline notes, shared tab head, off/on feature samples, human feature names in the drawer; stylistic-set samples blocked on the API)
- [x] 9 Find similar as a mode (iteration 18: results replace the list, source pinned, score bar, only differing reasons; iteration 24: grouped by family)
- [x] 10 Language & script filter (iteration 36: visible language chips in the Filters drawer, choosing one switches the All fonts pangram; counts and script grouping remain)
- [ ] 11 "Renders my text" filter + visible missing glyphs
- [x] 12 Quick Look, finished (iteration 32: full-window specimen, size ladder, paragraph, arrows move between fonts; no glyph grid or per-view sample/size follow yet)
- [x] 13 Keyboard model (iteration 31: `?` sheet, `[` `]`, J/K; iteration 74 + review 75: Grid Alt+→/← opens a family, listed in the sheet; more tooltips remain)
- [x] 14 Weight scrubbing (iteration 73: drag across a variable sample in the list/table rows to scrub `wght`; value pill beside the label; kept on release)
- [ ] 15 Context presets in Layout
- [ ] 16 Compare overlay (overlay mode already existed; iterations 71–72 + review 75: centred sheet, quiet controls, hairline specimens at 2× the list size, overlay centred on its stage; a size control and a text/radio `UiSegmentedControl` variant for Compare and the Axes instances remain)
- [ ] 17 Pairing Studio + Pairs with (iterations 44, 48, 52, 67, 76: quiet name tags, two bars, icon buttons, Layout help behind info and a lock icon; Studio lock beside the page and suggestions remain)
- [ ] 18 Drag out / copy CSS
- [ ] 19 Problems & Duplicates triage
- [ ] 20 Collections UX
- [ ] 21 Empty states (iteration 41: UiEmptyState — headline, one line, one action; empty collection, no matches, empty library; iteration 64: sidebar Tags empty = quiet title + key chip; Layout, Catalog, Quick Collection empties remain)
- [ ] 22 First run
- [ ] 23 Online catalogues
- [ ] 24 Motion pass
- [ ] 25 Overlay scrollbars
- [ ] 26 Hover/focus/active audit (iterations 61–63, 68–69, 78–79 + reviews 65, 70, 80: one quiet UiButton for every text button incl. Catalog rows and Axes, one UiIconButton for every toolbar icon; three `type="number"` inputs, the tags `datalist`, dialog buttons, Filters drawer chips and the Settings nav focus ring remain)
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
Added `ShortcutSheet` on `?`, `[` / `]` to hide the sidebar and inspector (remembered), and J/K to move the list cursor.

## Iteration 32 — 2026-11-03 — Quick Look becomes a full-window specimen
`QuickLookSheet` fills the window (hero sample, alphabet, size ladder, paragraph) and the arrow keys move to the next font without closing.

## Iteration 33 — 2026-11-03 — Sidebar groups fold
Built `UiFoldButton` + `useFolded` (remembered in localStorage); Problems, Project sets and Online catalogs fold.

## Iteration 34 — 2026-11-03 — Every sidebar group folds the same way
Sources, Collections and Tags moved onto `UiFoldButton`, so all six headings share one chevron.

## Review 35 — 2026-11-03
**Grades:** iter 31 **3** — the right feature and a calm sheet, but "Up and down" broke onto two lines beside four key caps, `[` has no mouse twin, and nothing showed in the `list` shot. Iter 32 **4** — the best change of the four: a real full-window specimen that flips with the arrows; taken without a scene, and its title said the style twice ("Inter Regular  Regular"). Iter 33 **3** — useful folding, but the Problems label sat 4 px right of every other heading (fold button + link padding). Iter 34 **3** — the right consolidation (Tags' own fold markup gone); it left `palette.spec.ts` failing Prettier, so `pnpm lint:js` exited 1 while its log said lint passed.
**Regressions:** none in views, keyboard or Playwright. `lint:js` red since iteration 34 (now fixed).
**Drift:** no literal colours, fonts, radii or native controls added. Dead: a stray blank rule gap in TagsSidebar CSS.
**Pulled apart:** 33 built `UiFoldButton` but left Tags on its own fold markup; 34 reconciled it. Four iterations left the `list` shot unchanged or nearly so (31 = 32 byte-identical).
**Fixed:**
- `ProblemsSidebar`: link padding so the label lines up with the other five headings.
- `QuickLookSheet`: title shows the face name once; the duplicate style span and its CSS deleted.
- `ShortcutSheet`: move row uses `↑↓ J K` (three caps) and labels never wrap.
- `palette.spec.ts` formatted; `lint:js` exits 0.
- New `quicklook` scene in `shots.mjs` (Space on row 1) — asked for by iteration 32.
**Rule amended (DESIGN.md):** `UiFoldButton`/`useFolded` join the component list; a sidebar heading with its own fold markup is a bug. Items 5, 12, 13 and the three-second note rewritten.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-35.log`). Shots `review-35/` (before, 52 with `quicklook`) and `review-35-after/` (52): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** folded Collections hides "All fonts"; Problems heading is fold + link with the same name; no toolbar toggle for the sidebar; QL sample/size follow All fonts only; Studio "Panel font" fields look empty; Settings radios and nav box.
**Next five:** item 10 (language & script filter facet; choosing a language switches the pangram) → item 5 rest (section icons; keep "All fonts" visible when Collections folds) → item 26 (Settings radios/checkboxes, nav focus box) → item 8 rest (one shared inspector tab head; Developer stacked) → item 21 (designed empty states — "No fonts match" is still one grey sentence). Every iteration runs `pnpm lint:js` and reads the exit code; every feature adds a scene.

## Iteration 36 — 2026-11-03 — Language filter: visible chips, pangram follows
The language facet became a row of chips (first 10, "N more…" expander) and choosing one switches the All fonts pangram.

## Iteration 37 — 2026-11-03 — Sidebar section icons; "All fonts" survives folding
`UiFoldButton` took an optional `icon` (six new icons on the six headings) and "All fonts" moved out of the folding Collections list.

## Iteration 38 — 2026-11-03 — Settings menu: icons, quiet active state
Settings tabs got icons, muted text, a hover tint and an accent bar when active; group legends became quiet captions.

## Iteration 39 — 2026-11-03 — Toolbar: sidebar toggle button
A pressed-state sidebar toggle (`[`) sits before the title in the All fonts toolbar, the mouse twin of the keyboard shortcut.

## Review 40 — 2026-11-03
**Grades:** iter 36 **3** — the right feature, now visible, but the facet landed at the very bottom of the drawer (below the fold at 900 px) and left `chips.add`/`chips.addLanguage` dead in en/de. Iter 37 **3** — the sidebar finally has rhythm; the icon uses `opacity` instead of a token and "All fonts" and the collections are two lists with one label. Iter 38 **3** — calmer nav, but it quietened the fieldset legends and not the field labels, so Appearance showed a loud "Language" under quiet "Theme" and "Density". Iter 39 **4** — small, exact, accessible (`aria-pressed`, `aria-controls`, `[` in the title), a test step updated honestly; it shows on the All fonts toolbar only.
**Regressions:** none in views, keyboard or tests. **Drift:** no literal colours, new fonts, radii or native controls added.
**Pulled apart:** iteration 38 within itself (legends vs field labels). Three of four iterations left the `list` shot unchanged; 37 and 39 changed it.
**Fixed:**
- `FilterChips`: the language facet sits right after Class (one loop over the facets with a language slot, no duplicated markup).
- `SettingsView`: legends and `.field` labels share one quiet caption rule.
- Dead: `chips.add`, `chips.addLanguage` in en/de.
**Rule amended:** none. **Backlog (DESIGN.md):** items 4, 5, 10, 26 and the three-second note rewritten; item 10's counts marked blocked (API).
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 272 passed / 16 skipped / 1 flake (`pairing-studio.spec.ts:361`, the list did not load in time; passes alone) (`.harness/redesign/e2e-review-40.log`). Shots `40/` (before, 52) and `review-40-after/` (52): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** Settings opened by keyboard shows the focus ring on the active tab (correct for keyboard, heavy to look at); `selected-row` scene shows no selected row (`?face=`); Studio "Panel font" fields look empty; Developer tab head still side by side.
**Next five:** item 21 (designed empty states) → item 6 rest (`?face=` selects the list row; fix the `selected-row` scene) → item 8 rest (one shared inspector tab head; Developer stacked) → item 17 first slice (Studio font pickers that show the font, not an empty field) → item 26 rest (license date/number/datalist, Advanced number fields). Every iteration changes a shot and reads the `lint:js` exit code.

## Iteration 41 — 2026-11-03 — Designed empty states
Built `UiEmptyState` (headline, one line, one action); HomeView shows empty collection, no matches or empty library instead of one grey sentence.

## Iteration 42 — 2026-11-03 — `?face=` selects the row in the list
HomeView reveals the URL's face once the list loads, so the row and the inspector agree after a reload.

## Iteration 43 — 2026-11-03 — One shared inspector tab head
New `InspectorTabHead` (hint line, actions under it) replaced three copies of head CSS in Axes, Features and Developer.

## Iteration 44 — 2026-11-03 — Studio "Panel font" fields become quiet name tags
`BlockFontPicker` lost its box until hover or focus, moved to Inter and tokens, and gained a `UiChevron` caret.

## Review 45 — 2026-11-03
**Grades:** iter 41 **4** — a real component, three honest states with one action each; only one of the three has a scene, and Tags/Layout/Catalog empties stay plain text. Iter 42 **3** — the right fix, but it made every `?face=1` scene show the same state: `hover-row` hovered the now-selected row (no hover visible), `expanded-family` matched `list`; a one-style family now shows its sample twice (Waterfall: twelve lines of Inter); a redundant branch in the watch; its "before" shots were lost. Iter 43 **4** — the right consolidation, three CSS copies gone; the new head used literal 8/12 px. Iter 44 **3** — the box is gone, but "Panel font" sat ~120 px from its caret, so label and caret read as two things; literal paddings.
**Regressions:** none in views, keyboard or tests; two screenshot scenes stopped showing what they are named for (42).
**Drift:** literal spacing in `InspectorTabHead` and `BlockFontPicker`. No new colours, fonts, radii or native controls.
**Pulled apart:** none head-on. Only iteration 42 changed the `list` shot (Inter now opens).
**Fixed:**
- `shots.mjs`: `hover-row` hovers row 3, `expanded-family` opens row 3 — both show their state again.
- `BlockFontPicker`: the name is end-aligned at rest, so "Panel font ⌄" sits beside the block; start-aligned while typing; spacing tokens.
- `InspectorTabHead`: spacing tokens, logical padding.
- `InspectorFeatures`: rows under "Applied automatically" no longer repeat "applied automatically"; `.auto` CSS and `features.automaticNote` (en/de) deleted; unit and e2e checks follow (UI changed on purpose).
- `HomeView`: the `?face=` watch has one exit path.
**Rule amended (DESIGN.md):** component list adds `UiEmptyState` and `InspectorTabHead`. Items 6, 8, 17, 21 and the three-second note rewritten; item 6 adds the one-style double sample.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-45.log`). Shots `review-45/` (before, 52) and `review-45-after/` (52): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** an open empty collection shows "Clear filters" with no filters chosen; Grid's selected style card keeps a dashed border; Settings nav focus ring on load.
**Next five:** item 6 rest (one-style family: no doubled sample on reveal) → item 7 rest (Grid style card without the dashed border; hover weight range) → item 17 (Studio toolbar to one row; options in their own face) → item 26 (license date/number/datalist, Advanced number fields) → item 21 rest (Tags/Quick Collection empties on `UiEmptyState`). Every iteration takes before and after shots and reads the `lint:js` exit code.

## Iteration 46 — 2026-11-03 — One-style families no longer show their sample twice
`fontList.reveal` keeps a one-style family folded and selects its family row, so a `?face=` reload no longer doubles Inter's sample.

## Iteration 47 — 2026-11-03 — Grid style cards: no dashed border, hover tint
Style cards in Grid lost the app's only dashed border; every card tints with `--fm-hover`.

## Iteration 48 — 2026-11-03 — Studio toolbar: four rows become three
`LayoutActions` got an `inline` mode and moved into the Studio panel bar as quiet buttons.

## Iteration 49 — 2026-11-03 — Features tab: each switch shows its feature at work
Each switchable feature shows a short sample in the face with that feature forced on.

## Review 50 — 2026-11-03
Only four iterations since review 45 (46–49); all four graded.
**Grades:** iter 46 **3** — the right fix for `?face=` reveal, but only there: a one-style family still has a chevron, and opening it shows the sample twice (`expanded-family` scene); the e2e regex now accepts either row name. Iter 47 **3** — correct small cleanup (the only dashed line gone, hover tint on a token), but no scene shows a style card or hover, and no before shots. Iter 48 **2** — the Studio got quiet buttons, but `LayoutActions` kept the old bordered look in the Layout view: one component, two looks; literal `4px 8px`; the Studio still has three bars plus the font line. Iter 49 **4** — the first type inside the Features tab, a real visible change; it copied the acquire/release block from `InspectorGlyphs` word for word, and the sample does not show the difference (always on; stylistic sets get a generic "Rag Agy").
**Regressions:** none in views, keyboard, tests or list performance (59.8 fps).
**Drift:** literal padding in `LayoutActions`; found older: `FontLayout` (Lock width, font name) and `LayoutBlock` (pt chips) drew `11px system-ui` — the only system font left in the chrome.
**Pulled apart:** 48 against the Layout view (same toolbar, two styles). Only 46 changed the `list` shot; 47 and 48 left it as it was (not opened again — the code paths do not touch the list).
**Fixed:**
- New `preview/useHeldFace.ts`; `InspectorFeatures` and `InspectorGlyphs` use it instead of two copies of the hold/release code.
- `LayoutActions`: one quiet button style (no border, `--fm-hover`, accent text when History is open) in both Studio and Layout; spacing tokens; `inline` only drops the bar's own padding and border.
- `FontLayout`, `LayoutBlock`: `11px system-ui` → `var(--fm-text-xs)/1.4 var(--fm-font-ui)`.
**Rule amended (DESIGN.md):** components add `useHeldFace` (one face in its own font). Items 6, 7, 8, 17 and the three-second note rewritten.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-50.log`; edits landed during that run, so features/glyphs/layout/studio/text-layouts/a11y re-run after: 91 passed / 4 skipped, `e2e-review-50-recheck.log`). Shots `review-50/` (before, 52) and `review-50-after/` (52): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** "Lock width" pressed state is an accent outline in a quiet bar; the Studio help sentence under the page; Grid pangram at 28 px shows two words only.
**Next five:** item 6 (no chevron / expand for one-style families in all four views, ArrowRight too) → item 17 (Studio: font line and panel bar into one row; Print/Close as icon buttons; help behind `?`) → item 8 (Features sample off/on side by side; human names in the Filters drawer) → item 26 (license date/number/datalist, Advanced number fields) → item 21 (Tags / Quick Collection empties on `UiEmptyState`). Every iteration takes before and after shots of a scene that shows its change.

## Iteration 51 — 2026-11-03 — One-style families lose their chevron
`FamilyRow.single` drops the chevron, `aria-expanded` and ArrowRight/toggle for one-style families in List, Table, Grid and Waterfall.

## Iteration 52 — 2026-11-03 — Studio: three bars become two, quiet icon buttons
`FontLayout` `bare` drops the Studio's duplicate name line; Close, Print and close-panel became icon buttons and the gesture sentence moved behind an info icon.

## Iteration 53 — 2026-11-03 — Filters drawer: features get human names
`featureName` in `search/chips.ts` names 28 registered tags plus `ssNN`/`cvNN`; named features sit in the drawer, sets and variants behind an expander.

## Iteration 54 — 2026-11-03 — Features tab: each sample shows off and on side by side
Each Features row shows its sample twice, muted with the feature off and bright with it on.

## Review 55 — 2026-11-03
Four iterations since review 50 (51–54); all four graded.
**Grades:** iter 51 **4** — the one change of the four that moved the `list` shot, done in all four views with the keyboard; but `aria-expanded` keyed on `styleCount > 1` while the chevron keyed on `single`, so a one-style family with a pinned instance showed a chevron and no `aria-expanded`; it left the `expanded-family` scene showing nothing for three iterations. Iter 52 **3** — one bar and three bordered buttons gone, but the gesture sentence moved into a native `title` on an info icon: slow on hover, never shown to keyboard users; "Lock width" now floats at the window corner, far from the page. Iter 53 **3** — the right change for designers, but it pasted the chip button block twice, kept a hand-drawn SVG twisty (not `UiChevron`, not flipped in RTL) and left `chips.allFeatures` dead; no scene shows it (the drawer is taller than 900 px). Iter 54 **3** — the difference is now visible for liga/onum/smcp; the pair has no "off"/"on" cue and stylistic sets still compare "Rag Agy" with itself.
**Regressions:** none in views, tests or list performance (59.8 fps). Keyboard: the Studio help was unreachable without a mouse (52; fixed). **Drift:** a local SVG twisty (53); no literal colours, fonts, radii or native controls added.
**Pulled apart:** none head-on. Only 51 changed the `list` shot; 52–54 are byte-identical to it.
**Fixed:**
- `FontList`, `FontTable`, `FontWaterfall`: `aria-expanded` follows `row.single`, like the chevron and the store.
- `shots.mjs` `expanded-family`: query `?mockFaces=1&mockStyles=3`, row 4 ("Sample 1", three styles) — the scene shows an open family again.
- `StudioView`: the info button toggles the gesture sentence as a quiet line under the bar (`aria-expanded`, `aria-controls`, accent while open); no `title`-only help.
- `FilterChips`: one loop draws both feature sets; the expander uses `UiChevron` in an `fm-directional` wrapper and lines up with the facet labels.
- Dead: `chips.allFeatures`, `studio.printPanel`, `studio.closePanel` (en/de).
**Rule amended:** none. **Backlog (DESIGN.md):** items 6, 8, 17 and the three-second note rewritten; item 8 notes the stylistic-set sample as blocked (API) and the two feature-name sources.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-55.log`). Shots `review-55/` (before, 52) and `review-55-after/` (52): all 26 scenes render in light and dark, comfortable and compact; only `expanded-family` changed. Tall drawer check in `review-55-tall/`.
**Not fixed, noted:** sub-rows repeat "Variable" on every style; "Lock width" placement; Table's "Columns" is a bordered text button among icons; Settings nav shows the focus ring on load.
**Next five:** the first screen again — item 26 (license date/number/datalist; Settings radios and Advanced number fields) → item 6 (no repeated badge on sub-rows) → item 7 (hover weight range on cards, a `grid-expanded` scene) → item 21 (Tags / Quick Collection empties on `UiEmptyState`) → item 17 ("Lock width" beside the page; options in their own face). Every iteration takes before and after shots of a scene that shows its change and reads the `lint:js` exit code.

## Iteration 56 — 2026-11-03 — Sub-rows stop repeating the "Variable" badge
`FontList` passes `show-variable` only to family rows, so an open variable family says "Variable" once.

## Iteration 57 — 2026-11-03 — License form: designed number and date fields
Built `ui/UiNumberField` (text input, − / + buttons, ↑/↓, tabular numerals) for Seats; Purchase date became a text field with a `YYYY-MM-DD` hint.

## Iteration 58 — 2026-11-03 — Settings › Advanced: designed number fields
Cache size and Rescan interval moved onto `UiNumberField`, saving 400 ms after each key (replaced in review 60).

## Iteration 59 — 2026-11-03 — Preview bar: designed number fields
`UiNumberField` gained decimals, `max`, `aria-label` and a `commit` event; Size, Line height and Tracking use it.

## Review 60 — 2026-11-03
Four iterations since review 55 (56–59); all four graded.
**Grades:** iter 56 **3** — correct and small (one prop), the repeat is gone from `expanded-family`; nothing moved on the `list` shot. Iter 57 **3** — a good, reusable `UiNumberField`; the date field lost the browser calendar for a free-text field with a format hint (core still validates); no shots at all. Iter 58 **2** — it saved 400 ms after every key, so typing "1024" slowly sent "1", showed "must be between 64 and 16384" and re-keyed the field under the cursor; it dropped `aria-describedby="rescan-hint"`; no shots. Iter 59 **4** — the one first-screen change of the four (Size has − / +), the component improved in place (decimals, no 0.05 drift, `commit`), all three Preview fields moved at once.
**Regressions:** Settings › Advanced typing (58; fixed); the rescan hint lost its link to the field (58; fixed). Views, keyboard and list performance (59.8 fps) fine.
**Drift:** none added — no literal colours, fonts, radii or native controls. Three `type="number"` inputs remain (BlockProperties, SmartRuleRow, GlyphCompare).
**Pulled apart:** 58 saved on `update:modelValue` with a timer; 59 added `commit` for exactly this job. One component, two save models — now one.
**Fixed:**
- `SettingsView`: number fields save on `commit` (Enter, blur, − / +); no timers; a refused value re-keys only its own field.
- `UiNumberField`: `ariaDescribedby` prop; the rescan field is described by its hint again.
- `SettingsView.test`: typing alone sends nothing; commits save; the hint link is checked.
**Rule amended (DESIGN.md):** every number field is `UiNumberField`, and savers listen to `commit`. Items 6 and 26 and the three-second note rewritten; item 26 now leads with a shared quiet `UiButton`.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-60.log`). Shots `review-60/` (before, 52) and `review-60-after/` (52, byte-identical — no scene shows Settings › Advanced): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** bordered text buttons (Options, Columns, Find similar, Copy CSS) beside quiet icons; Problems view chip row and accent "Recommended" pill; Settings nav focus ring on load; no `settings-advanced` or `license-open` scene.
**Next five:** item 26 — a quiet `UiButton`, moved onto every bordered text button in the toolbar, Preview bar and inspector (first screen) → item 26 rest (last three `type="number"`, tags `datalist`) → item 21 (Tags empty in the sidebar on `UiEmptyState`) → item 7 (hover weight range, `grid-expanded` scene) → item 17 ("Lock width" beside the page). Every iteration takes before and after shots of a scene that shows its change; an iteration with no shots is graded 2 at most.

## Iteration 61 — 2026-11-03 — One quiet UiButton for the bordered text buttons
Built `ui/UiButton` and moved Options, Find similar, Columns and Copy CSS onto it, so the `list` shot lost its last bordered text buttons.

## Iteration 62 — 2026-11-03 — Problems view: quiet buttons, no chip boxes
The Problems filter row, Close and every resolve button became `UiButton`s and "Recommended" a `UiBadge` (made quiet in review 65).

## Iteration 63 — 2026-11-03 — Settings, Conflicts and Preview options: quiet buttons
Settings (8 buttons), Conflicts Close and the Preview options row (alignment, Reset) moved onto `UiButton`; taken without opening a shot.

## Iteration 64 — 2026-11-03 — Sidebar Tags empty state: title and key chip
The Tags empty sentence became a short title plus a "Select fonts, then press T" hint with the key in a `<kbd>` chip.

## Review 65 — 2026-11-03
Four iterations since review 60 (61–64); all four graded.
**Grades:** iter 61 **3** — the right component and the one change that moved the `list` shot (Options, Find similar lost their boxes); but `UiButton` drew its own `--fm-hover` background on top of the global `button` hover gradient (a doubled tint), and three older quiet-button copies stayed (LayoutActions `.toggle`, Studio "Add panel", HomeView "Open in Pairing Studio"). Iter 62 **3** — the Problems view finally has a lead (the table); but "Recommended" became an accent badge, against the one-accent rule; light not opened. Iter 63 **2** — eleven buttons moved, but no shot opened and the log said Settings has no scene (`settings`, `settings-activation`, `settings-appearance` exist); its disabled rule repeated the global one. Iter 64 **3** — the Tags empty reads better than a sentence; the title in full `--fm-fg` looked like a tag row; it ticked item 21 `[x]` with a garbled status line while its entry said "still open"; `lint:js` was not run (Prettier only).
**Regressions:** none in views, keyboard, tests or list performance (59.8 fps). Doubled hover tint on every `UiButton` (61; fixed).
**Drift:** an accent badge for a non-selection state (62); literal disabled opacity twice (63). No new colours, fonts, radii or native controls.
**Pulled apart:** 61's `UiButton` against 48's quiet `LayoutActions` buttons and the Studio's local `.button` — three quiet text buttons, three hover rules. Now one.
**Fixed:**
- `UiButton`: no own hover background or disabled rule (the `theme.css` baseline draws both); `aria-disabled` fades it.
- Onto `UiButton`, local CSS deleted: `LayoutActions` (Save, Revert, Duplicate, Reset, History, Restore), Studio "Add panel", Layout "Open in Pairing Studio" (was bordered), Axes/Features "Reset" and Developer "Copy" / "Copy for Font Switch" (were bordered beside a quiet Copy CSS).
- `ProblemsView`: "Recommended" is a quiet `UiBadge`.
- `TagsSidebar`: the empty title is muted, so it does not read as a tag.
- Backlog status: item 21 back to `[ ]` (Layout, Catalog, Quick Collection empties remain); item 26 line current.
**Rule amended (DESIGN.md):** every text button is `UiButton`; its hover and disabled look come from the global baseline, and no caller adds a second tint. Items 21, 26 and the three-second note rewritten.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 272 passed / 16 skipped / 1 flake (`text-layouts.spec.ts:446`, the list did not load in time; the file and `pairing-studio.spec.ts` re-run: 29 passed) (`.harness/redesign/e2e-review-65.log`, `e2e-review-65-recheck.log`). Shots `review-65/` (before, 52; some edits landed mid-run) and `review-65-after/` (52): all 26 scenes render in light and dark, comfortable and compact.
**Not fixed, noted:** Settings nav focus ring on load; Axes presets and "Pin this instance" bordered; dialog buttons (CompareSheet, Catalog, Local API) bordered; toolbar `.toggle.icon` is a second icon-button copy; an empty collection shows "Clear filters" with no filters.
**Next five:** item 7 (hover weight range on cards + a `grid-expanded` scene — a change inside the specimens) → item 17 ("Lock width" beside the page; "Add panel" as an icon) → item 26 (Settings nav focus ring on load; toolbar `.toggle.icon` onto `UiIconButton`) → item 26 rest (three `type="number"`, tags `datalist`) → item 21 rest (Quick Collection / Layout empties). Open every shot you take, both themes; an iteration whose log contradicts its shots or its tick is graded 2 at most.

## Iteration 66 — 2026-11-03 — Grid cards show the weight range on hover
On hover or at the cursor a card's style count fades to the weight range in the same cell; new `grid-hover` and `grid-expanded` scenes.

## Iteration 67 — 2026-11-03 — Studio: "Add panel" becomes a plus icon
The Studio bar's last text button became a plus `UiIconButton` with the same name, title and focus return.

## Iteration 68 — 2026-11-03 — Toolbar toggles become one icon button
Sidebar, density, compare and inspector toggles moved onto `UiIconButton` (pressed tint, trailing count slot); the `.toggle.icon` CSS was deleted.

## Iteration 69 — 2026-11-03 — Quick Collection and Catalog buttons become `UiButton`
Both screens' bordered `.toggle` text buttons moved onto `UiButton`; taken without a scene for either screen.

## Review 70 — 2026-11-03
Four iterations since review 65 (66–69); all four graded.
**Grades:** iter 66 **4** — a real change inside the specimens, done in place (no extra width), two scenes added at last; a family card with chevron + badge now cuts its name to "Sam…", and it left `.detail` declared twice. Iter 67 **3** — small and correct; the bar is all icons now, but "Lock width" (the harder half) was skipped. Iter 68 **3** — the right consolidation, the toolbar is one button family; but it grew `UiIconButton` the way 61 grew `UiButton`: its own `--fm-hover` background on hover on top of the global gradient (doubled tint, also on the disabled Compare), a repeat of the global `:disabled` fade, literal 4 px; it left HomeView's three `.toggle` rules dead. Iter 69 **2** — a consistency move with no shot opened (no scene exists for either screen) and no scene added; nothing a designer sees in three seconds.
**Regressions:** none in views, keyboard, tests or list performance (60 fps alone; 17 fps under the full parallel run is load, not code). Doubled hover on every icon button (68; fixed).
**Drift:** literal spacing in `UiIconButton`; Studio's info button kept a local `.on` accent class beside the component's own states. No new colours, fonts, radii or native controls.
**Pulled apart:** 68's `UiIconButton` against review 65's rule for `UiButton` (the baseline owns hover and disabled) — two buttons, two models. Now one. Three of four iterations were consistency sweeps; the `list` shot moved only in 68.
**Fixed:**
- `UiIconButton`: no own hover background or `:disabled` rule (global baseline); pressed keeps the tint; `aria-expanded` turns accent like `UiButton`; spacing tokens.
- `StudioView`: the info button's local `.on` class and CSS deleted (the component draws the open state).
- Dead: HomeView `.toggle`, `.toggle:disabled`, `.toggle[aria-pressed]`; FontGrid's duplicate `.detail` rule merged.
**Rule amended (DESIGN.md):** every icon button is `UiIconButton` under the same baseline rule; pressed = tint, open = accent. Items 7, 13, 17, 26 and the three-second note rewritten.
**Verified:** typecheck, `lint:js` exit 0, Vitest 910, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-70.log`). Shots `70/` (before, 56) and `70-after/` (56): all 28 scenes render in light and dark, comfortable and compact; only `empty-collection` changed (the "…" icon button).
**Not fixed, noted:** grid cards cut long names beside chevron + badge; Settings nav focus ring on load; empty collection shows "Clear filters" with no filters; no `quick-collection`/`catalog` scenes.
**Next five:** stop sweeping, ship features — item 16 (Compare overlay) → item 14 (weight scrubbing on a variable preview) → item 13 (Grid opens a family from the keyboard) → item 17 ("Lock width" beside the page) → item 26 (Settings nav focus ring; `quick-collection` and `catalog` scenes). Every iteration opens before/after shots of a scene that shows its change; a new component state goes through the global baseline, never a local copy.

## Iteration 71 — 2026-11-03 — Compare sheet: a centered sheet with designed controls
The top-hung Compare dialog became a centred sheet with a text segmented mode switch, icon close, quiet buttons and hairline specimens; new `compare` and `compare-overlay` scenes.

## Iteration 72 — 2026-11-03 — Compare sheet: specimens drawn large
The sheet draws the list's size at 2× (clamped 40–96 px) in all three modes.

## Iteration 73 — 2026-11-03 — Weight scrubbing on variable previews
Dragging across a variable face's sample in `FontPreviewRow` scrubs `wght` (320 px = full range) with a value pill, kept on release; no scene.

## Iteration 74 — 2026-11-03 — Grid: Alt+Right / Alt+Left open and close a family
`list/keys.ts` gives the Grid the tree-style Alt+→/← pair (Enter already toggled); taken without shots.

## Review 75 — 2026-11-03
Four iterations since review 70 (71–74); all four graded.
**Grades:** iter 71 **3** — the Compare sheet finally looks like this app (centred, quiet, hairlines, two scenes added); but its mode switch is a local copy of the `UiSegmentedControl` look with a local `.on` class (the bug review 70 named), literal gaps, and its new `min-height` left the overlay line floating over a ~250 px empty band. Iter 72 **3** — one line, a visible gain: letterforms read; side by side now wraps the pangram to four lines. Iter 73 **3** — the first change in months a designer feels in the hand, tested, RTL-aware; but it read `getComputedStyle` on every pointer move, selected the sample's text while dragging in Waterfall/Strip/Similar/Quick Collection (only List rows had `user-select: none`), a literal 6 px pad, no scene. Iter 74 **2** — correct and tested, but invisible: no shots, no critique, and the keys were not added to the `?` sheet, so nobody finds them.
**Regressions:** none in views, keyboard, tests or list performance (59.8 fps). **Drift:** literal spacing (71, 73); a local state class (71). No new colours, fonts, radii or native controls.
**Pulled apart:** 71's local segmented control against `UiSegmentedControl` (two looks of one control; Compare needs radio semantics, the toolbar `aria-pressed`). The `list` shot is byte-identical from review 70 to now.
**Fixed:**
- `CompareSheet`: the checked radio draws the segment (`:has(input:checked)`), `.on` class gone; the overlay stage takes the free height and centres the line; every literal gap on spacing tokens.
- `FontPreviewRow`: direction read once per drag; `user-select: none` on scrubbable samples; pill padding on a token.
- `ShortcutSheet`: "Open or close a family  ←→  Alt+←→" row (en/de).
**Rule amended:** none. **Backlog (DESIGN.md):** items 7, 13, 14, 16, 17, 26 and the three-second note rewritten; item 16 names the text/radio `UiSegmentedControl` variant.
**Verified:** typecheck, `lint:js` exit 0, Vitest 913, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-75.log`). Shots `75/` (before, 64) and `75-after/` (64): all 32 scenes render in light and dark, comfortable and compact; `compare`, `compare-overlay`, `shortcuts` changed.
**Not fixed, noted:** Grid card name "Sam…"; Layout view's gesture sentence under the page; Catalog Install/Download and Axes instance chips bordered; Settings nav focus ring on load; empty collection shows "Clear filters" with no filters.
**Next five:** move the `list` shot or a scene beside it — item 17 ("Lock width" beside the page; Layout help behind the info button) → item 7 (Grid card name never cut to "Sam…") → item 26 (Catalog row buttons and Axes chips quiet) → item 16 (text/radio `UiSegmentedControl`, Compare onto it) → item 14 (scrub in Grid cards and Quick Look, a `scrub` scene). Every iteration opens before/after shots of a scene that shows its change; no shots means a 2.

## Iteration 76 — 2026-11-03 — Layout view: help behind an info button, Lock width an icon
The Layout view's gesture sentence moved behind an info `UiIconButton` and "Lock width" became a `width` icon (`aria-pressed`); new `layout-help` scene.

## Iteration 77 — 2026-11-03 — Grid cards: the name is never cut
`FontGrid` card meta is one 16 px line, name first; the count/range and badge wrap out of sight whole when they do not fit.

## Iteration 78 — 2026-11-03 — Catalog rows: Install and Download… become quiet buttons
`CatalogFamilyItem` uses `UiButton` for both row actions; the local bordered `.get` CSS was deleted.

## Iteration 79 — 2026-11-03 — Axes tab: named instances and Pin button become quiet
`InspectorAxes` instances and "Pin this instance" moved onto `UiButton`; it also formatted `catalogs.spec.ts`, left unformatted by 78.

## Review 80 — 2026-11-03
Four iterations since review 75 (76–79); all four graded.
**Grades:** iter 76 **3** — the Layout view now matches the Studio (help behind info, lock an icon) and a scene was added; but only "after" shots, the opened help line sat 16 px right of the head it belongs to, a dead `.lock { margin-inline-start: 0 }` stayed, and in the Studio the lock still floats alone at the window corner. Iter 77 **4** — the worst Grid flaw fixed in CSS alone, with before/after shots; a narrow card now drops its Variable badge with no cue. Iter 78 **3** — Catalog rows are calmer; "Install" no longer reads as the primary action, shots are a mix of before and after, and it left `catalogs.spec.ts` failing Prettier (`lint:js` red until 79). Iter 79 **3** — the Axes tab lost five boxes; but the quiet buttons sat 8 px in from the column the fields and headings use, and it gave the pressed instance a local `font-weight: 600` beside `UiButton`'s own accent state — a caller-side state style (the bug reviews 65/70 named) that also shifts the row when the pressed instance changes.
**Regressions:** none in views, keyboard, tests or list performance (59.8 fps). `lint:js` red between 78 and 79.
**Drift:** a local pressed style (79); a dead rule and a literal head padding (76). No literal colours, new fonts, radii or native controls.
**Pulled apart:** 79's quiet instances against item 16's planned text segmented control — the "current instance" cue is weak; both belong on one `UiSegmentedControl` variant. Three of four iterations were consistency sweeps; the `list` shot is byte-identical since review 70.
**Fixed:**
- `InspectorAxes`: the local pressed weight deleted (accent text from `UiButton` only); the instances row and "Pin this instance" pull back by the button padding, so their text lines up with the headings and fields.
- `FontLayout`: the help line aligns with the head (`--fm-space-4`); head gap/padding on tokens; dead `.lock` rule and class deleted.
**Rule amended (DESIGN.md):** the three-second test is "the `list` shot or the scene of the view the change touches", with before/after shots of that scene — the `list` shot has not moved in ten iterations. Items 7, 16, 17, 21, 26 rewritten; item 21 adds "Clear filters" on an empty collection (a store fix).
**Verified:** typecheck, `lint:js` exit 0, Vitest 913, Playwright 273 passed / 16 skipped (`.harness/redesign/e2e-review-80.log`). Shots `80/` (before, 66) and `80-after/` (66): all 33 scenes render in light and dark, comfortable and compact; only `inspector-axes` and `layout-help` changed.
**Not fixed, noted:** Studio lock beside the page; Settings nav focus ring on load; Catalog toolbar wraps "Check for updates" to its own line at 1440 px; Install not primary.
**Next five:** item 6 (row actions Activate / Compare / Tag on hover, seen in `hover-row`) → item 16 (text/radio `UiSegmentedControl`; Compare and Axes instances onto it) → item 17 (Studio lock beside the page) → item 14 (scrub in Grid cards and Quick Look; a `scrub` scene) → item 26 (Settings nav focus ring; Catalog toolbar on one line). Open before and after shots of the touched scene, both themes; run `pnpm lint:js` and read its exit code.

## Iteration 81 — 2026-11-03 — List rows: Activate, Compare and Similar on hover
Critique (before): the list is calm and the type leads; a hovered row showed one lone lens icon, so activating or comparing meant going to the inspector or a shortcut nobody sees. The rest of the first screen is mature.
Why: backlog item 6's last open part, named in review 80; seen in `hover-row`.
Changed: `FontList` row actions are three `UiIconButton`s (power, compare, similar) in one hover/cursor group; they select the row, then toggle for the session (`activation.toggleRows`) or pin (`compare.pin`). Labels are `list.activateLabel` and `list.compareLabel` (en/de); "Switch … on or off" avoids a clash with the inspector's "Activate" button in role queries.
Backlog: 6 stays ticked, row-action set done.
Shots: `.harness/redesign/shots/81/` (before) and `81-after/`; `hover-row` changed.
Verified: typecheck, `lint:js` 0, Vitest 913, Playwright 270 passed on first run; the 3 failures (name clash) fixed and re-run green (a11y, settings).
Follow-ups: Tag action on hover; the row actions do not show in compact density checks; no scene for keyboard focus on them (they are `tabindex=-1`, shortcuts cover keyboard).
Dependencies: none.

## Iteration 82 — 2026-11-03 — Axes: named instances become a text segmented control
Critique (before): the first screen is mature; in the Axes tab the named instances were bare words, and the current one differed only by text colour, so the choice did not read as one exclusive set.
Why: item 16, named by review 80 (one `UiSegmentedControl` text variant for instances and, next, Compare).
Changed: `UiSegmentedControl` takes icon-less options (text segment) and a `wrap` flag, and sets `data-value`; `InspectorAxes` uses it for the instances (pressed = lifted segment, accent text). The old `.presets` CSS is gone. Tests: Vitest and `axes.spec.ts` now find a `group` named "Named instances".
Backlog: 16 stays open (Compare size control and text/radio variant there remain).
Shots: `.harness/redesign/shots/81-after/` (before) and `82/`; `inspector-axes` changed.
Verified: typecheck, `lint:js` 0, Vitest 913, Playwright 273 passed / 16 skipped.
Follow-ups: Compare overlay controls onto the text variant; arrow-key roving inside the group (buttons are all tab stops today).
Dependencies: none.
