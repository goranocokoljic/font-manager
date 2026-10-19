import { describe, expect, it } from 'vitest';
import { WATERFALL_SIZES, waterfallLinePx, waterfallRowPx } from './waterfall';

describe('waterfall layout', () => {
  it('shows six sizes, smallest first', () => {
    expect(WATERFALL_SIZES).toHaveLength(6);
    expect([...WATERFALL_SIZES].sort((a, b) => a - b)).toEqual([...WATERFALL_SIZES]);
    expect(new Set(WATERFALL_SIZES).size).toBe(6);
  });

  it('a block is as high as its header and all six lines', () => {
    const lines = WATERFALL_SIZES.reduce((n, s) => n + waterfallLinePx(s, 1.2), 0);
    expect(waterfallRowPx(1.2)).toBe(36 + lines);
    // Every line holds its text at the line height.
    for (const s of WATERFALL_SIZES) expect(waterfallLinePx(s, 1.5)).toBeGreaterThan(s * 1.5);
    expect(waterfallRowPx(2)).toBeGreaterThan(waterfallRowPx(1));
  });
});
