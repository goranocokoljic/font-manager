import { describe, expect, it } from 'vitest';
import { poolSize, scrollToShow, windowRange } from './virtual';

describe('windowRange', () => {
  it('covers the viewport plus the overscan, always one pool long', () => {
    // 600 / 50 = 12 rows, +1 partly visible, +2 overscan each side = 17.
    expect(windowRange(0, 600, 50, 2, 10_000)).toEqual({ start: 0, end: 17 });
    expect(windowRange(1000, 600, 50, 2, 10_000)).toEqual({ start: 18, end: 35 });
    expect(windowRange(499_400, 600, 50, 2, 10_000)).toEqual({ start: 9983, end: 10_000 });
  });

  it('is the whole list when the list is shorter than a pool', () => {
    expect(windowRange(0, 600, 50, 2, 0)).toEqual({ start: 0, end: 0 });
    expect(windowRange(10_000, 600, 50, 2, 5)).toEqual({ start: 0, end: 5 });
  });

  it('always includes every row in the viewport', () => {
    for (const rowPx of [17, 50, 64, 174]) {
      for (const height of [1, 333, 600, 799, 1080]) {
        for (let top = 0; top < rowPx * 40; top += 7) {
          const { start, end } = windowRange(top, height, rowPx, 3, 1_000_000);
          expect(start).toBeLessThanOrEqual(Math.floor(top / rowPx));
          expect(end).toBeGreaterThanOrEqual(Math.ceil((top + height) / rowPx));
        }
      }
    }
  });
});

describe('poolSize', () => {
  it('gives every row of a window its own slot, and a scroll reuses the freed slots', () => {
    for (const rowPx of [17, 50, 174]) {
      for (const height of [1, 333, 600, 1080]) {
        const pool = poolSize(height, rowPx, 3);
        const slots = (top: number) => {
          const { start, end } = windowRange(top, height, rowPx, 3, 1_000_000);
          return new Set(Array.from({ length: end - start }, (_, i) => (start + i) % pool));
        };
        for (let top = 0; top < rowPx * 40; top += 7) {
          const now = slots(top);
          expect(now.size).toBe(pool);
          // Scrolling never needs a slot outside the pool: the same slots, recycled.
          expect(slots(top + rowPx * 3)).toEqual(now);
        }
      }
    }
  });
});

describe('scrollToShow', () => {
  it('scrolls the least that shows the row whole', () => {
    expect(scrollToShow(5, 50, 0, 600)).toBe(0); // already on screen
    expect(scrollToShow(12, 50, 0, 600)).toBe(50); // below: bottom edge aligns
    expect(scrollToShow(2, 50, 400, 600)).toBe(100); // above: top edge aligns
  });
});
