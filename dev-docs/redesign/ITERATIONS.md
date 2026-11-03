# Iteration log

Append-only. Newest at the bottom. Every iteration and every review pass adds an entry.
Backlog ticks live here, not in `DESIGN.md`.

## Backlog status
<!-- Keep this list in sync: mark an item [x] in the iteration that finishes it. -->
- [x] 1 Design tokens + theme + UI font + ui/ Select (iterations 2–3; the gate is lifted)
- [x] 2 Filters panel instead of the wall (iteration 4; UiDrawer built, chips stay in FilterChips; human feature names moved to item 8)
- [x] 3 Preview bar — sample text as the hero control (iteration 6; first pass: one row + Options row; UiField/UiSlider/UiToggle not built)
- [ ] 4 Dark by default + app shell (iterations 7, 11: toolbar icons + UiSegmentedControl, status bar reduced to live state; dark-first launch, collapsible panes remain)
- [ ] 5 Sidebar redesign (iteration 12: collection row actions in a "…" menu; iteration 8: UiIconButton + section-header and source-row icon actions; section icons, collapsible sections, smart subtype, Problems dedupe remain)
- [ ] 6 List rows (iteration 9 + review 10: UiBadge and UiChevron everywhere; hover actions, chevron alignment, sub-row styling remain)
- [ ] 7 Grid cards
- [ ] 8 Inspector
- [ ] 9 Find similar as a mode
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
**Critique:** The list is calm and the specimens lead. The worst thing left on screen is the bottom bar: `App: Font Manager (mock) 0.1.0  API: 0.23.0  Platform: linux` is developer data at the same weight as the actions. The sidebar still has bordered "Turn on" buttons and a stray "Smart collections" header with nothing under it.
**Why:** Backlog item 4 (rest): status bar to live state, version strings to About. Visible in the first three seconds: the bottom line is now `0 fonts active · 5 problems · Conflicts` on the left and `Commands Ctrl+K · Settings Ctrl+,` on the right.
**What changed:** `HomeView` status slot reordered (live state left, hints right via `.status-end`); the app/API/platform list moved to Settings › About (`data-testid="app-info"` kept). Tests: `App.test.ts` now asserts the bar has no version strings; e2e `app`, `rtl`, `settings` read app info from About.
**Ticked:** none (item 4 still open). **Shots:** `.harness/redesign/shots/11/` (before), `11a/` (after).
**Verified:** typecheck, lint:js, Vitest 906, Playwright 272 passed / 16 skipped.
**Follow-ups:** dark-first launch (first-run theme is "system"; the e2e theme tests assume system) and collapsible panes are the remainder of item 4. Settings link sits ~2 px higher than Commands in the bar (kbd height); align in the polish pass.
**Dependencies:** none. **Reverted:** no.

## Iteration 12 — 2026-11-03 — Collections: six row buttons become one "…" menu
**Critique:** The list is calm, but an opened collection grows a block of six bordered text buttons (New inside, Rename, Up, Down, Export…, Delete) that is the loudest chrome on screen. "Turn on" buttons and an empty "Smart collections" header are next.
**Why:** Backlog item 5 (rest). After: the open collection row shows its count plus a quiet "…" and stays one line tall.
**What changed:** `CollectionsSidebar` row actions moved into a `UiPopover` menu (`role=menu`, arrows/Home/End/Esc, focus returns to the "…"); new `more` icon; `collections.moreLabel` in en/de. The "…" shows on hover, focus and the open row. Tests: Vitest and e2e `collections`, `smart-rules` open the menu first.
**Ticked:** none (item 5 still open). **Shots:** `.harness/redesign/shots/12/` (before), `12a/` (after).
**Verified:** typecheck, lint:js, Vitest, Playwright 272 passed / 16 skipped (one export test fixed, re-run green).
**Follow-ups:** "Turn on" catalog buttons; smart subtype instead of the "Smart" pill (still a local `.badge` in this file); empty "Smart collections" header; Problems dedupe. A shared `UiMenu` could replace the hand-made menu if a second menu appears.
**Dependencies:** none. **Reverted:** no.

## Iteration 13 — 2026-11-03 — Sidebar: "Turn on" becomes an icon, empty header gone
**Critique:** The list is calm and specimens lead. The sidebar is the noisiest part: two bordered "Turn on" buttons with a grey line under each, an empty "Smart collections" header, and a "Smart" pill. The Sources "Fonts" row still has icon actions floating on their own line.
**Why:** Backlog item 5 (rest). After: the Online catalogs group is three plain rows (power icon on the off ones) and the empty header is gone. Dark-first launch (item 4) is blocked: the default theme comes from the settings the API returns (`theme: 'system'` in the mock / Rust default), not from the UI.
**What changed:** `OnlineCatalogsSidebar` uses `UiIconButton` (new `power` icon; tooltip "Turn on — Connects to the internet"; hint stays as the accessible description, visually hidden). `CollectionsSidebar` shows a sparkles icon (hidden "Smart" text kept) instead of the pill; its local `.badge` is deleted. `HomeView` drops the empty groups list. One Vitest line updated.
**Ticked:** none (item 5 still open). **Shots:** `.harness/redesign/shots/13/` (before), `13a/` (after).
**Verified:** typecheck, lint:js (only the old `collections.spec.ts` format warning), Vitest 906, Playwright 272 passed.
**Follow-ups:** Sources "Fonts" row actions on their own line; Problems header vs smart "Problems"; section icons and collapsible sections. Dark-first launch needs the API default theme changed to `dark` (API task).
**Dependencies:** none. **Reverted:** no.
