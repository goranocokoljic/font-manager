// Window math of the Characters section's grid: the list's characters in twelve columns, one row
// after another. Every row has the same height, so the list's window math (`list/virtual.ts`)
// picks the rows to render.

/** Cells in a row, whatever the width. */
export const GLYPH_COLUMNS = 12;

/** Height of a row in CSS px at a grid `width`: the cells are square, in whole pixels. */
export function glyphRowHeight(width: number): number {
  return Math.max(1, Math.floor(width / GLYPH_COLUMNS));
}

/** The rows that `total` characters fill. */
export function glyphRowCount(total: number): number {
  return Math.ceil(Math.max(0, total) / GLYPH_COLUMNS);
}

/** The row holding list index `index`. */
export function rowOf(index: number): number {
  return Math.floor(index / GLYPH_COLUMNS);
}

/**
 * The list index a key moves the cursor to from `index` in a list of `total`: Left/Right by a
 * cell, Up/Down to the same column of the row above or below (the last cell when that row is
 * shorter), Home/End to the first or last character. `null` for other keys.
 */
export function moveCursor(total: number, index: number, key: string): number | null {
  const last = total - 1;
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
    case 'ArrowUp':
      return index - GLYPH_COLUMNS < 0 ? index : index - GLYPH_COLUMNS;
    case 'ArrowDown':
      return rowOf(index) === rowOf(last) ? index : Math.min(index + GLYPH_COLUMNS, last);
    default:
      return null;
  }
}

/** `U+0416`: a code point as Unicode writes it. */
export function codePointLabel(cp: number): string {
  return `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`;
}
