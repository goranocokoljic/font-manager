import { describe, expect, it } from 'vitest';
import type { FeatureInfo } from '../api/types';
import { featureRows, featureSample, isRareFeature } from './featureRows';

const feature = (tag: string, extra: Partial<FeatureInfo> = {}): FeatureInfo => ({
  tag,
  name: null,
  automatic: false,
  defaultOn: false,
  group: null,
  sample: `sample of ${tag}`,
  ...extra,
});

describe('feature rows', () => {
  it('shows ligatures, capitals, figures, fractions and stylistic sets at once', () => {
    for (const tag of [
      'liga',
      'dlig',
      'smcp',
      'c2sc',
      'onum',
      'tnum',
      'pnum',
      'lnum',
      'frac',
      'ss01',
      'ss20',
    ])
      expect(isRareFeature(feature(tag)), tag).toBe(false);
  });

  it('puts character variants, automatic and unlisted tags behind the expander', () => {
    for (const tag of ['cv01', 'cv99', 'ss21', 'jp78', 'aalt'])
      expect(isRareFeature(feature(tag)), tag).toBe(true);
    expect(isRareFeature(feature('locl', { automatic: true }))).toBe(true);
    expect(isRareFeature(feature('mark', { automatic: true }))).toBe(true);
    // A common tag the shaper applies by itself has no switch, so it is rare too.
    expect(isRareFeature(feature('liga', { automatic: true }))).toBe(true);
  });

  it("splits a face's features keeping their order", () => {
    const got = featureRows([feature('cv01'), feature('smcp'), feature('locl'), feature('liga')]);
    expect(got.common.map((f) => f.tag)).toEqual(['smcp', 'liga']);
    expect(got.rare.map((f) => f.tag)).toEqual(['cv01', 'locl']);
  });

  it('takes the sample from data/features, the preview text for a set or a variant', () => {
    expect(featureSample(feature('liga', { sample: 'fi fl' }), 'Preview')).toBe('fi fl');
    expect(featureSample(feature('frac', { sample: '1/2' }), 'Preview')).toBe('1/2');
    expect(featureSample(feature('ss01', { sample: 'Rag Agy 0123' }), 'Preview')).toBe('Preview');
    expect(featureSample(feature('cv07', { sample: 'Rag Agy 0123' }), 'Preview')).toBe('Preview');
    expect(featureSample(feature('xxxx', { sample: null }), 'Preview')).toBe('Preview');
  });
});
