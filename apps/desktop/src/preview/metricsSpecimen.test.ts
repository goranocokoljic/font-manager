import { describe, expect, it } from 'vitest';
import { metricsSpecimen, xHeightRatio } from './metricsSpecimen';

const metrics = { unitsPerEm: 1000, ascender: 980, descender: -260, xHeight: 480, capHeight: 650 };

describe('metrics specimen', () => {
  it('puts the baseline at the ascender and the guides at cap and x-height, at 96 px', () => {
    const s = metricsSpecimen(metrics)!;
    expect(s.size).toBe(96);
    expect(s.baseline).toBeCloseTo(94.08);
    expect(s.height).toBeCloseTo(119.04);
    expect(s.capLine).toBeCloseTo(94.08 - 62.4);
    expect(s.xLine).toBeCloseTo(94.08 - 46.08);
  });

  it('scales with the em: 2048 units draw the same proportions', () => {
    const s = metricsSpecimen({
      unitsPerEm: 2048,
      ascender: 1900,
      descender: -500,
      xHeight: 1082,
      capHeight: 1456,
    })!;
    expect(s.baseline - s.capLine!).toBeCloseTo((1456 / 2048) * 96);
    expect(s.baseline - s.xLine!).toBeCloseTo((1082 / 2048) * 96);
  });

  it('leaves out a guide the font does not state', () => {
    const s = metricsSpecimen({ ...metrics, xHeight: null, capHeight: null })!;
    expect(s.capLine).toBeNull();
    expect(s.xLine).toBeNull();
    expect(s.baseline).toBeCloseTo(94.08);
  });

  it('keeps a cap height above the ascender inside the box', () => {
    const s = metricsSpecimen({ ...metrics, ascender: 500 })!;
    expect(s.capLine).toBe(0);
  });

  it('draws nothing for a face whose metrics could not be read', () => {
    expect(metricsSpecimen({ ...metrics, unitsPerEm: 0 })).toBeNull();
  });
});

describe('x-height ratio', () => {
  it('is the x-height over the em', () => {
    expect(xHeightRatio(metrics)).toBeCloseTo(0.48);
  });
  it('is absent without an x-height or an em', () => {
    expect(xHeightRatio({ ...metrics, xHeight: null })).toBeNull();
    expect(xHeightRatio({ ...metrics, unitsPerEm: 0 })).toBeNull();
  });
});
