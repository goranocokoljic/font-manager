// The geometry of the Info section's "Hxg" specimen (E42.8): where the baseline, the cap height
// and the x-height fall on a box the specimen is drawn in, at a given size in px. The values are
// the face's own metrics as fm-core reports them; this only scales font units to pixels.
import type { Metrics } from '../api/types';

/** The specimen box and its guide lines, in px from the top of the box. */
export interface MetricsSpecimen {
  /** The font size the guides are drawn for. */
  size: number;
  /** Height of the box: ascender to descender. */
  height: number;
  /** The baseline: the text's y. */
  baseline: number;
  /** The cap-height line; absent when the font does not state it. */
  capLine: number | null;
  /** The x-height line; absent when the font does not state it. */
  xLine: number | null;
  /** The ascender line (E50.7): the box's top, or below it when a cap or x-height rises higher. */
  ascLine: number;
  /** The descender line (E50.7): the box's bottom, never above the baseline. */
  descLine: number;
}

/** A ruler of a specimen (E50.7): which line, and its distance from the box's top in px. */
export interface SpecimenRuler {
  id: 'ascender' | 'xHeight' | 'baseline' | 'descender';
  top: number;
}

/** The rulers of `s`, top to bottom: ascender, x-height (when stated), baseline, descender. */
export function specimenRulers(s: MetricsSpecimen): SpecimenRuler[] {
  const rulers: SpecimenRuler[] = [{ id: 'ascender', top: s.ascLine }];
  if (s.xLine !== null) rulers.push({ id: 'xHeight', top: s.xLine });
  rulers.push({ id: 'baseline', top: s.baseline }, { id: 'descender', top: s.descLine });
  return rulers;
}

/** The letters of the specimen: a cap, an x-height letter and a descender. Not UI text. */
export const SPECIMEN_TEXT = 'Hxg';

/** The size the criterion sets for the specimen. */
export const SPECIMEN_SIZE = 96;

/** The guides of `metrics` at `size` px; `null` for a face whose metrics could not be read. */
export function metricsSpecimen(
  metrics: Metrics,
  size: number = SPECIMEN_SIZE,
): MetricsSpecimen | null {
  if (metrics.unitsPerEm <= 0) return null;
  const scale = size / metrics.unitsPerEm;
  // A cap or x-height above the ascender (a broken hhea) still fits in the box.
  const top = Math.max(metrics.ascender, metrics.capHeight ?? 0, metrics.xHeight ?? 0);
  const bottom = Math.min(metrics.descender, 0);
  const baseline = top * scale;
  const line = (v: number | null) => (v === null || v <= 0 ? null : baseline - v * scale);
  const height = (top - bottom) * scale;
  return {
    size,
    height,
    baseline,
    capLine: line(metrics.capHeight),
    xLine: line(metrics.xHeight),
    ascLine: baseline - metrics.ascender * scale,
    descLine: height,
  };
}

/** The x-height as a share of the em ("0.48"), or `null` when the font does not state it. */
export function xHeightRatio(metrics: Metrics): number | null {
  if (metrics.unitsPerEm <= 0 || metrics.xHeight === null || metrics.xHeight <= 0) return null;
  return metrics.xHeight / metrics.unitsPerEm;
}
