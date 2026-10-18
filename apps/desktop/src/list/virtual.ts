// Window math of a virtualized list with fixed-height rows. Only the rows in the
// window exist in the DOM. Rows are keyed by slot (`index % pool`), so a row that
// scrolls out at one edge is reused for the row that scrolls in at the other edge:
// Vue patches the same elements instead of creating new ones (row recycling).

/** The rows to render: `start` (inclusive) to `end` (exclusive). */
export interface RowRange {
  start: number;
  end: number;
}

/**
 * How many rows a window holds: the viewport's rows (a partly visible one at each
 * edge included) plus `overscan` beyond each edge.
 */
export function poolSize(height: number, rowPx: number, overscan: number): number {
  return Math.ceil(height / rowPx) + 1 + 2 * overscan;
}

/**
 * The rows to render: always `poolSize` rows (or the whole list when shorter),
 * starting `overscan` rows above the viewport. Because the window length never
 * changes, scrolling by k rows frees exactly the k slots the k new rows need.
 */
export function windowRange(
  scrollTop: number,
  height: number,
  rowPx: number,
  overscan: number,
  count: number,
): RowRange {
  const pool = poolSize(height, rowPx, overscan);
  const first = Math.max(0, Math.floor(scrollTop / rowPx) - overscan);
  const start = Math.min(first, Math.max(0, count - pool));
  return { start, end: Math.min(count, start + pool) };
}

/** The scroll position that shows row `index` whole, moving as little as possible. */
export function scrollToShow(
  index: number,
  rowPx: number,
  scrollTop: number,
  height: number,
): number {
  const top = index * rowPx;
  if (top < scrollTop) return top;
  const bottom = top + rowPx;
  if (bottom > scrollTop + height) return Math.max(0, bottom - height);
  return scrollTop;
}
