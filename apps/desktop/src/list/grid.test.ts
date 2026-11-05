import { describe, expect, it } from 'vitest';
import {
  CARD,
  CARD_TEXT_MAX,
  cardText,
  gridColumns,
  gridRange,
  lineCount,
  specimenBox,
  specimenRenderPx,
  specimenScale,
  specimenText,
  SPECIMEN_TEXT_MAX,
  PARAGRAPH_PX,
  WALL,
  WALL_PRESETS,
  wallSpecimenPx,
  wrappedLines,
  wallCardHeight,
  wallCardWidth,
  wallColumns,
  wallLinePx,
  wallMinWidth,
  wallSize,
} from './grid';

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

describe('card text fit', () => {
  it('ends on a whole word', () => {
    expect(cardText('The quick brown fox', 11)).toBe('The quick');
    expect(cardText('The quick brown fox', 9)).toBe('The quick');
  });
  it('keeps a long single word cut at a character', () => {
    expect(cardText('Supercalifragilistic', 8)).toBe('Supercal');
  });
});

describe('the wall (E41.1)', () => {
  it('follows the width with a 260 px floor: four columns at 1440 less the sidebar', () => {
    // 1440 window − 240 sidebar, with and without a 15 px scrollbar.
    expect(wallColumns(1200, 28)).toBe(4);
    expect(wallColumns(1185, 28)).toBe(4);
    // Exactly n cards of the floor width and n − 1 gaps fit; one px less does not.
    const three = 3 * WALL.minWidth + 2 * WALL.gap + 2 * WALL.inset;
    expect(wallColumns(three, 28)).toBe(3);
    expect(wallColumns(three - 1, 28)).toBe(2);
    expect(wallColumns(100, 28)).toBe(1);
    expect(wallCardWidth(three, 3)).toBe(WALL.minWidth);
  });

  it('asks for wider cards only past the floor, so the top of the slider still grows text', () => {
    expect(wallMinWidth(24)).toBe(260);
    expect(wallMinWidth(61)).toBe(260);
    expect(wallMinWidth(120)).toBe(504);
    expect(wallColumns(1200, 61)).toBe(4);
    expect(wallColumns(1200, 120)).toBe(2);
  });

  it('keeps sizes in the 24–120 range and grows the line with the size', () => {
    expect(wallSize(8)).toBe(24);
    expect(wallSize(300)).toBe(120);
    expect(wallSize(61)).toBe(61);
    expect(wallLinePx(24)).toBeLessThan(wallLinePx(120));
    expect(wallLinePx(61)).toBe(wallCardHeight(61) + WALL.gap);
    // The board's card: about 230 px tall at its slider position.
    expect(wallCardHeight(61)).toBe(230);
    expect(wallCardHeight(300)).toBe(wallCardHeight(120));
  });

  it('shows the longest start that ends on a word', () => {
    const pangram = 'The quick brown fox jumps over the lazy dog';
    // 242 px at 28 px text: 15 characters, "The quick brown" exactly.
    expect(specimenText(pangram, 242, 28)).toBe('The quick brown');
    // One character less: back to the word before.
    expect(specimenText(pangram, 226, 28)).toBe('The quick');
    expect(specimenText(`  ${pangram}  `, 5000, 28)).toBe(pangram);
    expect(specimenText('The\n quick', 5000, 28)).toBe('The quick');
  });

  it('never cuts the first word: it stays whole, and the scale shrinks it', () => {
    expect(specimenText('Hamburg', 242, 120)).toBe('Hamburg');
    expect(specimenText('Hamburg ist', 242, 120)).toBe('Hamburg');
    expect(specimenText('Hamburg', 0, 120)).toBe('Hamburg');
  });

  it('cuts only a run with no space past the guard, at a character', () => {
    const long = '永'.repeat(SPECIMEN_TEXT_MAX + 10);
    expect(specimenText(long, 242, 28)).toBe('永'.repeat(SPECIMEN_TEXT_MAX));
    const emoji = '😀'.repeat(SPECIMEN_TEXT_MAX + 10);
    expect(specimenText(emoji, 242, 28)).toBe('😀'.repeat(SPECIMEN_TEXT_MAX));
  });

  it('renders a long line smaller, so it stays inside the 8,192 px canvas', () => {
    expect(specimenRenderPx('Hamburg', 1)).toBe(WALL.renderPx);
    expect(specimenRenderPx('Hamburg', 3)).toBe(WALL.renderPx);
    expect(specimenRenderPx('', 2)).toBe(WALL.renderPx);
    for (const scale of [1, 1.5, 2, 3]) {
      for (let chars = 1; chars <= SPECIMEN_TEXT_MAX; chars++) {
        const px = specimenRenderPx('永'.repeat(chars), scale);
        expect(px).toBeGreaterThanOrEqual(8);
        expect(chars * 1.5 * px * scale).toBeLessThanOrEqual(8192);
      }
    }
    // Code points, not UTF-16 units: an emoji is one character.
    expect(specimenRenderPx('😀'.repeat(40), 2)).toBe(specimenRenderPx('永'.repeat(40), 2));
  });

  it('scales a specimen down to fit and never up', () => {
    const box = specimenBox(278, 61);
    expect(box).toEqual({ width: 240, height: 177 });
    expect(specimenScale(100, 50, box)).toBe(1);
    expect(specimenScale(480, 50, box)).toBe(0.5);
    expect(specimenScale(100, 354, box)).toBe(0.5);
    expect(specimenScale(0, 0, box)).toBe(1);
  });
});

describe('the wall presets (E41.2)', () => {
  // The wall at 1440 px, with the board's sidebar and no inspector.
  const WIDE = 1208;

  it('scales the slider by the preset: 1, 0.6, 0.42, 0.75, and a fixed Paragraph', () => {
    expect(wallSpecimenPx(100)).toBe(100);
    expect(wallSpecimenPx(100, 'word')).toBe(100);
    expect(wallSpecimenPx(100, 'pangram')).toBeCloseTo(60, 6);
    expect(wallSpecimenPx(100, 'alphabet')).toBeCloseTo(42, 6);
    expect(wallSpecimenPx(100, 'numerals')).toBe(75);
    expect(wallSpecimenPx(24, 'paragraph')).toBe(PARAGRAPH_PX);
    expect(wallSpecimenPx(120, 'paragraph')).toBe(PARAGRAPH_PX);
  });

  it('keeps the slider inside the wall range before it scales', () => {
    expect(wallSpecimenPx(300, 'numerals')).toBe(90);
    expect(wallSpecimenPx(8, 'numerals')).toBe(18);
  });

  it('lays each preset out as its board: 4 / 2 / 2 / 4 / 2 columns at 1440', () => {
    expect(wallColumns(WIDE, 60, 'word')).toBe(4);
    expect(wallColumns(WIDE, 60, 'pangram')).toBe(2);
    expect(wallColumns(WIDE, 60, 'alphabet')).toBe(2);
    expect(wallColumns(WIDE, 60, 'numerals')).toBe(4);
    expect(wallColumns(WIDE, 60, 'paragraph')).toBe(2);
  });

  it('gives each preset the card height of its board at the slider of the boards', () => {
    expect(wallCardHeight(60, 'word')).toBe(227);
    expect(wallCardHeight(60, 'pangram')).toBe(183);
    expect(wallCardHeight(60, 'alphabet')).toBe(204);
    expect(wallCardHeight(60, 'numerals')).toBe(184);
    expect(wallCardHeight(60, 'paragraph')).toBe(184);
  });

  it('a bigger size asks for wider cards, but not in the fixed Paragraph', () => {
    expect(wallColumns(WIDE, 120, 'word')).toBeLessThan(wallColumns(WIDE, 60, 'word'));
    expect(wallColumns(WIDE, 120, 'numerals')).toBeLessThan(wallColumns(WIDE, 60, 'numerals'));
    expect(wallMinWidth(120, 'pangram')).toBeGreaterThan(wallMinWidth(60, 'pangram'));
    expect(wallColumns(WIDE, 120, 'paragraph')).toBe(wallColumns(WIDE, 24, 'paragraph'));
    expect(wallCardHeight(120, 'paragraph')).toBe(wallCardHeight(24, 'paragraph'));
  });

  it('only Alphabet and Paragraph wrap', () => {
    const wrapping = Object.entries(WALL_PRESETS).flatMap(([id, p]) => (p.wrapped ? [id] : []));
    expect(wrapping).toEqual(['alphabet', 'paragraph']);
  });

  it('counts the whole lines of wrapped text that fit; at least one', () => {
    expect(wrappedLines(131, 17, 1.45)).toBe(5);
    expect(wrappedLines(10, 17, 1.45)).toBe(1);
    // Alphabet at slider 120: five lines need 302.4 px, and the box rounds to 302.
    expect(wrappedLines(specimenBox(500, 120, 'alphabet').height, 50.4, 1.2)).toBe(5);
    expect(wrappedLines(301, 50.4, 1.2)).toBe(4);
  });

  it('the specimen box and the line follow the preset', () => {
    expect(specimenBox(500, 60, 'pangram').height).toBe(
      wallCardHeight(60, 'pangram') - WALL.chromePx,
    );
    expect(wallLinePx(60, 'alphabet')).toBe(wallCardHeight(60, 'alphabet') + WALL.gap);
  });
});
