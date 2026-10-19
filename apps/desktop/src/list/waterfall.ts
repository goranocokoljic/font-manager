// Layout of the waterfall view: every row shows its face's preview text at six
// sizes, smallest first. Each size is one line (a size label above the text), so
// a block has a fixed height and the waterfall virtualizes like the list.

/** The six sizes of a waterfall block, in CSS px. */
export const WATERFALL_SIZES = [12, 16, 24, 32, 48, 72] as const;

/** The block's header (family and style name) and padding, in CSS px. */
const HEADER_PX = 28;
const BLOCK_PAD_PX = 8;
/** The size label above each line, and the gap under it. */
const LABEL_PX = 14;
const LINE_PAD_PX = 8;

/** Height of the line that shows the text at `sizePx`. */
export function waterfallLinePx(sizePx: number, lineHeight: number): number {
  return Math.ceil(sizePx * lineHeight) + LABEL_PX + LINE_PAD_PX;
}

/** Height of one block: the header and all six lines. */
export function waterfallRowPx(lineHeight: number): number {
  return (
    HEADER_PX +
    BLOCK_PAD_PX +
    WATERFALL_SIZES.reduce((n, size) => n + waterfallLinePx(size, lineHeight), 0)
  );
}
