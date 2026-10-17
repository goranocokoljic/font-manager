// `font-variation-settings` for a preview row, and the axes it is built from.
import { useClient } from '../api/client';
import type { Axis, AxisValue, FaceId } from '../api/types';

/**
 * The CSS `font-variation-settings` value for a face: each axis at the given
 * coordinate, else at its default. `normal` for a static face.
 */
export function variationSettings(axes: Axis[], values: AxisValue[] = []): string {
  const parts = axes.flatMap((axis) => {
    const given = values.find((v) => v.tag === axis.tag)?.value;
    const value = given != null && Number.isFinite(given) ? given : axis.default;
    // Tags are four printable ASCII characters; anything else is not CSS-safe.
    if (value == null || !Number.isFinite(value) || !/^[\x20-\x7e]{4}$/.test(axis.tag)) return [];
    return [`"${axis.tag.replace(/["\\]/g, '\\$&')}" ${value}`];
  });
  return parts.length ? parts.join(', ') : 'normal';
}

/** Axes of faces already asked for; a few numbers each, bounded. */
const axesCache = new Map<FaceId, Promise<Axis[]>>();
const AXES_CACHE_LIMIT = 2000;

/** The variation axes of a face (from its detail), fetched once. */
export function faceAxes(face: FaceId): Promise<Axis[]> {
  let axes = axesCache.get(face);
  if (!axes) {
    axes = useClient()
      .getFace(face)
      .then((d) => d.axes);
    axes.catch(() => axesCache.delete(face));
    axesCache.set(face, axes);
    if (axesCache.size > AXES_CACHE_LIMIT) axesCache.delete(axesCache.keys().next().value!);
  }
  return axes;
}

/** Forgets every fetched axis list (tests, library changes). */
export function clearFaceAxes(): void {
  axesCache.clear();
}
