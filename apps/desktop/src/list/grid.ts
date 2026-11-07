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

/** The preset chips (E41.2): each is a different layout of the wall. */
export type WallPreset = 'word' | 'pangram' | 'alphabet' | 'numerals' | 'paragraph';

/** The body size of the Paragraph preset, in CSS px: the slider does not change it. */
export const PARAGRAPH_PX = 17;

/**
 * Each preset's layout (boards `1-wall-<preset>.png`). `scale` is the slider's factor;
 * Paragraph has none, its size is fixed. Widths and heights are per px of specimen text.
 * `lineHeight` is set for a preset of wrapped text; null takes the view's line height.
 */
export const WALL_PRESETS: Record<
  WallPreset,
  {
    scale: number | null;
    minWidth: number;
    widthPerPx: number;
    heightPerPx: number;
    wrapped: boolean;
    lineHeight: number | null;
  }
> = {
  word: {
    scale: 1,
    minWidth: WALL.minWidth,
    widthPerPx: WALL.widthPerPx,
    heightPerPx: WALL.heightPerPx,
    wrapped: false,
    lineHeight: null,
  },
  pangram: {
    scale: 0.6,
    minWidth: 520,
    widthPerPx: 7.5,
    heightPerPx: 3.6,
    wrapped: false,
    lineHeight: null,
  },
  alphabet: {
    scale: 0.42,
    minWidth: 520,
    widthPerPx: 10,
    heightPerPx: 6,
    wrapped: true,
    lineHeight: null,
  },
  numerals: {
    scale: 0.75,
    minWidth: WALL.minWidth,
    widthPerPx: WALL.widthPerPx,
    heightPerPx: WALL.heightPerPx,
    wrapped: false,
    lineHeight: null,
  },
  paragraph: {
    scale: null,
    minWidth: 520,
    widthPerPx: 0,
    heightPerPx: 7.7,
    wrapped: true,
    lineHeight: 1.45,
  },
};

/**
 * Paragraph mode (E43.1, board `3-wall-text.png`): your own text longer than this many
 * characters turns the wall into reading rows, as the Paragraph preset does.
 */
export const PARAGRAPH_TEXT_MIN = 80;

/** The body line height of the reading rows, by default. */
export const PARAGRAPH_LINE_HEIGHT = 1.45;

/**
 * The layout the wall shows: the preset's, or the reading rows of Paragraph for your own
 * text past `PARAGRAPH_TEXT_MIN` characters. Only your own text: a stock text (a long
 * pangram, the alphabet) keeps its preset's board.
 */
export function wallLayout(preset: WallPreset, sample: string, customText: string): WallPreset {
  if (preset === 'paragraph') return 'paragraph';
  return sample === 'custom' && [...customText.trim()].length > PARAGRAPH_TEXT_MIN
    ? 'paragraph'
    : preset;
}

/** The reading rows' measures in CSS px: the meta line on top, then the text. */
export const ROWS = {
  /** Two columns at most, one where two would be narrower than `minWidth`. */
  columnsMax: 2,
  /** The narrowest row: about 45 characters of 17 px text, a comfortable measure. */
  minWidth: 440,
  /** Padding on every side, inside the hairline. */
  paddingPx: 18,
  /** The family name and its meta: a 12 px line. */
  metaPx: 12,
  metaLinePx: 16,
  /** Space between the meta line and the text. */
  metaGapPx: 8,
  /** The most lines of text a row shows. */
  linesMax: 6,
  /** The advance to plan for per character, in em: a little over a text face's, for a mono. */
  emPerChar: 0.6,
} as const;

/** Reading rows per line in a viewport `width` px wide; at least one, at most two. */
export const rowsColumns = (width: number) =>
  Math.min(
    ROWS.columnsMax,
    gridColumns(width - 2 * WALL.inset + WALL.gap, ROWS.minWidth + WALL.gap),
  );

/**
 * The lines every row gives its text: enough for `text` at `bodyPx` in `width` px (a guess
 * at `ROWS.emPerChar`), from one to `ROWS.linesMax`. Rows of a line have one height, so the
 * list stays virtualized; a row whose face sets the text shorter keeps the room under it.
 */
export function rowsLines(text: string, width: number, bodyPx: number): number {
  const chars = [...text.trim()].length;
  if (!chars || width <= 0 || bodyPx <= 0) return 1;
  const lines = Math.ceil((chars * ROWS.emPerChar * bodyPx) / width);
  return Math.min(ROWS.linesMax, Math.max(1, lines));
}

/** A reading row's height: hairlines, padding, the meta line and `lines` of text. */
export const rowsCardHeight = (lines: number, bodyPx: number, lineHeight: number) =>
  2 * (ROWS.paddingPx + 1) +
  ROWS.metaLinePx +
  ROWS.metaGapPx +
  Math.ceil(lines * bodyPx * lineHeight);

/** The specimen's text size for the slider's `sizePx` in `preset`. */
export function wallSpecimenPx(sizePx: number, preset: WallPreset = 'word'): number {
  const { scale } = WALL_PRESETS[preset];
  return scale === null ? PARAGRAPH_PX : wallSize(sizePx) * scale;
}

/** The narrowest card at `sizePx` in `preset`. */
export function wallMinWidth(sizePx: number, preset: WallPreset = 'word'): number {
  const p = WALL_PRESETS[preset];
  return Math.max(p.minWidth, Math.round(wallSpecimenPx(sizePx, preset) * p.widthPerPx));
}

/** Cards per line in a viewport `width` px wide at `sizePx`; at least one. */
export function wallColumns(width: number, sizePx: number, preset: WallPreset = 'word'): number {
  return gridColumns(width - 2 * WALL.inset + WALL.gap, wallMinWidth(sizePx, preset) + WALL.gap);
}

/** A card's width when `columns` cards share a viewport `width` px wide. */
export const wallCardWidth = (width: number, columns: number) =>
  Math.max(0, (width - 2 * WALL.inset - (columns - 1) * WALL.gap) / columns);

/** A card's height at `sizePx`. */
export const wallCardHeight = (sizePx: number, preset: WallPreset = 'word') =>
  Math.round(wallSpecimenPx(sizePx, preset) * WALL_PRESETS[preset].heightPerPx) + WALL.chromePx;

/** A line's height at `sizePx`: its card and the gap under it. */
export const wallLinePx = (sizePx: number, preset: WallPreset = 'word') =>
  wallCardHeight(sizePx, preset) + WALL.gap;

/** The room a card's specimen has, in CSS px. */
export const specimenBox = (cardWidth: number, sizePx: number, preset: WallPreset = 'word') => ({
  width: Math.max(0, cardWidth - 2 * WALL.paddingPx),
  height: Math.max(0, wallCardHeight(sizePx, preset) - WALL.chromePx),
});

/**
 * The lines of wrapped text that fit `height` px; at least one, so no line is cut.
 * `height` is rounded to a whole px: half a px of slack keeps a line that fits exactly.
 */
export const wrappedLines = (height: number, specimenPx: number, lineHeight: number) =>
  Math.max(1, Math.floor((height + 0.5) / (specimenPx * lineHeight)));

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

/** The widest image `render_preview` makes (`MAX_SIDE_PX` in fm-core); it clips the rest. */
const RENDER_MAX_SIDE_PX = 8192;
/** The advance to plan for per character, in em: 1 for Chinese, more in a wide face. */
const RENDER_EM_PER_CHAR = 1.5;

/**
 * The CSS px size to render `text` at on a display of `scale`: `WALL.renderPx`, or less
 * when the line would pass the renderer's canvas and lose its end. The lost size costs no
 * sharpness: text that long shrinks to its card anyway.
 */
export function specimenRenderPx(text: string, scale: number): number {
  const chars = Math.max([...text].length, 1);
  return Math.min(
    WALL.renderPx,
    Math.floor(RENDER_MAX_SIDE_PX / (chars * RENDER_EM_PER_CHAR * scale)),
  );
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
