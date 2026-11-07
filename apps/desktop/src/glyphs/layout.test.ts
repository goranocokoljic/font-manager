import { describe, expect, it } from 'vitest';
import {
  GLYPH_COLUMNS,
  codePointLabel,
  glyphRowCount,
  glyphRowHeight,
  moveCursor,
  rowOf,
} from './layout';

describe('glyph grid layout', () => {
  it('always has twelve columns of square cells in whole pixels', () => {
    expect(GLYPH_COLUMNS).toBe(12);
    expect(glyphRowHeight(720)).toBe(60);
    expect(glyphRowHeight(725)).toBe(60);
    expect(glyphRowHeight(0)).toBe(1);
  });

  it('counts the rows a list fills', () => {
    expect(glyphRowCount(0)).toBe(0);
    expect(glyphRowCount(1)).toBe(1);
    expect(glyphRowCount(12)).toBe(1);
    expect(glyphRowCount(13)).toBe(2);
    expect(rowOf(11)).toBe(0);
    expect(rowOf(12)).toBe(1);
  });

  it('moves the cursor by cell and by row', () => {
    expect(moveCursor(30, 5, 'ArrowRight')).toBe(6);
    expect(moveCursor(30, 5, 'ArrowLeft')).toBe(4);
    expect(moveCursor(30, 5, 'ArrowDown')).toBe(17);
    expect(moveCursor(30, 17, 'ArrowUp')).toBe(5);
    expect(moveCursor(30, 5, 'Home')).toBe(0);
    expect(moveCursor(30, 5, 'End')).toBe(29);
  });

  it('stops at the ends of the list', () => {
    expect(moveCursor(30, 0, 'ArrowLeft')).toBe(0);
    expect(moveCursor(30, 29, 'ArrowRight')).toBe(29);
    expect(moveCursor(30, 5, 'ArrowUp')).toBe(5);
    expect(moveCursor(30, 25, 'ArrowDown')).toBe(25);
  });

  it('goes to the last cell when the row below is shorter', () => {
    expect(moveCursor(30, 23, 'ArrowDown')).toBe(29);
  });

  it('ignores other keys and an empty list', () => {
    expect(moveCursor(30, 5, 'a')).toBeNull();
    expect(moveCursor(0, 0, 'ArrowDown')).toBeNull();
  });

  it('writes a code point as Unicode does', () => {
    expect(codePointLabel(0x17d)).toBe('U+017D');
    expect(codePointLabel(0x1f600)).toBe('U+1F600');
  });
});
