import { describe, expect, it } from 'vitest';
import {
  CELL,
  cellTemplate,
  clampCell,
  codePointLabel,
  glyphColumns,
  glyphFontSize,
  glyphRowCount,
  glyphRowHeight,
  moveCursor,
  rowOf,
  specimenBox,
} from './layout';

describe('glyph grid layout', () => {
  it('has a 48–140 px cell slider with 88 px by default', () => {
    expect(CELL).toMatchObject({ min: 48, max: 140, default: 88 });
    expect((CELL.default - CELL.min) % CELL.step).toBe(0);
    expect(clampCell(20)).toBe(48);
    expect(clampCell(200)).toBe(140);
    expect(clampCell(90.4)).toBe(90);
    expect(clampCell('88')).toBe(88);
    expect(clampCell(Number.NaN)).toBe(88);
    expect(clampCell(undefined)).toBe(88);
  });

  it('gives the CSS grid an auto-fill template with the cell as the minimum', () => {
    expect(cellTemplate(88)).toBe('repeat(auto-fill, minmax(88px, 1fr))');
  });

  it('counts columns as auto-fill does: as many cells of at least the size as fit', () => {
    expect(glyphColumns(720, 88)).toBe(8);
    expect(glyphColumns(704, 88)).toBe(8);
    expect(glyphColumns(703.9, 88)).toBe(7);
    expect(glyphColumns(720, 48)).toBe(15);
    expect(glyphColumns(40, 88)).toBe(1);
    expect(glyphColumns(0, 88)).toBe(1);
  });

  it('makes a wider window give larger cells, not only more of them', () => {
    // 760 px holds 8 cells of 95 px; 790 px still 8, now of 98 px.
    const narrow = glyphRowHeight(760, glyphColumns(760, 88));
    const wide = glyphRowHeight(790, glyphColumns(790, 88));
    expect(glyphColumns(790, 88)).toBe(glyphColumns(760, 88));
    expect(wide).toBeGreaterThan(narrow);
    // Every cell is at least the slider's size.
    for (const w of [300, 529, 720, 1000, 1999])
      for (const c of [48, 88, 140])
        expect(glyphRowHeight(w, glyphColumns(w, c))).toBeGreaterThanOrEqual(Math.min(c, w) - 1);
  });

  it('draws the glyph at half the cell', () => {
    expect(glyphFontSize(88)).toBe(44);
    expect(glyphFontSize(48)).toBe(24);
    expect(glyphFontSize(141)).toBe(71);
    expect(glyphFontSize(0)).toBe(1);
  });

  it('has square rows in whole pixels and counts them by the columns', () => {
    expect(glyphRowHeight(720, 8)).toBe(90);
    expect(glyphRowHeight(725, 8)).toBe(90);
    expect(glyphRowHeight(0, 8)).toBe(1);
    expect(glyphRowCount(0, 8)).toBe(0);
    expect(glyphRowCount(1, 8)).toBe(1);
    expect(glyphRowCount(8, 8)).toBe(1);
    expect(glyphRowCount(9, 8)).toBe(2);
    expect(glyphRowCount(812, 15)).toBe(55);
    expect(rowOf(7, 8)).toBe(0);
    expect(rowOf(8, 8)).toBe(1);
  });

  it('moves the cursor by a cell and by a row of the current width', () => {
    expect(moveCursor(30, 5, 'ArrowRight', 12)).toBe(6);
    expect(moveCursor(30, 5, 'ArrowLeft', 12)).toBe(4);
    expect(moveCursor(30, 5, 'ArrowDown', 12)).toBe(17);
    expect(moveCursor(30, 5, 'ArrowDown', 8)).toBe(13);
    expect(moveCursor(30, 17, 'ArrowUp', 12)).toBe(5);
    expect(moveCursor(30, 13, 'ArrowUp', 8)).toBe(5);
    expect(moveCursor(30, 5, 'Home', 8)).toBe(0);
    expect(moveCursor(30, 5, 'End', 8)).toBe(29);
  });

  it('stops at the edges', () => {
    expect(moveCursor(30, 0, 'ArrowLeft', 12)).toBe(0);
    expect(moveCursor(30, 29, 'ArrowRight', 12)).toBe(29);
    expect(moveCursor(30, 5, 'ArrowUp', 12)).toBe(5);
    expect(moveCursor(30, 25, 'ArrowDown', 12)).toBe(25);
  });

  it('goes to the last cell when the row below is shorter', () => {
    expect(moveCursor(30, 23, 'ArrowDown', 12)).toBe(29);
  });

  it('ignores other keys and empty lists', () => {
    expect(moveCursor(30, 5, 'a', 12)).toBeNull();
    expect(moveCursor(0, 0, 'ArrowDown', 12)).toBeNull();
  });

  it('writes a code point as Unicode does', () => {
    expect(codePointLabel(0x17d)).toBe('U+017D');
    expect(codePointLabel(0x1f600)).toBe('U+1F600');
  });
});

describe('the specimen box', () => {
  const metrics = {
    unitsPerEm: 1000,
    ascender: 900,
    descender: -250,
    xHeight: 520,
    capHeight: 720,
  };

  it('runs from the ascender to the descender, so descenders stay inside', () => {
    const box = specimenBox(metrics, 100)!;
    expect(box.height).toBe(115);
    expect(box.baseline).toBe(90);
    expect(box.height - box.baseline).toBe(25);
  });

  it('places the rulers at ascender, x-height, baseline and descender, top to bottom', () => {
    const box = specimenBox(metrics, 100)!;
    expect(box.rulers).toEqual([
      { id: 'ascender', top: 0 },
      { id: 'xHeight', top: 38 },
      { id: 'baseline', top: 90 },
      { id: 'descender', top: 115 },
    ]);
  });

  it('does not use a line height: a tall descender makes a taller box', () => {
    const deep = specimenBox({ ...metrics, descender: -400 }, 100)!;
    expect(deep.height).toBe(130);
    expect(deep.baseline).toBe(90);
  });

  it('leaves the x-height ruler out when the face declares none', () => {
    const box = specimenBox({ ...metrics, xHeight: null }, 100)!;
    expect(box.rulers.map((r) => r.id)).toEqual(['ascender', 'baseline', 'descender']);
    expect(specimenBox({ ...metrics, xHeight: 0 }, 100)!.rulers).toHaveLength(3);
  });

  it('gives no box for metrics it cannot place', () => {
    expect(specimenBox(undefined, 100)).toBeNull();
    expect(specimenBox({ ...metrics, unitsPerEm: 0 }, 100)).toBeNull();
    expect(specimenBox({ ...metrics, ascender: -300 }, 100)).toBeNull();
    expect(specimenBox(metrics, 0)).toBeNull();
  });
});
