// The ramps of the family page's Styles section (E42.3): the stops a row list shows along one
// variation axis. These are places on the axis fm-api already gives, not facts about the font,
// so they sit with the slider's marks (`weightMarks`).
import type { Axis, NamedInstance } from '../api/types';
import { axisStep, instanceNear, NEAR_INSTANCE, weightMarks } from '../stores/axes';

const WEIGHT = 'wght';
/** How many stops a ramp has on an axis other than weight. */
const AXIS_STEPS = 9;
const WEIGHT_EVERY = 100;

export interface RampStop {
  value: number;
  /** The named instance at this weight; null between instances and on other axes. */
  name: string | null;
}

const hasRange = (axis: Axis): boolean => (axis.max ?? 0) > (axis.min ?? 0);

/** The axes that can be ramped, weight first, the others in the font's order. */
export function rampAxes(axes: readonly Axis[]): Axis[] {
  const usable = axes.filter(hasRange);
  return [...usable.filter((a) => a.tag === WEIGHT), ...usable.filter((a) => a.tag !== WEIGHT)];
}

/** Nine stops from the axis minimum to its maximum, at the precision of the axis slider. */
function evenStops(axis: Axis): number[] {
  const [min, max] = [axis.min ?? 0, axis.max ?? 0];
  const decimals = axisStep(axis) >= 1 ? 0 : axisStep(axis) >= 0.1 ? 1 : 2;
  return Array.from({ length: AXIS_STEPS }, (_, i) =>
    Number((min + (i * (max - min)) / (AXIS_STEPS - 1)).toFixed(decimals)),
  );
}

/** Every 100 inside the weight axis; a range too narrow to hold one gets even stops. */
function weightStops(axis: Axis): number[] {
  const stops: number[] = [];
  const first = Math.ceil((axis.min ?? 0) / WEIGHT_EVERY) * WEIGHT_EVERY;
  for (let v = first; v <= (axis.max ?? 0); v += WEIGHT_EVERY) stops.push(v);
  return stops.length ? stops : evenStops(axis);
}

/** The stops of `axis`; on the weight axis each has the name of the instance it reads as. */
export function rampStops(axis: Axis, instances: readonly NamedInstance[]): RampStop[] {
  if (axis.tag !== WEIGHT) return evenStops(axis).map((value) => ({ value, name: null }));
  const marks = weightMarks(instances);
  return weightStops(axis).map((value) => ({ value, name: instanceNear(marks, value) }));
}

/**
 * The stop the accent ring is on, or -1: the closest one. On the weight axis it must also be
 * within ±12, the distance at which the axis row names an instance; another axis has no such
 * unit, so its ring stays on the closest of its nine stops.
 */
export function ringedStop(axis: Axis, stops: readonly RampStop[], value: number): number {
  let best = -1;
  stops.forEach((stop, i) => {
    if (best < 0 || Math.abs(stop.value - value) < Math.abs(stops[best]!.value - value)) best = i;
  });
  if (best >= 0 && axis.tag === WEIGHT && Math.abs(stops[best]!.value - value) > NEAR_INSTANCE)
    return -1;
  return best;
}
