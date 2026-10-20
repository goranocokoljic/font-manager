import { describe, expect, it } from 'vitest';
import type { GlyphBlock } from '../api/types';
import { codePointLabel, glyphColumns, glyphLayout, moveCursor, rowOf } from './layout';

const block = (name: string, count: number): GlyphBlock => ({ name, first: 0, last: 0, count });
// Basic Latin: 7 cells (rows of 3: 3, 3, 1); Cyrillic: 4 cells (3, 1).
const layout = glyphLayout([block('Basic Latin', 7), block('Cyrillic', 4)], 3);

describe('glyph grid layout', () => {
  it('lays out each block as a heading and rows of cells', () => {
    expect(layout.rows.map((r) => (r.kind === 'heading' ? r.block.name : r.count))).toEqual([
      'Basic Latin',
      3,
      3,
      1,
      'Cyrillic',
      3,
      1,
    ]);
    expect(layout.rows.map((r) => r.start)).toEqual([0, 0, 3, 6, 7, 7, 10]);
    expect(layout.total).toBe(11);
  });

  it('fits whole cells, at least one', () => {
    expect(glyphColumns(300)).toBe(6);
    expect(glyphColumns(10)).toBe(1);
  });

  it('finds the row of a list index', () => {
    expect([0, 2, 3, 6, 7, 10].map((i) => rowOf(layout, i))).toEqual([1, 1, 2, 3, 5, 6]);
    expect(rowOf(layout, 11)).toBe(-1);
    expect(rowOf(layout, -1)).toBe(-1);
  });

  it('a CJK-sized list is a few thousand rows, not 30k cells', () => {
    const big = glyphLayout([block('CJK Unified Ideographs', 20_992), block('Hangul', 11_172)], 6);
    expect(big.total).toBe(32_164);
    expect(big.rows.length).toBe(2 + Math.ceil(20_992 / 6) + Math.ceil(11_172 / 6));
    expect(rowOf(big, 32_163)).toBe(big.rows.length - 1);
  });

  it('arrow keys move by cell and by row, across block headings', () => {
    const move = (i: number, key: string) => moveCursor(layout, i, key);
    expect(move(0, 'ArrowRight')).toBe(1);
    expect(move(10, 'ArrowRight')).toBe(10);
    expect(move(0, 'ArrowLeft')).toBe(0);
    expect(move(1, 'ArrowDown')).toBe(4);
    // Into a shorter row: its last cell.
    expect(move(5, 'ArrowDown')).toBe(6);
    // Past the heading into the next block, same column.
    expect(move(6, 'ArrowDown')).toBe(7);
    expect(move(8, 'ArrowUp')).toBe(6);
    expect(move(1, 'ArrowUp')).toBe(1);
    expect(move(4, 'Home')).toBe(0);
    expect(move(4, 'End')).toBe(10);
    expect(move(4, 'a')).toBeNull();
    expect(moveCursor(glyphLayout([], 3), 0, 'ArrowRight')).toBeNull();
  });

  it('writes code points as Unicode does', () => {
    expect(codePointLabel(0x416)).toBe('U+0416');
    expect(codePointLabel(0x41)).toBe('U+0041');
    expect(codePointLabel(0x1f600)).toBe('U+1F600');
  });
});
