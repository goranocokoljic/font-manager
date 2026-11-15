import { describe, expect, it } from 'vitest';
import type { GlyphDetail, Metrics } from '../api/types';
import { glyphMetricRow, perThousandEm } from './metrics';

const glyph = (g: Partial<GlyphDetail>): GlyphDetail => ({
  codePoint: 0x67,
  glyphId: 1,
  name: 'g',
  advanceWidth: 1024,
  producedBy: [],
  replacedBy: [],
  health: { kind: 'ok' },
  leftSideBearing: 68,
  rightSideBearing: 56,
  bounds: null,
  unicodeName: 'LATIN SMALL LETTER G',
  ...g,
});
const metrics = (unitsPerEm: number, xHeight: number | null) =>
  ({ unitsPerEm, xHeight }) as unknown as Metrics;

describe('perThousandEm', () => {
  it('scales font units to 1000 per em and rounds', () => {
    expect(perThousandEm(1024, 2048)).toBe(500);
    expect(perThousandEm(-37, 1000)).toBe(-37);
    expect(perThousandEm(1, 3)).toBe(333);
  });
  it('keeps a missing value missing, and reads a zero em as 1000', () => {
    expect(perThousandEm(null, 1000)).toBeNull();
    expect(perThousandEm(undefined, 1000)).toBeNull();
    expect(perThousandEm(500, 0)).toBe(500);
  });
});

describe('glyphMetricRow', () => {
  it('lists advance, both bearings and x-height of both faces, per 1000 em', () => {
    const row = glyphMetricRow(
      { glyph: glyph({}), metrics: metrics(2048, 964) },
      {
        glyph: glyph({ advanceWidth: 578, leftSideBearing: 46, rightSideBearing: 52 }),
        metrics: metrics(1000, 546),
      },
    );
    expect(row).toEqual([
      { id: 'advance', mine: 500, theirs: 578 },
      { id: 'leftBearing', mine: 33, theirs: 46 },
      { id: 'rightBearing', mine: 27, theirs: 52 },
      { id: 'xHeight', mine: 471, theirs: 546 },
    ]);
  });
  it('a compared face that lacks the glyph still has its x-height; the rest is missing', () => {
    const row = glyphMetricRow(
      { glyph: glyph({}), metrics: metrics(1000, 500) },
      { glyph: null, metrics: metrics(1000, null) },
    );
    expect(row.map((m) => m.theirs)).toEqual([null, null, null, null]);
    expect(row.map((m) => m.mine)).toEqual([1024, 68, 56, 500]);
  });
});
