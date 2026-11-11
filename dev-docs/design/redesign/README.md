# Redesign target images

Exports of the concept canvas, light theme, exported at 2× (2880 px wide for a 1440 px board):
https://claude.ai/artifact/4EtzdBCYU4LWsxaXNQKvNv

## How to export

Boards 1, 2 and 3 have several states. Each state is a Tweak, so an export is reproducible:

1. Select the board on the canvas and open its **Tweaks** (the Tweaks button on the artboard).
2. Under **Export state** choose the state from the table below (leave the other tweaks alone).
3. **Share › Export** the board as PNG at 2× and save it under the file name in the table.
4. Put the files in this folder; fill in the export date at the bottom.

Boards 4, 5 and 6 have one state each: export them as they are. Board 7 (Compare) has three states, set by its height, mode and toast tweaks; board 8 (Letters) has four, set by its state tweak.

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

A UI story is reviewed against its image, not against the previous screenshot. The boards show
the light theme at 1440 wide; the dark theme, compact density and narrow windows follow the
design-system rules in dev-docs/redesign/DESIGN.md.

Export date: 2026-11-10 (`8-letters-like.png`: 2026-11-11).
