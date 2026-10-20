// Window math of the Glyphs tab's grid. fm-core sends the blocks of the list with
// their counts; the grid is those blocks one after another, each a heading row
// and then rows of `columns` cells. Every row has the same height, so the list's
// window math (`list/virtual.ts`) picks the rows to render.
import type { GlyphBlock } from '../api/types';

/** Size of a cell and height of every row, in CSS px. */
export const GLYPH_CELL = 48;

/** One row of the grid: a block heading, or a run of cells of one block. */
export type GlyphRow =
  | { kind: 'heading'; block: GlyphBlock; start: number }
  | { kind: 'cells'; block: GlyphBlock; start: number; count: number };

/** The rows of the grid; `start` is the list index of a row's first character. */
export interface GlyphLayout {
  rows: GlyphRow[];
  columns: number;
  total: number;
}

/** Cells that fit in `width` px, at least one. */
export function glyphColumns(width: number): number {
  return Math.max(1, Math.floor(width / GLYPH_CELL));
}

/** The grid of `blocks` (in list order) at `columns` cells per row. */
export function glyphLayout(blocks: GlyphBlock[], columns: number): GlyphLayout {
  const rows: GlyphRow[] = [];
  let start = 0;
  for (const block of blocks) {
    if (block.count <= 0) continue;
    rows.push({ kind: 'heading', block, start });
    for (let i = 0; i < block.count; i += columns)
      rows.push({
        kind: 'cells',
        block,
        start: start + i,
        count: Math.min(columns, block.count - i),
      });
    start += block.count;
  }
  return { rows, columns, total: start };
}

/** The row holding list index `index` (a cells row), or -1 outside the list. */
export function rowOf(layout: GlyphLayout, index: number): number {
  if (index < 0 || index >= layout.total) return -1;
  // The last row starting at or before `index`: a heading shares its start with
  // the cells row after it, so this is always a cells row.
  let lo = 0;
  let hi = layout.rows.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (layout.rows[mid]!.start <= index) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** The cells row next to `row` in `step` direction (+1 or -1), skipping headings. */
function nextCellsRow(layout: GlyphLayout, row: number, step: 1 | -1): number {
  for (let r = row + step; r >= 0 && r < layout.rows.length; r += step)
    if (layout.rows[r]!.kind === 'cells') return r;
  return -1;
}

/**
 * The list index a key moves the cursor to from `index`: Left/Right by a cell,
 * Up/Down to the same column of the row above or below (the last cell when that
 * row is shorter), Home/End to the first or last character. `null` for other keys.
 */
export function moveCursor(layout: GlyphLayout, index: number, key: string): number | null {
  const last = layout.total - 1;
  if (last < 0) return null;
  switch (key) {
    case 'ArrowRight':
      return Math.min(index + 1, last);
    case 'ArrowLeft':
      return Math.max(index - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return last;
    case 'ArrowDown':
    case 'ArrowUp': {
      const row = rowOf(layout, index);
      if (row < 0) return 0;
      const target = nextCellsRow(layout, row, key === 'ArrowDown' ? 1 : -1);
      if (target < 0) return index;
      const column = index - layout.rows[row]!.start;
      const to = layout.rows[target] as Extract<GlyphRow, { kind: 'cells' }>;
      return to.start + Math.min(column, to.count - 1);
    }
    default:
      return null;
  }
}

/** `U+0416`: a code point as Unicode writes it. */
export function codePointLabel(cp: number): string {
  return `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`;
}
