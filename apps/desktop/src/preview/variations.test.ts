import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { Axis } from '../api/types';
import { clearFaceAxes, faceAxes, variationSettings } from './variations';

const wght: Axis = { tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 };
const wdth: Axis = { tag: 'wdth', name: 'Width', min: 75, default: 100, max: 125 };

describe('variationSettings', () => {
  it('is normal for a static face', () => {
    expect(variationSettings([])).toBe('normal');
  });

  it('puts every axis at its default', () => {
    expect(variationSettings([wght, wdth])).toBe('"wght" 400, "wdth" 100');
  });

  it('takes the given coordinates over the defaults', () => {
    expect(variationSettings([wght, wdth], [{ tag: 'wdth', value: 87.5 }])).toBe(
      '"wght" 400, "wdth" 87.5',
    );
  });

  it('skips axes without a usable value or tag', () => {
    const noDefault: Axis = { ...wght, default: null };
    const badTag: Axis = { ...wdth, tag: 'w"); x' };
    expect(variationSettings([noDefault, badTag])).toBe('normal');
    expect(variationSettings([wght], [{ tag: 'wght', value: Number.NaN }])).toBe('"wght" 400');
  });
});

describe('faceAxes', () => {
  beforeEach(() => clearFaceAxes());
  afterEach(() => setClient(undefined));

  it('fetches the axes of a face once', async () => {
    const client = createMockClient();
    const getFace = vi.spyOn(client, 'getFace');
    setClient(client);
    expect(await faceAxes(1)).toEqual([wght]);
    expect(await faceAxes(1)).toEqual([wght]);
    expect(getFace).toHaveBeenCalledTimes(1);
  });

  it('asks again after a failure', async () => {
    const client = createMockClient();
    const getFace = vi.spyOn(client, 'getFace');
    setClient(client);
    await expect(faceAxes(999)).rejects.toMatchObject({ code: 'notFound' });
    await expect(faceAxes(999)).rejects.toMatchObject({ code: 'notFound' });
    expect(getFace).toHaveBeenCalledTimes(2);
  });
});
