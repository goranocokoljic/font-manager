import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ref } from 'vue';
import type { Axis } from '../api/types';
import { useAxesStore } from '../stores/axes';
import { useWeightScrub } from './useWeightScrub';

const wght: Axis = { tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 };
const FACE = 1;

function setup(axis: Axis | null = wght) {
  const el = document.createElement('p');
  el.setPointerCapture = vi.fn();
  document.body.append(el);
  const scrub = useWeightScrub(ref(FACE), ref(axis));
  const event = (clientX: number, more: Partial<PointerEvent> = {}) =>
    ({ button: 0, pointerId: 7, clientX, currentTarget: el, ...more }) as unknown as PointerEvent;
  return { el, scrub, event, weight: () => useAxesStore().value(FACE, wght) };
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => {
  document.body.innerHTML = '';
});

describe('scrubbing the weight on a specimen', () => {
  it('captures the pointer and moves 2 units a pixel', () => {
    const { el, scrub, event, weight } = setup();
    scrub.onPointerDown(event(100));
    expect(el.setPointerCapture).toHaveBeenCalledWith(7);
    scrub.onPointerMove(event(150));
    expect(weight()).toBe(500);
    scrub.onPointerMove(event(140));
    expect(weight()).toBe(480);
  });

  it('moves 1 unit a pixel while Shift is held', () => {
    const { scrub, event, weight } = setup();
    scrub.onPointerDown(event(100));
    scrub.onPointerMove(event(150, { shiftKey: true }));
    expect(weight()).toBe(450);
  });

  it('runs the other way in a right-to-left layout', () => {
    const { el, scrub, event, weight } = setup();
    el.style.direction = 'rtl';
    scrub.onPointerDown(event(100));
    scrub.onPointerMove(event(150));
    expect(weight()).toBe(300);
  });

  it('stops at the end of the axis, and coming back moves at once', () => {
    const { scrub, event, weight } = setup();
    scrub.onPointerDown(event(0));
    scrub.onPointerMove(event(5000));
    expect(weight()).toBe(900);
    scrub.onPointerMove(event(4990));
    expect(weight()).toBe(880);
  });

  it('ignores moves with no press, after the end, and from another pointer', () => {
    const { scrub, event, weight } = setup();
    scrub.onPointerMove(event(150));
    expect(weight()).toBe(400);
    scrub.onPointerDown(event(100));
    scrub.onPointerMove(event(150, { pointerId: 8 }));
    expect(weight()).toBe(400);
    scrub.onPointerEnd(event(100));
    scrub.onPointerMove(event(200));
    expect(weight()).toBe(400);
  });

  it('does nothing for another button or a face with no weight axis', () => {
    const right = setup();
    right.scrub.onPointerDown(right.event(100, { button: 2 }));
    expect(right.el.setPointerCapture).not.toHaveBeenCalled();
    right.scrub.onPointerMove(right.event(150));
    expect(right.weight()).toBe(400);

    const none = setup(null);
    none.scrub.onPointerDown(none.event(100));
    expect(none.el.setPointerCapture).not.toHaveBeenCalled();
  });
});
