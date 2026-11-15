// Window math of the Characters section's grid (E50.7): the CSS lays the cells out with
// `repeat(auto-fill, minmax(<cell>px, 1fr))`, so a row holds as many cells of at least the
// chosen size as fit, and the cells stretch to fill it. A wider window gives larger cells, not
// smaller ones. Every row has the same height, so the list's window math (`list/virtual.ts`)
// picks the rows to render. Also the specimen above the grid: its box and its rulers, from the
// face's vertical metrics.

import type { Metrics } from '../api/types';

/** The cell-size slider: CSS px, the smallest a cell may get (E50.7). */
export const CELL = { min: 48, max: 140, step: 4, default: 88 } as const;

/** A glyph is drawn at this share of its cell. */
export const GLYPH_SCALE = 0.5;

/** `n` as a cell size: a number in the slider's range, the default for anything else. */
export function clampCell(n: unknown): number {
  if (typeof n !== 'number' || !Number.isFinite(n)) return CELL.default;
  return Math.min(CELL.max, Math.max(CELL.min, Math.round(n)));
}

/** The track template of a row: the string the CSS grid gets. */
export function cellTemplate(cell: number): string {
  return `repeat(auto-fill, minmax(${cell}px, 1fr))`;
}

/** Cells in a row of `width` px when a cell is at least `cell` px, as `auto-fill` counts them. */
export function glyphColumns(width: number, cell: number): number {
  return Math.max(1, Math.floor(width / Math.max(1, cell)));
}

/** Height of a row in CSS px: the cells are square, in whole pixels. */
export function glyphRowHeight(width: number, columns: number): number {
  return Math.max(1, Math.floor(width / Math.max(1, columns)));
}

/** The glyph's font size in a cell `cellPx` wide: half the cell, in whole pixels. */
export function glyphFontSize(cellPx: number): number {
  return Math.max(1, Math.round(cellPx * GLYPH_SCALE));
}

/** The rows that `total` characters fill. */
export function glyphRowCount(total: number, columns: number): number {
  return Math.ceil(Math.max(0, total) / Math.max(1, columns));
}

/** The row holding list index `index`. */
export function rowOf(index: number, columns: number): number {
  return Math.floor(index / Math.max(1, columns));
}

/**
 * The list index a key moves the cursor to from `index` in a list of `total`: Left/Right by a
 * cell, Up/Down to the same column of the row above or below (the last cell when that row is
 * shorter), Home/End to the first or last character. `null` for other keys.
 */
export function moveCursor(
  total: number,
  index: number,
  key: string,
  columns: number,
): number | null {
  const last = total - 1;
  if (last < 0) return null;
  const cols = Math.max(1, columns);
  switch (key) {
    case 'ArrowRight':
      return Math.min(index + 1, last);
    case 'ArrowLeft':
      return Math.max(index - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return last;
    case 'ArrowUp':
      return index - cols < 0 ? index : index - cols;
    case 'ArrowDown':
      return rowOf(index, cols) === rowOf(last, cols) ? index : Math.min(index + cols, last);
    default:
      return null;
  }
}

/** `U+0416`: a code point as Unicode writes it. */
export function codePointLabel(cp: number): string {
  return `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`;
}

/** A ruler of the specimen: which line, and its distance from the box's top in CSS px. */
export interface SpecimenRuler {
  id: 'ascender' | 'xHeight' | 'baseline' | 'descender';
  top: number;
}

/** The specimen's box: its height, where the baseline sits, and its rulers. */
export interface SpecimenBox {
  height: number;
  baseline: number;
  rulers: SpecimenRuler[];
}

/**
 * The specimen's box at `sizePx` from the face's ascender and descender, not from a line
 * height, so descenders (g j p q y) stay inside it. `null` when the metrics cannot place it:
 * no units per em, or an ascender not above the descender. The x-height ruler is left out when
 * the face declares no x-height.
 */
export function specimenBox(
  metrics: Metrics | null | undefined,
  sizePx: number,
): SpecimenBox | null {
  if (!metrics || !(metrics.unitsPerEm > 0) || !(sizePx > 0)) return null;
  const { ascender, descender, xHeight, unitsPerEm } = metrics;
  if (!Number.isFinite(ascender) || !Number.isFinite(descender) || ascender <= descender)
    return null;
  const scale = sizePx / unitsPerEm;
  const at = (units: number) => Math.round((ascender - units) * scale * 100) / 100;
  const rulers: SpecimenRuler[] = [{ id: 'ascender', top: 0 }];
  if (xHeight !== null && xHeight > 0 && xHeight < ascender)
    rulers.push({ id: 'xHeight', top: at(xHeight) });
  rulers.push({ id: 'baseline', top: at(0) }, { id: 'descender', top: at(descender) });
  return { height: at(descender), baseline: at(0), rulers };
}
