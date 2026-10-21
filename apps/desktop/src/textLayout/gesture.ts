// Direct manipulation of a layout block (E12.2): turns a drag, a pinch or an
// arrow key into a new size and line height. The ranges and steps come from
// fm-core (`layoutBounds`), so a value is kept inside the core's bounds every
// frame without a round trip; the committed block still goes through
// `clampLayout`.
import type { BlockRange, LayoutBounds } from '../api/types';
import { formatNumber, t } from '../i18n';

/** The two values a block gesture changes. */
export interface BlockSize {
  sizePt: number;
  lineHeight: number;
}

/** Drag distance in px before a press becomes a drag (a click or double-click is not one). */
export const DRAG_THRESHOLD_PX = 3;

/** How strongly one unit of Ctrl+wheel `deltaY` (a trackpad pinch in Chromium) scales the size. */
const WHEEL_PINCH_PER_DELTA = 0.01;

/** `value` inside the range; a value that is not a finite number takes the default. */
export function fit(value: number, range: BlockRange): number {
  if (!Number.isFinite(value)) return range.default ?? range.min ?? 0;
  return Math.min(range.max ?? Infinity, Math.max(range.min ?? -Infinity, value));
}

// An `f32` crosses the API as `number | null` (null for NaN); a missing step moves nothing.
const step = (r: BlockRange) => r.step ?? 0;
const perPx = (r: BlockRange) => r.perPx ?? 0;

// Sizes to 0.1 pt and line heights to 0.01, so the badge and the stored block
// never carry float noise like 36.00000001.
const roundSize = (v: number) => Math.round(v * 10) / 10;
const roundLineHeight = (v: number) => Math.round(v * 100) / 100;

function sized(sizePt: number, lineHeight: number, bounds: LayoutBounds): BlockSize {
  return {
    sizePt: roundSize(fit(sizePt, bounds.sizePt)),
    lineHeight: roundLineHeight(fit(lineHeight, bounds.lineHeight)),
  };
}

/**
 * The values after a drag of (`dx`, `dy`) px from `start`: right makes the
 * text larger, up opens the line height (like the Up arrow key).
 */
export function dragSize(
  start: BlockSize,
  dx: number,
  dy: number,
  bounds: LayoutBounds,
): BlockSize {
  return sized(
    start.sizePt + dx * perPx(bounds.sizePt),
    start.lineHeight - dy * perPx(bounds.lineHeight),
    bounds,
  );
}

/** The values after a pinch that scales the size by `scale` (WebKit `GestureEvent.scale`). */
export function pinchSize(start: BlockSize, scale: number, bounds: LayoutBounds): BlockSize {
  return sized(start.sizePt * scale, start.lineHeight, bounds);
}

/** The scale of one Ctrl+wheel event: a pinch out (negative `deltaY`) enlarges. */
export function wheelScale(deltaY: number): number {
  return Math.exp(-deltaY * WHEEL_PINCH_PER_DELTA);
}

/**
 * The values after an arrow key: Left/Right change the size, Up/Down the line
 * height, by one step (ten with Shift). Null for any other key.
 */
export function keySize(
  current: BlockSize,
  key: string,
  shift: boolean,
  bounds: LayoutBounds,
): BlockSize | null {
  const n = shift ? 10 : 1;
  const { sizePt, lineHeight } = current;
  switch (key) {
    case 'ArrowRight':
      return sized(sizePt + n * step(bounds.sizePt), lineHeight, bounds);
    case 'ArrowLeft':
      return sized(sizePt - n * step(bounds.sizePt), lineHeight, bounds);
    case 'ArrowUp':
      return sized(sizePt, lineHeight + n * step(bounds.lineHeight), bounds);
    case 'ArrowDown':
      return sized(sizePt, lineHeight - n * step(bounds.lineHeight), bounds);
    default:
      return null;
  }
}

/** The badge text of a block: "36 pt · 1.15". */
export function sizeLabel({ sizePt, lineHeight }: BlockSize): string {
  return t('blocks.badge', {
    size: roundSize(sizePt),
    lineHeight: formatNumber(roundLineHeight(lineHeight), {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }),
  });
}
