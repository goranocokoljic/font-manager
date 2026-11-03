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

/** Characters of sample text that fit a card `cardWidth` px wide at `sizePx` text (about 0.55 em each). */
export function cardChars(cardWidth: number, sizePx: number): number {
  const fit = Math.floor((cardWidth - 28) / (sizePx * 0.55));
  return Math.min(CARD_TEXT_MAX, Math.max(4, Number.isFinite(fit) ? fit : CARD_TEXT_MAX));
}

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
