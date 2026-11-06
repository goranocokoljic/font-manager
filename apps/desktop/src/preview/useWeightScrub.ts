// Dragging sideways on a specimen scrubs the weight axis of its face (E42.2): the pointer is
// captured, 1 px is 2 units, Shift halves that. The axes store clamps to the range, so the
// value is read back on each move and a drag past the end does not build up.
import { type Ref } from 'vue';
import type { Axis, FaceId } from '../api/types';
import { useAxesStore } from '../stores/axes';

const UNITS_PER_PX = 2;

export function useWeightScrub(face: Ref<FaceId | null>, axis: Ref<Axis | null>) {
  const axes = useAxesStore();
  let drag: { pointer: number; x: number; dir: number } | null = null;

  function onPointerDown(e: PointerEvent): void {
    if (e.button !== 0 || face.value === null || !axis.value) return;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    // Like a range input, the axis runs the other way in a right-to-left layout.
    const dir = getComputedStyle(el).direction === 'rtl' ? -1 : 1;
    drag = { pointer: e.pointerId, x: e.clientX, dir };
  }

  function onPointerMove(e: PointerEvent): void {
    if (drag?.pointer !== e.pointerId || face.value === null || !axis.value) return;
    const px = (e.clientX - drag.x) * drag.dir;
    drag.x = e.clientX;
    const by = px * (e.shiftKey ? UNITS_PER_PX / 2 : UNITS_PER_PX);
    axes.set(face.value, axis.value, axes.value(face.value, axis.value) + by);
  }

  function onPointerEnd(e: PointerEvent): void {
    if (drag?.pointer === e.pointerId) drag = null;
  }

  return { onPointerDown, onPointerMove, onPointerEnd };
}
