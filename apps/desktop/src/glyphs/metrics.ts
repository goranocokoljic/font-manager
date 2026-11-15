// The metrics row of the glyph popover while a comparison is on (E50.9): advance, left and
// right bearing (GlyphDetail, from fm-core) and the x-height (FaceDetail.metrics) of this
// face and the compared one. fm-core reads every value in font units; this file only scales
// them to 1000 units per em, so faces with different em sizes line up.
import type { GlyphDetail, Metrics } from '../api/types';

export type GlyphMetricId = 'advance' | 'leftBearing' | 'rightBearing' | 'xHeight';

/** One metric of the row: this face's value and the compared face's, per 1000 em. */
export interface GlyphMetric {
  id: GlyphMetricId;
  /** This face's value, or null when the font does not give it. */
  mine: number | null;
  /** The compared face's value, or null when it does not give it (or lacks the glyph). */
  theirs: number | null;
}

/** One side of a comparison: the glyph's detail (null while loading or missing) and the face's metrics. */
export interface MetricSide {
  glyph: GlyphDetail | null;
  metrics: Metrics | null | undefined;
}

/** `units` of a font with `unitsPerEm` scaled to 1000 per em and rounded; null stays null. */
export function perThousandEm(units: number | null | undefined, unitsPerEm: number): number | null {
  if (units === null || units === undefined || !Number.isFinite(units)) return null;
  const em = unitsPerEm > 0 ? unitsPerEm : 1000;
  return Math.round((units * 1000) / em);
}

function values(side: MetricSide): Record<GlyphMetricId, number | null> {
  const em = side.metrics?.unitsPerEm ?? 1000;
  const g = side.glyph;
  return {
    advance: perThousandEm(g?.advanceWidth, em),
    leftBearing: perThousandEm(g?.leftSideBearing, em),
    rightBearing: perThousandEm(g?.rightSideBearing, em),
    xHeight: perThousandEm(side.metrics?.xHeight, em),
  };
}

const ORDER: GlyphMetricId[] = ['advance', 'leftBearing', 'rightBearing', 'xHeight'];

/** The four metrics of the row, in the order the popover shows them. */
export function glyphMetricRow(mine: MetricSide, theirs: MetricSide): GlyphMetric[] {
  const a = values(mine);
  const b = values(theirs);
  return ORDER.map((id) => ({ id, mine: a[id], theirs: b[id] }));
}
