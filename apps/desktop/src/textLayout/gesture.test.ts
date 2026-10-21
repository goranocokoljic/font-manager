import { describe, expect, it } from 'vitest';
import { MOCK_LAYOUT_BOUNDS as B } from '../api/mock';
import { dragSize, fit, keySize, pinchSize, sizeLabel, wheelScale } from './gesture';

const start = { sizePt: 36, lineHeight: 1.2 };

describe('block gestures (E12.2)', () => {
  it('a horizontal drag changes only the size, a vertical drag only the line height', () => {
    expect(dragSize(start, 20, 0, B)).toEqual({ sizePt: 46, lineHeight: 1.2 });
    expect(dragSize(start, -20, 0, B)).toEqual({ sizePt: 26, lineHeight: 1.2 });
    // Up opens the line height, down closes it.
    expect(dragSize(start, 0, -30, B)).toEqual({ sizePt: 36, lineHeight: 1.5 });
    expect(dragSize(start, 0, 30, B)).toEqual({ sizePt: 36, lineHeight: 0.9 });
  });

  it('drags are clamped to the fm-core bounds (8–600 pt, 0.1–3.0)', () => {
    expect(dragSize(start, 100_000, -100_000, B)).toEqual({ sizePt: 600, lineHeight: 3 });
    expect(dragSize(start, -100_000, 100_000, B)).toEqual({ sizePt: 8, lineHeight: 0.1 });
  });

  it('a pinch scales the size and keeps it in range', () => {
    expect(pinchSize(start, 2, B)).toEqual({ sizePt: 72, lineHeight: 1.2 });
    expect(pinchSize(start, 0.5, B)).toEqual({ sizePt: 18, lineHeight: 1.2 });
    expect(pinchSize(start, 100, B).sizePt).toBe(600);
    expect(pinchSize(start, 0.01, B).sizePt).toBe(8);
    // Ctrl+wheel: a pinch out (negative deltaY) enlarges, in shrinks, zero keeps.
    expect(wheelScale(-10)).toBeGreaterThan(1);
    expect(wheelScale(10)).toBeLessThan(1);
    expect(wheelScale(0)).toBe(1);
  });

  it('arrow keys step the size (Left/Right) and line height (Up/Down); Shift steps ten', () => {
    expect(keySize(start, 'ArrowRight', false, B)).toEqual({ sizePt: 37, lineHeight: 1.2 });
    expect(keySize(start, 'ArrowLeft', true, B)).toEqual({ sizePt: 26, lineHeight: 1.2 });
    expect(keySize(start, 'ArrowUp', false, B)).toEqual({ sizePt: 36, lineHeight: 1.25 });
    expect(keySize(start, 'ArrowDown', true, B)).toEqual({ sizePt: 36, lineHeight: 0.7 });
    expect(keySize({ sizePt: 600, lineHeight: 3 }, 'ArrowRight', true, B)?.sizePt).toBe(600);
    expect(keySize({ sizePt: 8, lineHeight: 0.1 }, 'ArrowDown', true, B)?.lineHeight).toBe(0.1);
    expect(keySize(start, 'a', false, B)).toBeNull();
  });

  it('a value that is not a number takes the default; float noise is rounded away', () => {
    expect(fit(Number.NaN, B.sizePt)).toBe(12);
    expect(dragSize({ sizePt: Number.NaN, lineHeight: 1.2 }, 0, 0, B).sizePt).toBe(12);
    expect(dragSize({ sizePt: 10, lineHeight: 1.2 }, 1, 7, B)).toEqual({
      sizePt: 10.5,
      lineHeight: 1.13,
    });
    expect(sizeLabel({ sizePt: 36, lineHeight: 1.2 })).toBe('36 pt · 1.20');
  });
});
