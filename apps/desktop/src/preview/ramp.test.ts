import { describe, expect, it } from 'vitest';
import type { Axis, NamedInstance } from '../api/types';
import { rampAxes, rampStops, ringedStop } from './ramp';

const axis = (tag: string, min: number, max: number, name = tag): Axis => ({
  tag,
  name,
  min,
  default: min,
  max,
});
const WGHT = axis('wght', 100, 900);
const instance = (name: string, value: number): NamedInstance => ({
  name,
  coordinates: [{ tag: 'wght', value }],
});

describe('rampAxes', () => {
  it('puts weight first and keeps the font order for the rest', () => {
    const axes = [axis('opsz', 9, 144), axis('SOFT', 0, 100), WGHT];
    expect(rampAxes(axes).map((a) => a.tag)).toEqual(['wght', 'opsz', 'SOFT']);
  });

  it('leaves out an axis with no range', () => {
    expect(rampAxes([axis('wdth', 100, 100), WGHT]).map((a) => a.tag)).toEqual(['wght']);
    expect(rampAxes([])).toEqual([]);
  });
});

describe('rampStops', () => {
  it('stops at every 100 inside the weight axis, not at its ends', () => {
    const stops = rampStops(axis('wght', 250, 850), []).map((s) => s.value);
    expect(stops).toEqual([300, 400, 500, 600, 700, 800]);
  });

  it('names a weight stop only where an instance is, within ±12', () => {
    const stops = rampStops(WGHT, [
      instance('Regular', 400),
      instance('Book', 512),
      instance('Off', 613),
    ]);
    const name = (value: number) => stops.find((s) => s.value === value)?.name;
    expect(name(400)).toBe('Regular');
    expect(name(500)).toBe('Book');
    expect(name(600)).toBeNull();
    expect(name(100)).toBeNull();
  });

  it('gives a weight axis too narrow for a hundred even stops', () => {
    const stops = rampStops(axis('wght', 420, 480), []).map((s) => s.value);
    expect(stops).toHaveLength(9);
    expect(stops[0]).toBe(420);
    expect(stops[8]).toBe(480);
  });

  it('gives another axis nine stops from end to end, without names', () => {
    const stops = rampStops(axis('opsz', 12, 24), [instance('Regular', 400)]);
    expect(stops.map((s) => s.value)).toEqual([12, 13.5, 15, 16.5, 18, 19.5, 21, 22.5, 24]);
    expect(stops.every((s) => s.name === null)).toBe(true);
  });

  it('rounds to the precision of the axis slider', () => {
    const stops = rampStops(axis('CRSV', 0, 1), []).map((s) => s.value);
    expect(stops).toEqual([0, 0.13, 0.25, 0.38, 0.5, 0.63, 0.75, 0.88, 1]);
  });
});

describe('ringedStop', () => {
  const stops = rampStops(WGHT, []);

  it('rings a weight stop within ±12 and none past it', () => {
    expect(stops[ringedStop(WGHT, stops, 412)]?.value).toBe(400);
    expect(stops[ringedStop(WGHT, stops, 388)]?.value).toBe(400);
    expect(ringedStop(WGHT, stops, 413)).toBe(-1);
    expect(ringedStop(WGHT, stops, 387)).toBe(-1);
  });

  it('rings the closest stop of another axis wherever the value is', () => {
    const opsz = axis('opsz', 12, 24);
    const own = rampStops(opsz, []);
    expect(own[ringedStop(opsz, own, 13.6)]?.value).toBe(13.5);
    expect(own[ringedStop(opsz, own, 23)]?.value).toBe(22.5);
  });

  it('rings nothing with no stops', () => {
    expect(ringedStop(WGHT, [], 400)).toBe(-1);
  });
});
