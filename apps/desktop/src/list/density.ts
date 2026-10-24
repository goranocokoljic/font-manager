// Row density of the font list and the detail table (the Settings `density`).
// The virtualizers take their fixed row height from here, so the rows on screen,
// the spacer and the keyboard paging always agree with what the density draws.
import type { Density } from '../api/types';
import { t } from '../i18n';

/** The density before the settings are read, and the fm-api default. */
export const DEFAULT_DENSITY: Density = 'comfortable';

/** A density and its label in the UI language. */
const density = (value: Density): { value: Density; label: string } => ({
  value,
  get label() {
    return t(`density.${value}`);
  },
});

/** The densities in the order the toolbar, the palette and Settings list them. */
export const DENSITIES: readonly { value: Density; label: string }[] = [
  density('comfortable'),
  density('compact'),
];

/** Row metrics of each density, in CSS px. */
export const DENSITY_ROWS: Record<Density, { tablePx: number; listChromePx: number }> = {
  // A table row; the list row's name label, gap and padding around the sample line.
  comfortable: { tablePx: 32, listChromePx: 30 },
  compact: { tablePx: 24, listChromePx: 20 },
};

/** Height of one detail-table row (also its header). */
export function tableRowHeight(density: Density): number {
  return DENSITY_ROWS[density].tablePx;
}

/**
 * The scroll position that keeps row `index` in view after the row height changed
 * from `beforePx` to `afterPx`: the row stays where it was on screen when it can,
 * and is scrolled in whole when it would not fit there.
 */
export function keepRowInView(
  index: number,
  beforePx: number,
  afterPx: number,
  scrollTop: number,
  height: number,
): number {
  const offset = index * beforePx - scrollTop;
  const kept = Math.max(0, index * afterPx - Math.min(Math.max(offset, 0), height));
  const top = index * afterPx;
  if (top < kept) return top;
  if (top + afterPx > kept + height) return Math.max(0, top + afterPx - height);
  return kept;
}
