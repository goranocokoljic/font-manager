# Redesign target images

Exports of the concept canvas, light theme, exported at 2× (2880 px wide for a 1440 px board):
https://claude.ai/artifact/4EtzdBCYU4LWsxaXNQKvNv

## How to export

Boards 1, 2 and 3 have several states. Each state is a Tweak, so an export is reproducible:

1. Select the board on the canvas and open its **Tweaks** (the Tweaks button on the artboard).
2. Under **Export state** choose the state from the table below (leave the other tweaks alone).
3. **Share › Export** the board as PNG at 2× and save it under the file name in the table.
4. Put the files in this folder; fill in the export date at the bottom.

Boards 4, 5 and 6 have one state each: export them as they are. Board 7 (Compare) has three states, set by its height, mode and toast tweaks; board 8 (Letters) has four, set by its state tweak; boards 9–11 have one, three and one. Board 12 (Preview options) is 1120 px wide, so its exports are 2240 px; its two popover files are the popover alone, cropped (848 px). Board 14 (Duplicates) has nine states, set by its state tweak; its second board, the ways in, has one state and is 1280 px wide, so its export is 2560 px. Board 15 (Settings) has seven states and board 16 (Problems) two, each set by its state tweak.

## The files

| File | Board | Export state | Shows | Story |
| --- | --- | --- | --- | --- |
| `1-wall-word.png` | 1 · Type wall | preset = Word | your word, four columns, size slider | E41.1, E41.2, E41.3, E41.4 |
| `1-wall-pangram.png` | 1 | preset = Pangram | two columns, one line per family | E41.2 |
| `1-wall-alphabet.png` | 1 | preset = Alphabet | two columns, the character set wrapped | E41.2 |
| `1-wall-numerals.png` | 1 | preset = Numerals | four columns, figures | E41.2 |
| `1-wall-paragraph.png` | 1 | preset = Paragraph | two columns of body text (same layout as board 3) | E41.2, E43.1 |
| `2-family-styles.png` | 2 · Family page | section = Styles, stylesView = named | hero, axes, named styles, rail | E42.1, E42.2, E42.3, E42.9 |
| `2-family-ramp.png` | 2 | section = Styles, stylesView = ramp | the weight ramp | E42.3 |
| `2-family-language.png` | 2 | section = Styles, language = Serbian (Latin) | the page previewing in a chosen language | E42.9 |
| `2-family-text.png` | 2 | section = Text | the reading test | E42.4 |
| `2-family-characters.png` | 2 | section = Characters | glyph grid and detail | E42.5 |
| `2-family-waterfall.png` | 2 | section = Waterfall | the hero's text and weight on the wall's size ladder, 10 → 96 px | E50.10 |
| `2-family-features.png` | 2 | section = Features | features off and on | E42.6 |
| `2-family-pairings.png` | 2 | section = Pairings | pairing cards | E42.7 |
| `2-family-info.png` | 2 | section = Info | design, licence, files, metrics | E42.8 |
| `3-wall-text.png` | 3 · Your own text | hideIncomplete = off | reading rows, tofu on the gap families | E43.1, E43.3, E43.4 |
| `3-wall-text-complete.png` | 3 | hideIncomplete = on | only families that render every glyph | E43.3 |
| `4-drag.png` | 4 · Select and drag | — | selection, ghost, drop target, selection bar | E44.1, E44.2 |
| `5-studio.png` | 5 · Pairing Studio | — | toolbar, roles rail, paper | E45.1, E45.2, E45.3 |
| `6-motion.png` | 6 · Motion spec | — | the motion table | E46 (and MOTION.md) |
| `7-compare-partial.png` | 7 · Compare | height = Partial, mode = Stacked, toast = on | sheet at 40 %, wall and toast behind, stacked faces, the similar band in the same text grouped by reason | E48.1, E48.3, E48.4, E48.6 |
| `7-compare-full-grid.png` | 7 | height = Full, mode = Grid, toast = off | the sheet as the whole window, two faces per row | E48.1, E48.2 |
| `7-compare-overlay.png` | 7 | height = Full, mode = Overlay, toast = off | two faces on one baseline | E48.2 |
| `7-compare-stacked-full-height.png` | 7 | height = Full, mode = Stacked, toast = off | the sheet as the whole window, one face per row, the similar band clearly a second list | E48.3 |
| `8-letters-wall.png` | 8 · Match by letters | state = Letters only | the Letters preset: ROag on every card, underlined letters in the field, no chips | E49.7 |
| `8-letters-picker.png` | 8 | state = Picker open | the R picker open under the field: three drawn values with counts, Any | E49.6 |
| `8-letters-filtered.png` | 8 | state = Filtered | R leg straight and O round chosen: accent underlines, two chips, 41 families | E49.6, E49.10 |
| `8-letters-like.png` | 8 | state = Like | the wall sorted by an R like Inter: the card's letters on hover with 'Fonts with an R like this', the 'R like Inter' chip, 'Sorted by R like Inter' in the count line | E49.9 |
| `9-split.png` | 9 · Lines, dot, split Layout, Foundries | foundries = Open | Lines on the left with the chevron gutter and the 3-state dot, the layout pane on the right, Foundries open in the sidebar | E50.2, E50.3, E50.4, E50.5, E50.12 |
| `10-characters-plain.png` | 10 · Characters | state = Plain | scaled cells, hairline grid, the preview with rulers and descenders | E50.7 |
| `10-characters-popover.png` | 10 | state = Popover | the glyph details popover, no metrics | E50.9 |
| `10-characters-compare.png` | 10 | state = Compare metrics | Compare with Inter on: the metrics row and the ghosted glyph | E50.9 |
| `11-sources.png` | 11 · Settings: Sources | scanning = on | source cards, one scanning, one missing, the drop zone | E50.13 |
| `12-preview-options-open.png` | 12 · Preview options | theme = Paper, preset = Pangram | the popover open under its toolbar button on the wall: Text, Size and spacing, Guides; no colour controls | E54.7, E54.8 |
| `12-preview-options-inactive.png` | 12 | theme = Paper, preset = Word | Line height faded, with the line "No effect while the preview is a single line." under it | E54.7, E54.8 |
| `12-preview-options-dark.png` | 12 | theme = Dark, preset = Pangram | the same popover in the dark theme | E54.7, E54.8 |
| `12-preview-options-popover.png` | 12 | theme = Paper, popover only | the popover alone, for the pixel comparison | E54.7, E54.8 |
| `12-preview-options-popover-dark.png` | 12 | theme = Dark, popover only | the popover alone in the dark theme | E54.7, E54.8 |
| `13-setup-folders-empty.png` | 13 · First-run setup | step = Font folders, no folder yet | the setup in the main window: logotype, step indicator, the empty folder zone with "Add folder…", "Skip for now"; the library pane empty | E55.1, E55.4 |
| `13-setup-folders.png` | 13 | step = Font folders, three folders | folder rows (one scanning, in a cloud folder), "Add another folder…", Continue; the count and the first families found so far | E55.4 |
| `13-setup-catalogs-off.png` | 13 | step = Online catalogs, both off | the two catalog switches with the "connects to the internet" line; no folder question | E55.5 |
| `13-setup-catalogs-on.png` | 13 | step = Online catalogs, Google Fonts on | "Where should installed fonts go?" with "Inside my font folder" chosen; the folder tree an install lands in | E55.3, E55.5 |
| `13-setup-updates.png` | 13 | step = Updates | "Check for updates automatically" on, Finish; what a check does | E55.6 |
| `13-setup-from-settings.png` | 13 | reopened from Settings, both catalogs on, "Another folder" | the step names as links, the close button, the chosen folder with "Change…", both catalogs in the tree | E55.4, E55.5 |
| `13-setup-folders-dark.png` | 13 | theme = Dark, step = Font folders, three folders | the folders step in the dark theme | E55.4 |
| `13-setup-catalogs-cloud-dark.png` | 13 | theme = Dark, step = Online catalogs, a cloud folder chosen | the note under the select when the font folder syncs with a cloud service | E55.5 |
| `14-duplicates-overview.png` | 14 · Duplicates | state = Overview | the module with its navigation; identical copies as one decision, the levels to decide, the top folder pairs | E56.10, E56.11 |
| `14-duplicates-folders.png` | 14 | state = Folders | folder pairs with their share, "Keep this side", a system folder that can only be kept | E56.12 |
| `14-duplicates-sets.png` | 14 | state = Sets | the queue and one set: three copies, the recommended one with its reason, missing glyphs as tofu, an active copy that stays | E56.11 |
| `14-duplicates-compare.png` | 14 | state = Compare | two copies side by side in a Serbian (Cyrillic) sample; the differences from languages to format | E56.13 |
| `14-duplicates-plan.png` | 14 | state = Plan | totals, what goes to the trash by folder, what stays and why, the metadata that moves, a rating conflict | E56.15 |
| `14-duplicates-applying.png` | 14 | state = Applying | a running cleanup: progress, the current file, Cancel, skipped so far | E56.15 |
| `14-duplicates-history.png` | 14 | state = History | sessions with "Undo all", single files, a file that cannot come back | E56.15 |
| `14-duplicates-hidden.png` | 14 | state = Hidden | hidden copies with "Show again"; remembered decisions with "Take back" | E56.15 |
| `14-duplicates-rules.png` | 14 | state = Rules | the languages, the guard above the list, the rules in order, the preview before Save | E56.14 |
| `14-duplicates-entry.png` | 14 · Duplicates: the ways in | — | "Duplicates" in the File menu (no sidebar row), the notice after a scan, the "copies" mark on a family, the command palette (the accent rings mark what is new and are not drawn in the app) | E56.15, E57 |
| `15-settings-activation.png` | 15 · Settings | state = Activation | the shell with the x to close; choice cards for the activation mode and for name conflicts, setting rows with switches | E57.3, E57.4 |
| `15-settings-appearance.png` | 15 | state = Appearance | theme cards, light or dark as three choice cards, the language row | E57.4 |
| `15-settings-catalogs.png` | 15 | state = Online catalogs | catalog cards, the folder for installed fonts with its tree, the fetch-missing switch | E57.5 |
| `15-settings-advanced.png` | 15 | state = Advanced | number rows with units, the logs folder, run setup again | E57.5 |
| `15-settings-local-api.png` | 15 | state = Local API | the address, the masked token, regenerate | E57.6 |
| `15-settings-ai-assistants.png` | 15 | state = AI assistants | the switch, the address, the snippet block with a tab per client, the tools that may change things | E57.6 |
| `15-settings-about.png` | 15 | state = About | the logotype panel (the serif wordmark stands in for the logotype), the update row, the three update levels, platform notes | E57.7 |
| `16-problems.png` | 16 · Problems | state = Problems | groups with counts, problem rows with their sentence and path, a sidecar conflict with both values, the duplicates line at the foot | E57.9 |
| `16-problems-empty.png` | 16 | state = Empty | "All good." and the duplicates line | E57.9 |

A UI story is reviewed against its image, not against the previous screenshot. The boards show
the light theme at 1440 wide; the dark theme, compact density and narrow windows follow the
design-system rules in dev-docs/redesign/DESIGN.md.

Export date: 2026-11-10 (`8-letters-like.png`: 2026-11-11; `2-family-waterfall.png`: 2026-11-15; board 12: 2026-11-17; board 13: 2026-11-18; board 14: 2026-11-20).
