import { describe, expect, it } from 'vitest';
import type { Axis, FeatureInfo } from '../api/types';
import { glyphAlternates } from './alternates';

const feature = (tag: string, over: Partial<FeatureInfo> = {}): FeatureInfo => ({
  tag,
  name: `name of ${tag}`,
  group: 'stylistic',
  ...over,
});
const axis = (tag: string, min: number | null, max: number | null): Axis => ({
  tag,
  name: `axis ${tag}`,
  min,
  default: min,
  max,
});

describe('glyph alternates', () => {
  it('draws each stylistic feature that replaces the glyph with that feature on', () => {
    const got = glyphAlternates({ replacedBy: [feature('ss01'), feature('swsh')] }, [], []);
    expect(got.map((a) => [a.name, a.fontFeatureSettings])).toEqual([
      ['name of ss01', '"ss01" 1'],
      ['name of swsh', '"swsh" 1'],
    ]);
  });

  it('leaves out a feature that is not stylistic, or that the shaper applies by itself', () => {
    const got = glyphAlternates(
      {
        replacedBy: [
          feature('smcp', { group: 'capitals' }),
          feature('ss02', { automatic: true }),
          feature('ss03'),
        ],
      },
      [],
      [],
    );
    expect(got.map((a) => a.key)).toEqual(['feature-ss03']);
  });

  it('names a feature without a name by its tag', () => {
    const [got] = glyphAlternates({ replacedBy: [feature('ss04', { name: null })] }, [], []);
    expect(got?.name).toBe('ss04');
  });

  it('draws both ends of each axis and keeps the other axes where they were moved', () => {
    const axes = [axis('wght', 100, 900), axis('opsz', 9, 144)];
    const got = glyphAlternates(null, axes, [
      { tag: 'wght', value: 500 },
      { tag: 'opsz', value: 20 },
    ]);
    expect(got.map((a) => [a.name, a.value, a.fontVariationSettings])).toEqual([
      ['axis wght', 100, '"wght" 100, "opsz" 20'],
      ['axis wght', 900, '"wght" 900, "opsz" 20'],
      ['axis opsz', 9, '"wght" 500, "opsz" 9'],
      ['axis opsz', 144, '"wght" 500, "opsz" 144'],
    ]);
  });

  it('leaves out an axis with no range', () => {
    expect(glyphAlternates(null, [axis('wght', 400, 400), axis('wdth', null, 100)], [])).toEqual(
      [],
    );
  });

  it('is empty for a static face no feature replaces', () => {
    expect(glyphAlternates({ replacedBy: [] }, [], [])).toEqual([]);
  });
});
