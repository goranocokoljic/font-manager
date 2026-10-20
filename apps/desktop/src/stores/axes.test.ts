import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import type { Axis, NamedInstance } from '../api/types';
import { axisStep, useAxesStore } from './axes';

const wght: Axis = { tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 };
const wdth: Axis = { tag: 'wdth', name: 'Width', min: 75, default: 100, max: 125 };
const slnt: Axis = { tag: 'slnt', name: 'Slant', min: -10, default: 0, max: 0 };
const axes = [wght, wdth];

beforeEach(() => setActivePinia(createPinia()));

describe('axes store', () => {
  it('starts every axis at its default, with nothing set', () => {
    const s = useAxesStore();
    expect(s.value(1, wght)).toBe(400);
    expect(s.isChanged(1)).toBe(false);
    expect(s.values(1)).toBeUndefined();
  });

  it('clamps a value to the axis range', () => {
    const s = useAxesStore();
    s.set(1, wght, 1200);
    expect(s.value(1, wght)).toBe(900);
    s.set(1, wght, 20);
    expect(s.value(1, wght)).toBe(100);
    expect(s.values(1)).toEqual([{ tag: 'wght', value: 100 }]);
  });

  it('clamps a typed value to the axis range', () => {
    const s = useAxesStore();
    expect(s.setTyped(1, wght, ' 1000 ')).toBe(true);
    expect(s.value(1, wght)).toBe(900);
    expect(s.setTyped(1, slnt, '-12.5')).toBe(true);
    expect(s.value(1, slnt)).toBe(-10);
    expect(s.setTyped(1, wdth, '.5')).toBe(true);
    expect(s.value(1, wdth)).toBe(75);
  });

  it('rejects typed text that is not a number and changes nothing', () => {
    const s = useAxesStore();
    s.set(1, wght, 650);
    for (const text of ['', 'bold', '7OO', '0x200', 'Infinity', 'NaN', '1e3', '4 00']) {
      expect(s.setTyped(1, wght, text), text).toBe(false);
      expect(s.value(1, wght)).toBe(650);
    }
    expect(s.values(1)).toEqual([{ tag: 'wght', value: 650 }]);
    expect(s.set(1, wght, Number.NaN)).toBe(false);
    expect(s.value(1, wght)).toBe(650);
  });

  it('Reset returns every axis of the face to its default', () => {
    const s = useAxesStore();
    s.set(1, wght, 700);
    s.set(1, wdth, 80);
    s.set(2, wght, 300);
    expect(s.isChanged(1)).toBe(true);
    s.reset(1);
    expect(s.value(1, wght)).toBe(400);
    expect(s.value(1, wdth)).toBe(100);
    expect(s.isChanged(1)).toBe(false);
    // Another face keeps its own coordinates.
    expect(s.value(2, wght)).toBe(300);
  });

  it('forgets an axis moved back to its default', () => {
    const s = useAxesStore();
    s.set(1, wght, 700);
    s.set(1, wght, 400);
    expect(s.isChanged(1)).toBe(false);
  });

  it('sets every axis to a named instance, the axes it leaves out to their default', () => {
    const s = useAxesStore();
    s.set(1, wdth, 80);
    const bold: NamedInstance = { name: 'Bold', coordinates: [{ tag: 'wght', value: 700 }] };
    s.applyInstance(1, axes, bold);
    expect(s.value(1, wght)).toBe(700);
    expect(s.value(1, wdth)).toBe(100);
    expect(s.isAt(1, axes, bold)).toBe(true);
    s.set(1, wght, 701);
    expect(s.isAt(1, axes, bold)).toBe(false);
  });

  it('gives a coordinate for every axis, in the font order, for a pin', () => {
    const s = useAxesStore();
    s.set(1, wdth, 90);
    expect(s.coordinates(1, axes)).toEqual([
      { tag: 'wght', value: 400 },
      { tag: 'wdth', value: 90 },
    ]);
  });

  it('steps a slider finer on a narrow axis', () => {
    expect(axisStep(wght)).toBe(1);
    expect(axisStep(wdth)).toBe(0.1);
    expect(axisStep(slnt)).toBe(0.1);
    expect(axisStep({ ...slnt, min: 0, max: 1 })).toBe(0.01);
  });
});
