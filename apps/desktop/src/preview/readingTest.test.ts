import { describe, expect, it } from 'vitest';
import type { Axis } from '../api/types';
import {
  LEADING,
  MEASURE,
  READING_SIZES,
  readingPassage,
  readingVariation,
  STOCK_PASSAGE,
} from './readingTest';
import { LOREM, PARAGRAPH } from './sampleText';

const axis = (tag: string, min: number, max: number, def: number): Axis => ({
  tag,
  name: tag,
  min,
  max,
  default: def,
});

describe('the passage', () => {
  it('takes the preview text from 81 characters on, the stock passage up to 80', () => {
    expect(readingPassage('x'.repeat(80))).toBe(STOCK_PASSAGE);
    expect(readingPassage('x'.repeat(81))).toBe('x'.repeat(81));
  });

  it('counts characters, not UTF-16 units, and not the spaces around the text', () => {
    // 41 astral characters are 82 units but only 41 characters.
    expect(readingPassage('𝐀'.repeat(41))).toBe(STOCK_PASSAGE);
    expect(readingPassage(`  ${'x'.repeat(80)}  `)).toBe(STOCK_PASSAGE);
  });

  it('has no text of its own for an empty preview', () => {
    expect(readingPassage('')).toBe(STOCK_PASSAGE);
  });

  it('reads the longer samples as they are', () => {
    expect(readingPassage(PARAGRAPH)).toBe(PARAGRAPH);
    expect(readingPassage(LOREM)).toBe(LOREM);
    expect(STOCK_PASSAGE).not.toBe(PARAGRAPH);
  });
});

describe('the variation at a size', () => {
  const opsz = axis('opsz', 9, 144, 14);
  const wght = axis('wght', 100, 900, 400);

  it('sets opsz to each column size', () => {
    expect(READING_SIZES.map((s) => readingVariation([opsz], [], s.px))).toEqual([
      '"opsz" 12',
      '"opsz" 17',
      '"opsz" 24',
    ]);
  });

  it('holds opsz at the end of an axis that does not reach the size', () => {
    const short = axis('opsz', 14, 20, 16);
    expect(readingVariation([short], [], 12)).toBe('"opsz" 14');
    expect(readingVariation([short], [], 17)).toBe('"opsz" 17');
    expect(readingVariation([short], [], 24)).toBe('"opsz" 20');
  });

  it('wins over an opsz moved on the hero, and keeps the other axes the hero moved', () => {
    const moved = [
      { tag: 'opsz', value: 144 },
      { tag: 'wght', value: 650 },
    ];
    expect(readingVariation([opsz, wght], moved, 17)).toBe('"opsz" 17, "wght" 650');
  });

  it('adds no opsz to a family without the axis', () => {
    expect(readingVariation([wght], [{ tag: 'wght', value: 650 }], 17)).toBe('"wght" 650');
    expect(readingVariation([], [], 17)).toBe('normal');
  });
});

describe('the sliders', () => {
  it('have the ranges of the story, with the default on a step', () => {
    expect([MEASURE.min, MEASURE.max, LEADING.min, LEADING.max]).toEqual([40, 90, 1.1, 1.8]);
    expect(LEADING.default).toBe(1.45);
    const steps = (LEADING.default - LEADING.min) / LEADING.step;
    expect(steps).toBeCloseTo(Math.round(steps));
  });
});
