// The selection box of the wall (E44.1): a drag on the wall's empty ground draws a box, and
// every card it touches is selected. The cards are virtualized, so the box is checked against
// where each card is laid out (the same places `FontGrid`'s `cardStyle` gives them), not
// against the elements on screen: a box that scrolls past the rendered lines still selects.

/** A box in the wall's content box: inline from its start edge, block from its top. */
export interface Box {
  inline0: number;
  block0: number;
  inline1: number;
  block1: number;
}

/** How the wall lays its cards out (see `cardStyle` in FontGrid.vue). */
export interface WallGeometry {
  count: number;
  columns: number;
  cardWidth: number;
  cardHeight: number;
  /** A card's height plus the gap under it. */
  linePx: number;
  gap: number;
  inset: number;
}

/** A drag shorter than this, in px each way, is a click, not a box. */
export const MARQUEE_MIN_PX = 4;

/** The box between two points, whichever way the drag went. */
export function boxBetween(
  a: { inline: number; block: number },
  b: { inline: number; block: number },
): Box {
  return {
    inline0: Math.min(a.inline, b.inline),
    block0: Math.min(a.block, b.block),
    inline1: Math.max(a.inline, b.inline),
    block1: Math.max(a.block, b.block),
  };
}

/** True when the box is too small to be one (the pointer barely moved). */
export function isClick(box: Box): boolean {
  return box.inline1 - box.inline0 < MARQUEE_MIN_PX && box.block1 - box.block0 < MARQUEE_MIN_PX;
}

/** The indexes of the cards the box touches, in list order. */
export function cardsInBox(box: Box, g: WallGeometry): number[] {
  const cols = Math.max(1, g.columns);
  if (g.count <= 0 || g.linePx <= 0) return [];
  // Lines whose card spans [top, top + cardHeight) meet [block0, block1].
  const firstLine = Math.max(0, Math.floor((box.block0 - g.gap - g.cardHeight) / g.linePx) + 1);
  const lastLine = Math.min(
    Math.ceil(g.count / cols) - 1,
    Math.floor((box.block1 - g.gap) / g.linePx),
  );
  const step = g.cardWidth + g.gap;
  const firstCol = Math.max(0, Math.floor((box.inline0 - g.inset - g.cardWidth) / step) + 1);
  const lastCol = Math.min(cols - 1, Math.floor((box.inline1 - g.inset) / step));
  const out: number[] = [];
  for (let line = firstLine; line <= lastLine; line++) {
    const top = g.gap + line * g.linePx;
    if (top > box.block1 || top + g.cardHeight < box.block0) continue;
    for (let col = firstCol; col <= lastCol; col++) {
      const start = g.inset + col * step;
      if (start > box.inline1 || start + g.cardWidth < box.inline0) continue;
      const i = line * cols + col;
      if (i < g.count) out.push(i);
    }
  }
  return out;
}
