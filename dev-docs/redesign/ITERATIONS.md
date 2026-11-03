# Iteration log

Append-only. Newest at the bottom. Every iteration and every review pass adds an entry.
Backlog ticks live here, not in `DESIGN.md`.

## Backlog status
<!-- Keep this list in sync: mark an item [x] in the iteration that finishes it. -->
- [x] 1 Design tokens + theme + UI font + ui/ Select (iterations 2–3; the gate is lifted)
- [x] 2 Filters panel instead of the wall (iteration 4; UiDrawer built, chips stay in FilterChips; human feature names moved to item 8)
- [x] 3 Preview bar — sample text as the hero control (iteration 6; first pass: one row + Options row; UiField/UiSlider/UiToggle not built)
- [ ] 4 Dark by default + app shell (iterations 7, 11: toolbar icons + UiSegmentedControl, status bar reduced to live state; collapsible panes remain; dark-first launch blocked on the API default theme)
- [ ] 5 Sidebar redesign (iterations 8, 12, 13, 19: Fonts row actions inline on hover; UiIconButton, collection "…" menu, catalog power icons, smart sparkle; Project sets counts, section icons, collapsible sections, Problems dedupe remain)
- [ ] 6 List rows (iterations 9, 14 + review 10: UiBadge, UiChevron, hover Find-similar icon; chevron alignment, sub-row styling remain)
- [x] 7 Grid cards (iteration 16: sample ends on a whole word that fits; hover weight range remains)
- [ ] 8 Inspector (iteration 17: Info shows About + License + Your notes first, raw data behind "Technical details"; Glyphs clipped column, Features names remain)
- [ ] 9 Find similar as a mode (iteration 18: results replace the list, source pinned, score bar, only differing reasons; grouping by family remains)
- [ ] 10 Language & script filter
- [ ] 11 "Renders my text" filter + visible missing glyphs
- [ ] 12 Quick Look, finished
- [ ] 13 Keyboard model
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
Critique: the grid showed "The quick b" on every card, a cut mid-letter that looks like a bug. Chrome is calm; the inspector Info is still a raw dump. Cards are fine otherwise (dashed border on face cards is odd).
Why: backlog item 7, the worst visible flaw in any view.
Changed: `list/grid.ts` gets `cardChars` (characters that fit the card width) and `cardText` now ends on a whole word; `FontGrid` passes the fit. Grid now shows "The quick". 3 new Vitest cases.
Verified: typecheck, `lint:js` exit 0, Vitest 909, Playwright 272 passed (`.harness/redesign/e2e-16.log`). Shots `16/`.
Follow-ups: hover reveals weight range; dashed border on style cards; inspector Info grouping (item 8) next.

## Iteration 17 — 2026-11-03 — Inspector Info: what matters first, raw data folded
Critique: the Info tab opened on a wall of raw fields (name IDs, vendor ID, copyright) before Designer or Foundry. Hash and path sat at the same weight as everything else. The list and grid are calm now; the inspector is the loudest chrome left.
Why: backlog item 8; the right pane is visible in every first-screen shot.
Changed: `InspectorInfo` order is now header, About (Designer, Foundry, Version), License, Your notes, then a "Technical details" expander (`UiChevron`, `aria-expanded`) holding Names, File, Style, Metrics, Characters. New `info.about` / `info.technical` strings (en, de). `inspector.spec` and `rtl.spec` open the expander first.
Verified: typecheck, `lint:js` exit 0, Vitest 909, Playwright 271 passed + 1 flaky (`app.spec` theme test passed alone; `.harness/redesign/e2e-17.log`). Shots `17/`.
Follow-ups: License URL wraps mid-word; the license editor form is still tall (fold it); Glyphs clipped column; Features human names; Axes tab.

## Iteration 18 — 2026-11-03 — Find similar becomes a mode, not a split
Critique: the similar panel halved the list and put three rows under a mostly empty pane. Every result said the same "Why" line, and the close was a text "×". The rest of the screen is calm; this was the roughest surface left.
Why: backlog item 9. Visible the moment a designer presses S.
Changed: `SimilarPanel` fills the main area; `HomeView` hides the list (`v-show`, so state and focus survive) while it is open. The source face is pinned on top. Each result has a quiet score bar. "Why" shows only reasons that differ between results. Close is a `UiIconButton` (new `close` icon). Esc returns to the list at the picked face. `similar.spec` and `SimilarPanel.test` updated on purpose (list check after Esc; shared reasons dropped).
Verified: typecheck, `lint:js` exit 0, Vitest 909, Playwright 272 passed (`.harness/redesign/e2e-18.log`). Shots `18/`.
Follow-ups: group results by family (closest face shown); the list `list` shot is unchanged (mode shows only in `similar`); inspector license form still tall.

## Iteration 19 — 2026-11-03 — Sources: row actions sit on the row
Critique: the first screen shows a floating strip of three icons under "Fonts", a line of chrome with no label. The list itself is calm; the right pane Info is still long (license form, wrapped URL). Sidebar rows have uneven heights because of that strip.
Why: backlog item 5 rest; the only visible oddity in the `list` shot.
Changed: `SourcesSidebar` puts Rescan / Pause / Remove inside the row, replacing the count on hover or keyboard focus (same grid cell, 120 ms fade). Rows keep one height; the sidebar is 21 px shorter. The local `.badge` became `UiBadge`.
Verified: typecheck, `lint:js` exit 0, Vitest 909, Playwright 3 failed on first run (invisible buttons had `pointer-events: none`; removed), then sources + a11y specs 36 passed. Full run before the fix: 269 passed (`.harness/redesign/e2e-19.log`). Shots `19/`, `19-after/`.
Follow-ups: Project sets count not right-aligned; license URL wraps mid-word; fold the license form; Glyphs clipped column; Features human names.
