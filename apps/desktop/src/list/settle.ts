// Tells a fast scroll from a resting one, so a virtualized view can skip the work a
// row starts when it is rendered (a face load, sample text, a thumbnail) for rows
// that leave the viewport a few frames later. The view is *moving* once it moved
// more than one row per frame for two frames in a row; it is *settled* again once
// its position stayed within one row for `SETTLE_MS`. A slow wheel (a notch every
// 50 ms) never moves a row per frame, so it stays settled; a programmatic scroll to a
// row (`jump`) is settled at once.
import { onBeforeUnmount, ref, type Ref } from 'vue';

/** One frame at 60 Hz, in ms. */
export const FRAME_MS = 1000 / 60;
/** How long the position must rest within one row before the view counts as settled. */
export const SETTLE_MS = 100;
/** Fast frames in a row that make the view moving. */
const FAST_FRAMES = 2;

/** What the helper knows about the scroll so far. */
export interface ScrollMotion {
  moving: boolean;
  /** Fast frames in a row up to the last sample. */
  fast: number;
  /** The last scroll position seen, and when. */
  top: number;
  time: number;
  /** Where the position last came to rest (within one row), and since when. */
  restTop: number;
  restSince: number;
}

/** A view that has not scrolled yet: settled at `top`. */
export function restingAt(top: number, time: number): ScrollMotion {
  return { moving: false, fast: 0, top, time, restTop: top, restSince: time };
}

/**
 * The view scrolled to `top` at `time` (ms). `rowPx` is the height of one row (a
 * line of the grid). A frame is fast when it moved more than one row in one frame's
 * time; scroll events closer than a frame count as one frame.
 */
export function observeScroll(
  m: ScrollMotion,
  top: number,
  time: number,
  rowPx: number,
): ScrollMotion {
  const row = Math.max(1, rowPx);
  const dt = Math.max(FRAME_MS, time - m.time);
  const perFrame = (Math.abs(top - m.top) * FRAME_MS) / dt;
  const fast = perFrame > row ? m.fast + 1 : 0;
  const away = Math.abs(top - m.restTop) > row;
  const restTop = away ? top : m.restTop;
  const restSince = away ? time : m.restSince;
  const next = { moving: m.moving || fast >= FAST_FRAMES, fast, top, time, restTop, restSince };
  return settleCheck(next, time);
}

/** `m` at `time` with no scroll since its last sample: settled once it rested `SETTLE_MS`. */
export function settleCheck(m: ScrollMotion, time: number): ScrollMotion {
  if (!m.moving || time - m.restSince < SETTLE_MS) return m;
  return { ...m, moving: false, fast: 0 };
}

/** When a moving view settles if it does not scroll again; `null` when it is settled. */
export function settlesAt(m: ScrollMotion): number | null {
  return m.moving ? m.restSince + SETTLE_MS : null;
}

/** The view was scrolled to `top` by the app (a row revealed, the anchor restored). */
export function jump(top: number, time: number): ScrollMotion {
  return restingAt(top, time);
}

/** A view's settled flag, fed by its scroll events. */
export interface ScrollSettle {
  /** False while the view moves fast; rows start no loads then. */
  settled: Readonly<Ref<boolean>>;
  /** The user scrolled to `top`. */
  scrolled(top: number): void;
  /** The app scrolled to `top`: settled at once. */
  jumped(top: number): void;
}

/** `rowPx` is the height of one row (a line of the grid). */
export function useScrollSettle(rowPx: () => number): ScrollSettle {
  const now = () => performance.now();
  const settled = ref(true);
  let motion = restingAt(0, now());
  let timer: ReturnType<typeof setTimeout> | undefined;

  function apply(m: ScrollMotion): void {
    motion = m;
    settled.value = !m.moving;
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    const at = settlesAt(m);
    if (at !== null) timer = setTimeout(() => apply(settleCheck(motion, now())), at - now());
  }

  onBeforeUnmount(() => {
    if (timer !== undefined) clearTimeout(timer);
  });

  return {
    settled,
    scrolled: (top) => apply(observeScroll(motion, top, now(), rowPx())),
    jumped: (top) => apply(jump(top, now())),
  };
}
