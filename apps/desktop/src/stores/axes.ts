import { defineStore } from 'pinia';
import { reactive } from 'vue';
import type { Axis, AxisValue, FaceId, NamedInstance } from '../api/types';

/** A typed coordinate: a plain decimal number ("400", "-12.5", ".5"); no hex, no "Infinity". */
const DECIMAL = /^[+-]?(\d+\.?\d*|\.\d+)$/;

/** The step of an axis slider and of the arrow keys: finer for a narrow axis. */
export function axisStep(axis: Axis): number {
  const span = (axis.max ?? 0) - (axis.min ?? 0);
  if (span >= 100) return 1;
  if (span >= 10) return 0.1;
  return 0.01;
}

/** The axis default, else its minimum, else 0 (fm-api gives all three for a real axis). */
const defaultOf = (axis: Axis): number => axis.default ?? axis.min ?? 0;

/** `value` inside the axis range. */
function clamp(axis: Axis, value: number): number {
  return Math.min(axis.max ?? Infinity, Math.max(axis.min ?? -Infinity, value));
}

/**
 * Variation coordinates set in the inspector's Axes tab, per face, while the app
 * runs (not saved). Only axes moved away from their default are kept, so "Reset"
 * is forgetting the face. Preview rows of the face read `values` for their
 * `font-variation-settings`; the font is not reloaded.
 */
export const useAxesStore = defineStore('axes', () => {
  /** Per face: tag → coordinate, for axes away from their default. */
  const changed = reactive(new Map<FaceId, Record<string, number>>());

  /** The coordinate of `axis` for `face`: the one set, else the default. */
  function value(face: FaceId, axis: Axis): number {
    return changed.get(face)?.[axis.tag] ?? defaultOf(axis);
  }

  /** Sets `axis` of `face` to `to`, clamped to the axis range. False for a non-finite number. */
  function set(face: FaceId, axis: Axis, to: number): boolean {
    if (!Number.isFinite(to)) return false;
    const v = clamp(axis, to);
    const own = { ...changed.get(face) };
    if (v === defaultOf(axis)) delete own[axis.tag];
    else own[axis.tag] = v;
    if (Object.keys(own).length) changed.set(face, own);
    else changed.delete(face);
    return true;
  }

  /**
   * A coordinate typed into the axis field: clamped to the range. Text that is
   * not a number is rejected (false) and changes nothing.
   */
  function setTyped(face: FaceId, axis: Axis, text: string): boolean {
    const t = text.trim();
    if (!DECIMAL.test(t)) return false;
    return set(face, axis, Number(t));
  }

  /** Every axis of `face` to the coordinates of `instance`; an axis it leaves out to its default. */
  function applyInstance(face: FaceId, axes: readonly Axis[], instance: NamedInstance): void {
    changed.delete(face);
    for (const axis of axes) {
      const given = instance.coordinates.find((c) => c.tag === axis.tag)?.value;
      if (given != null) set(face, axis, given);
    }
  }

  /** Whether every axis of `face` is at the coordinates of `instance`. */
  function isAt(face: FaceId, axes: readonly Axis[], instance: NamedInstance): boolean {
    return axes.every((axis) => {
      const given = instance.coordinates.find((c) => c.tag === axis.tag)?.value;
      return value(face, axis) === clamp(axis, given ?? defaultOf(axis));
    });
  }

  /** Back to the defaults for `face`. */
  function reset(face: FaceId): void {
    changed.delete(face);
  }

  /** Whether any axis of `face` is away from its default. */
  function isChanged(face: FaceId): boolean {
    return changed.has(face);
  }

  /** The coordinates set for `face` (axes away from their default), or undefined. */
  function values(face: FaceId): AxisValue[] | undefined {
    const own = changed.get(face);
    if (!own) return undefined;
    return Object.keys(own).map((tag) => ({ tag, value: own[tag]! }));
  }

  /** A coordinate for every axis of `face`, in the font's axis order (for pinning). */
  function coordinates(face: FaceId, axes: readonly Axis[]): AxisValue[] {
    return axes.map((axis) => ({ tag: axis.tag, value: value(face, axis) }));
  }

  return { value, set, setTyped, applyInstance, isAt, reset, isChanged, values, coordinates };
});
