import { describe, expect, it } from 'vitest';
import { CARD, CARD_TEXT_MAX, cardText, gridColumns, gridRange, lineCount } from './grid';

describe('grid layout', () => {
  it('fits as many cards per line as the width allows, at least one', () => {
    expect(gridColumns(800)).toBe(4);
    expect(gridColumns(799)).toBe(3);
    expect(gridColumns(CARD.minWidth - 1)).toBe(1);
    expect(gridColumns(0)).toBe(1);
    expect(gridColumns(Number.NaN)).toBe(1);
  });

  it('renders whole lines of the window, as card indexes', () => {
    expect(lineCount(10, 4)).toBe(3);
    // 600 px at 148 px lines, overscan 2: 5 + 1 + 4 = 10 lines of 4 cards.
    expect(gridRange(0, 600, 148, 2, 10_000, 4)).toEqual({ start: 0, end: 40 });
    // Scrolled to line 100: the window starts 2 lines above it.
    expect(gridRange(100 * 148, 600, 148, 2, 10_000, 4)).toEqual({ start: 98 * 4, end: 108 * 4 });
    // The last line is short: the range stops at the last card.
    expect(gridRange(1e9, 600, 148, 2, 10, 4)).toEqual({ start: 0, end: 10 });
  });

  it('cuts a long preview text at a character, never inside a surrogate pair', () => {
    expect(cardText('  Ag  ')).toBe('Ag');
    const long = 'The quick brown fox jumps over the lazy dog';
    expect([...cardText(long)].length).toBeLessThanOrEqual(CARD_TEXT_MAX);
    expect(long.startsWith(cardText(long))).toBe(true);
    const emoji = '😀'.repeat(30);
    expect(cardText(emoji)).toBe('😀'.repeat(CARD_TEXT_MAX));
  });
});
