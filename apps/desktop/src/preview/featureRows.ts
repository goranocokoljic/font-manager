// The rows of the family page's Features section (E42.6): which features show at once, which sit
// behind "N more", and the text each row shows. Presentation only: the names, groups and samples
// come from fm-core (`data/features`, E42.11); this only picks what to show first.
import type { FeatureInfo } from '../api/types';

/**
 * Features a designer reaches for: ligatures, capitals, figures, fractions, positions, swashes
 * and kerning. Stylistic sets (`ss01`–`ss20`) also show at once; any other tag is rare.
 */
const COMMON: ReadonlySet<string> = new Set([
  'afrc',
  'c2pc',
  'c2sc',
  'calt',
  'case',
  'clig',
  'cpsp',
  'cswh',
  'dlig',
  'dnom',
  'frac',
  'hlig',
  'kern',
  'liga',
  'lnum',
  'numr',
  'onum',
  'ordn',
  'pcap',
  'pnum',
  'salt',
  'sinf',
  'smcp',
  'subs',
  'sups',
  'swsh',
  'titl',
  'tnum',
  'unic',
  'zero',
]);

/** `ss01`–`ss20` and `cv01`–`cv99`: each font gives them its own use, so no sample fits them. */
const STYLISTIC_SET = /^ss(0[1-9]|1\d|20)$/;
const CHARACTER_VARIANT = /^cv(0[1-9]|[1-9]\d)$/;

/** Whether `feature` sits behind the "N more" expander: automatic, `cvXX`, `locl` and the like. */
export function isRareFeature(feature: FeatureInfo): boolean {
  if (feature.automatic) return true;
  return !COMMON.has(feature.tag) && !STYLISTIC_SET.test(feature.tag);
}

/** The features of a face split into the rows shown at once and the rare ones, in their order. */
export function featureRows(features: readonly FeatureInfo[]): {
  common: FeatureInfo[];
  rare: FeatureInfo[];
} {
  const common: FeatureInfo[] = [];
  const rare: FeatureInfo[] = [];
  for (const f of features) (isRareFeature(f) ? rare : common).push(f);
  return { common, rare };
}

/**
 * The text a row shows off and on: the sample `data/features` gives the tag (ligature words for
 * `liga`, digits for `onum`, fractions for `frac`). A stylistic set or character variant, and a
 * tag without a sample, show the preview text instead.
 */
export function featureSample(feature: FeatureInfo, previewText: string): string {
  if (STYLISTIC_SET.test(feature.tag) || CHARACTER_VARIANT.test(feature.tag)) return previewText;
  return feature.sample || previewText;
}
