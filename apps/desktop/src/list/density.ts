// Row metrics of the font list and the detail table. Every list has the comfortable
// metrics: the compact density left the interface in E51.5 (a stored `compact` in the
// settings is read and ignored). The virtualizers take their fixed row height from here,
// so the rows on screen, the spacer and the keyboard paging always agree with what is drawn.

/** Height of one detail-table row (also its header), in CSS px. */
export const TABLE_ROW_PX = 32;

/** A list row's name label, gap and padding around the sample line, in CSS px. */
export const LIST_CHROME_PX = 30;

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
