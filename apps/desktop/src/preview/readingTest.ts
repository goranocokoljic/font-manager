// The Text section of the family page (E42.4): which passage it reads and how each size sets it.
import type { Axis, AxisValue } from '../api/types';
import { PARAGRAPH } from './sampleText';
import { variationSettings } from './variations';

/** The three columns, smallest first; `role` names the label in `family.textRoles`. */
export const READING_SIZES = [
  { px: 12, role: 'captions' },
  { px: 17, role: 'body' },
  { px: 24, role: 'lead' },
] as const;
/** The size the ink block shows. */
export const BODY_PX = 17;

/** Line length in `ch` at the caption size. At the default, three columns fit a 1,000 px window. */
export const MEASURE = { min: 40, max: 90, step: 1, default: 45 } as const;
/** Line height, unitless. */
export const LEADING = { min: 1.1, max: 1.8, step: 0.05, default: 1.45 } as const;

/** What the section reads when the user's preview text is too short to read as a passage. */
export const STOCK_PASSAGE = `${PARAGRAPH} The best type does not call attention to itself.`;
/** Preview text of this many characters or fewer is too short to read as a passage. */
const TOO_SHORT = 80;

/** The user's preview text when it is longer than 80 characters, otherwise the stock passage. */
export function readingPassage(text: string): string {
  const own = text.trim();
  return [...own].length > TOO_SHORT ? own : STOCK_PASSAGE;
}

/**
 * The CSS `font-variation-settings` of the passage at `px`: the axes as the hero has them, with
 * `opsz` at that size, or at the end of the axis when the size is outside it. `normal` for a
 * static face.
 */
export function readingVariation(axes: Axis[], moved: readonly AxisValue[], px: number): string {
  const opsz = axes.find((a) => a.tag === 'opsz');
  if (!opsz) return variationSettings(axes, [...moved]);
  const low = opsz.min ?? px;
  const high = opsz.max ?? px;
  const value = Math.min(high, Math.max(low, px));
  return variationSettings(axes, [{ tag: 'opsz', value }, ...moved]);
}
