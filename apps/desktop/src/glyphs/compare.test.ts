import { describe, expect, it } from 'vitest';
import type { Metrics } from '../api/types';
import { COMPARE_SIZE, clampSize, compareLayout } from './compare';

const a: Metrics = {
  unitsPerEm: 1000,
  ascender: 900,
  descender: -250,
  xHeight: 520,
  capHeight: 720,
};
// A 2048-unit em with no declared x-height.
const b: Metrics = {
  unitsPerEm: 2048,
  ascender: 1900,
  descender: -500,
  xHeight: null,
  capHeight: 1434,
};

describe('glyph compare layout', () => {
  it('scales each font’s lines by its own units per em above one shared baseline', () => {
    const l = compareLayout(200, [a, b]);
    const y = (font: number, kind: string) =>
      l.lines.find((x) => x.font === font && x.kind === kind)?.y;
    expect(y(0, 'xHeight')).toBeCloseTo(l.baseline - 104);
    expect(y(0, 'capHeight')).toBeCloseTo(l.baseline - 144);
    expect(y(1, 'capHeight')).toBeCloseTo(l.baseline - (1434 / 2048) * 200);
    // A metric the font does not declare draws no line.
    expect(y(1, 'xHeight')).toBeUndefined();
    expect(l.lines.map((x) => x.units)).toEqual([520, 720, 1434]);
  });

  it('is tall enough for the highest ascender and the lowest descender of both fonts', () => {
    const tall: Metrics = { ...a, ascender: 1200, descender: -400 };
    const one = compareLayout(100, [a]);
    const two = compareLayout(100, [a, tall]);
    expect(two.baseline - one.baseline).toBeCloseTo(20);
    expect(two.height - two.baseline).toBeGreaterThan(one.height - one.baseline);
    expect(two.height - two.baseline).toBeGreaterThanOrEqual(40);
  });

  it('grows with the em size', () => {
    const small = compareLayout(100, [a, b]);
    const big = compareLayout(400, [a, b]);
    expect(big.width).toBeGreaterThan(small.width);
    expect(big.baseline - big.lines[0]!.y).toBeCloseTo(4 * (small.baseline - small.lines[0]!.y));
  });

  it('keeps the size in the offered range', () => {
    expect(clampSize(10)).toBe(COMPARE_SIZE.min);
    expect(clampSize(9999)).toBe(COMPARE_SIZE.max);
    expect(clampSize(123.4)).toBe(123);
    expect(clampSize(Number.NaN)).toBe(COMPARE_SIZE.initial);
  });
});
