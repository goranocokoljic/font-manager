// The geometry of the glyph compare overlay: two fonts drawn at one em size on
// one baseline. Every vertical line comes from a face's `FaceDetail.metrics`
// (fm-core reads them from the font); this file only scales font units to pixels.
import type { Metrics } from '../api/types';

/** The em sizes the overlay offers, in CSS pixels. */
export const COMPARE_SIZE = { min: 48, max: 480, step: 8, initial: 200 } as const;

/** Space around the drawing, in pixels. */
const PAD = 16;

/** A horizontal guide line of one font. */
export interface CompareLine {
  /** Which font (0 or 1) the line belongs to. */
  font: number;
  kind: 'xHeight' | 'capHeight';
  /** The metric in font units, as the font declares it. */
  units: number;
  /** Distance from the top of the drawing, in pixels. */
  y: number;
}

/** Where the overlay draws: its size, the shared baseline and each font's lines. */
export interface CompareLayout {
  width: number;
  height: number;
  /** The shared baseline, from the top, in pixels. */
  baseline: number;
  /** The x both glyphs start at (their origin). */
  origin: number;
  lines: CompareLine[];
}

/** `size` clamped to the offered range and rounded to a whole pixel. */
export function clampSize(size: number): number {
  if (!Number.isFinite(size)) return COMPARE_SIZE.initial;
  return Math.round(Math.min(COMPARE_SIZE.max, Math.max(COMPARE_SIZE.min, size)));
}

/**
 * The layout of the overlay at em size `size` (pixels) for the fonts with
 * `metrics` (one or two). The drawing is tall enough for the highest ascender
 * or cap height and the lowest descender of both; a metric the font does not
 * declare has no line.
 */
export function compareLayout(size: number, metrics: Metrics[]): CompareLayout {
  const px = (units: number, m: Metrics) => (units / (m.unitsPerEm || 1000)) * size;
  const above = Math.max(
    size,
    ...metrics.map((m) => px(Math.max(m.ascender, m.capHeight ?? 0), m)),
  );
  const below = Math.max(size / 4, ...metrics.map((m) => px(-m.descender, m)));
  const baseline = PAD + above;
  const lines: CompareLine[] = [];
  metrics.forEach((m, font) => {
    for (const kind of ['xHeight', 'capHeight'] as const) {
      const units = m[kind];
      if (units !== null) lines.push({ font, kind, units, y: baseline - px(units, m) });
    }
  });
  return {
    width: Math.round(size * 1.5 + 2 * PAD),
    height: Math.round(baseline + below + PAD),
    baseline,
    origin: PAD + size * 0.25,
    lines,
  };
}
