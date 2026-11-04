// Layout math of the grid view: the font list's rows as cards, `columns` cards
// per line. Lines have a fixed height, so the grid virtualizes whole lines with
// the list's window math (`virtual.ts`) and keys cards by slot the same way.
import { windowRange, type RowRange } from './virtual';

/** Card size in CSS px: the narrowest a card gets, and a line's height. */
export const CARD = { minWidth: 200, linePx: 148 } as const;

/** Cards per line in a viewport `width` px wide; at least one. */
export function gridColumns(width: number, minWidth: number = CARD.minWidth): number {
  if (!Number.isFinite(width) || width <= 0) return 1;
  return Math.max(1, Math.floor(width / minWidth));
}

/** Lines of `count` cards at `columns` per line. */
export const lineCount = (count: number, columns: number) => Math.ceil(count / columns);

/**
 * The cards to render: the window of lines (`windowRange` over lines) as card
 * indexes. Always whole lines, so the pool of slots stays `pool lines × columns`.
 */
export function gridRange(
  scrollTop: number,
  height: number,
  linePx: number,
  overscan: number,
  count: number,
  columns: number,
): RowRange {
  const lines = windowRange(scrollTop, height, linePx, overscan, lineCount(count, columns));
  return { start: lines.start * columns, end: Math.min(count, lines.end * columns) };
}

/** Longest thumbnail text, in characters. */
export const CARD_TEXT_MAX = 24;

/**
 * The text a card's thumbnail renders: the start of the row's preview text, cut
 * at a character (not inside a surrogate pair), so a long sample stays a small image.
 * A cut after a space ends on a whole word ("The quick", never "The quick b").
 */
export function cardText(text: string, max: number = CARD_TEXT_MAX): string {
  const chars = [...text.trim()];
  if (chars.length <= max) return chars.join('');
  const cut = chars.slice(0, max);
  const space = chars[max] === ' ' ? max : cut.lastIndexOf(' ');
  return (space >= 3 ? cut.slice(0, space) : cut).join('').trimEnd();
}

// The wall (E41.1): the library grid's cards. The catalog browser keeps `CARD`.

/** The size slider's range on the wall, in CSS px of text. */
export const WALL_SIZE = { min: 24, max: 120, step: 1 } as const;

/** The wall's measures in CSS px (board `1-wall-word.png`). */
export const WALL = {
  /** The narrowest a card gets, whatever the size. */
  minWidth: 260,
  gap: 10,
  /** Space between the wall and the viewport's inline edges. */
  inset: 26,
  /** A card's width per px of text: past the floor, a larger size asks for wider cards. */
  widthPerPx: 4.2,
  /** A card's height per px of text, plus its footer and padding. */
  heightPerPx: 2.9,
  chromePx: 53,
  /** A specimen's inline padding plus the card's hairline, each side. */
  paddingPx: 19,
  /** Thumbnails render at the largest size; the slider only scales them, so it never renders. */
  renderPx: WALL_SIZE.max,
  /** Shortest time between two reflows during a drag, in ms. */
  reflowMs: 100,
} as const;

/** `sizePx` inside the wall's range: other views keep their own larger or smaller values. */
export const wallSize = (sizePx: number) =>
  Math.min(WALL_SIZE.max, Math.max(WALL_SIZE.min, sizePx));

/** The narrowest card at `sizePx`. */
export const wallMinWidth = (sizePx: number) =>
  Math.max(WALL.minWidth, Math.round(wallSize(sizePx) * WALL.widthPerPx));

/** Cards per line in a viewport `width` px wide at `sizePx`; at least one. */
export function wallColumns(width: number, sizePx: number): number {
  return gridColumns(width - 2 * WALL.inset + WALL.gap, wallMinWidth(sizePx) + WALL.gap);
}

/** A card's width when `columns` cards share a viewport `width` px wide. */
export const wallCardWidth = (width: number, columns: number) =>
  Math.max(0, (width - 2 * WALL.inset - (columns - 1) * WALL.gap) / columns);

/** A card's height at `sizePx`. */
export const wallCardHeight = (sizePx: number) =>
  Math.round(wallSize(sizePx) * WALL.heightPerPx) + WALL.chromePx;

/** A line's height at `sizePx`: its card and the gap under it. */
export const wallLinePx = (sizePx: number) => wallCardHeight(sizePx) + WALL.gap;

/** The room a card's specimen has, in CSS px. */
export const specimenBox = (cardWidth: number, sizePx: number) => ({
  width: Math.max(0, cardWidth - 2 * WALL.paddingPx),
  height: Math.max(0, wallCardHeight(sizePx) - WALL.chromePx),
});

/** Longest specimen, in characters: a guard for long text without spaces (Chinese, say). */
export const SPECIMEN_TEXT_MAX = 64;

/**
 * The text a specimen shows: the longest start of `text` that ends on a word and is
 * expected to fit `width` px at `sizePx` (about 0.55 em a character). The first word
 * always stays whole, however long: `specimenScale` shrinks what does not fit, so a
 * card never shows a cut word. Only a run with no space inside `SPECIMEN_TEXT_MAX`
 * characters is cut, at a character.
 */
export function specimenText(text: string, width: number, sizePx: number): string {
  const chars = [...text.trim().replace(/\s+/g, ' ')];
  const fit = Math.floor(width / (sizePx * 0.55));
  const max = Math.min(SPECIMEN_TEXT_MAX, Number.isFinite(fit) ? Math.max(fit, 1) : 1);
  if (chars.length <= max) return chars.join('');
  const spaces = chars.flatMap((c, i) => (c === ' ' ? [i] : []));
  const end = spaces.filter((i) => i <= max).pop() ?? spaces[0] ?? chars.length;
  return chars.slice(0, Math.min(end, SPECIMEN_TEXT_MAX)).join('');
}

/** The scale that fits a `width` × `height` px specimen into `box`; never above 1. */
export function specimenScale(
  width: number,
  height: number,
  box: { width: number; height: number },
): number {
  if (width <= 0 || height <= 0) return 1;
  return Math.min(1, box.width / width, box.height / height);
}
