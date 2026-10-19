// Keeps the scroll position across view switches, by family: every view tells
// the font list store which rows its first visible line holds (`setAnchor`),
// and a view that mounts scrolls the anchor family's line to the top. When the
// rows change under the view (the families come back in another order, a family
// above is expanded) and the anchor family moves to another row, the view
// scrolls it back to the top.
import { watch, type Ref } from 'vue';
import { useFontListStore } from '../stores/fontList';

export interface ScrollAnchor {
  /** Scrolls the anchor family's line to the top of the viewport. */
  restore(): void;
  /** The user scrolled to `scrollTop`: the anchor follows the first visible line. */
  track(scrollTop: number): void;
}

/**
 * `linePx` is the height of a line of the view; `perLine` how many rows a line
 * holds (1, or the grid's columns).
 */
export function useScrollAnchor(
  viewport: Ref<HTMLElement | null>,
  linePx: () => number,
  perLine: () => number = () => 1,
): ScrollAnchor {
  const list = useFontListStore();
  /** The scrollTop `restore` set; its scroll event does not move the anchor. */
  let restored: number | null = null;

  function restore(): void {
    const el = viewport.value;
    const i = list.anchorIndex;
    if (!el || i < 0) return;
    const top = Math.floor(i / perLine()) * linePx();
    if (el.scrollTop === top) return;
    el.scrollTop = top;
    restored = el.scrollTop;
  }

  function track(scrollTop: number): void {
    if (restored !== null && Math.abs(scrollTop - restored) < 1) {
      restored = null;
      return;
    }
    restored = null;
    const n = perLine();
    const start = Math.floor(scrollTop / linePx()) * n;
    list.setAnchor(start, start + n);
  }

  // The anchor family moved to another row while it stayed the anchor: the rows
  // changed, not the scroll. (A scroll that picks another anchor is the user's.)
  watch(
    () => [list.anchor, list.anchorIndex] as const,
    ([family, index], [before]) => {
      if (family !== null && family === before && index >= 0) restore();
    },
    { flush: 'post' },
  );

  return { restore, track };
}
