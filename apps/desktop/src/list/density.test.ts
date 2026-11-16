import { describe, expect, it } from 'vitest';
import { keepRowInView, LIST_CHROME_PX, TABLE_ROW_PX } from './density';

describe('row metrics', () => {
  it('are the comfortable ones: the only density since E51.5', () => {
    expect(TABLE_ROW_PX).toBe(32);
    expect(LIST_CHROME_PX).toBe(30);
  });
});

describe('keepRowInView', () => {
  it('keeps the row at the same place on screen', () => {
    // Row 100 at 64 px is 200 px from the top; at 54 px it is still 200 px down.
    expect(keepRowInView(100, 64, 54, 6200, 600)).toBe(5200);
    expect(keepRowInView(100, 54, 64, 5200, 600)).toBe(6200);
  });

  it('scrolls a row back in that was off screen', () => {
    // Above the viewport: it becomes the top row.
    expect(keepRowInView(10, 32, 24, 1000, 600)).toBe(240);
    // Below the viewport: it becomes the bottom row, whole.
    expect(keepRowInView(100, 24, 32, 0, 600)).toBe(100 * 32 + 32 - 600);
  });

  it('shows a row whole when it grows past the bottom edge', () => {
    // Row 20 at 29 px sits 580 px down; at 64 px it would end at 644 px, past 600.
    const top = keepRowInView(20, 29, 64, 0, 600);
    expect(top).toBe(20 * 64 + 64 - 600);
  });

  it('never scrolls above the top', () => {
    expect(keepRowInView(0, 32, 24, 0, 600)).toBe(0);
    expect(keepRowInView(2, 64, 30, 0, 600)).toBe(0);
  });
});
