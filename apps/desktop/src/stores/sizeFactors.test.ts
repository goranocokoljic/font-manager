import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { SizeFactorRequest } from '../api/types';
import { useLibraryStore } from './library';
import { useSizeFactorsStore } from './sizeFactors';

let calls: SizeFactorRequest[];
let fail: boolean;

beforeEach(() => {
  setActivePinia(createPinia());
  calls = [];
  fail = false;
  const mock = createMockClient();
  const client: FmClient = {
    ...mock,
    sizeFactors: (req) => {
      calls.push(structuredClone(req));
      return fail
        ? Promise.reject({ code: 'internal', message: 'x', detail: null })
        : mock.sizeFactors(req);
    },
  };
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('size factors store', () => {
  it('batches the asks of one tick into one call per basis', async () => {
    const sizes = useSizeFactorsStore();
    sizes.want(1, 'xHeight');
    sizes.want(6, 'xHeight');
    sizes.want(1, 'capHeight');
    sizes.want(1, 'xHeight');
    expect(sizes.get(1, 'xHeight')).toBe(1);
    await flushPromises();
    expect(calls).toEqual([
      { faces: [1, 6], basis: 'xHeight' },
      { faces: [1], basis: 'capHeight' },
    ]);
    expect(sizes.get(1, 'xHeight')).toBeCloseTo(0.5 / 0.52);
    expect(sizes.get(6, 'xHeight')).toBeCloseTo(0.5 / 0.55);
    expect(sizes.get(1, 'capHeight')).toBeCloseTo(0.7 / 0.72);
  });

  it('does not ask again for a known factor', async () => {
    const sizes = useSizeFactorsStore();
    sizes.want(7, 'xHeight');
    await flushPromises();
    sizes.want(7, 'xHeight');
    await flushPromises();
    expect(calls).toHaveLength(1);
    // Face 7 declares no x-height: factor 1.
    expect(sizes.get(7, 'xHeight')).toBe(1);
  });

  it('keeps factor 1 when the call fails', async () => {
    fail = true;
    const sizes = useSizeFactorsStore();
    sizes.want(1, 'xHeight');
    await flushPromises();
    expect(sizes.get(1, 'xHeight')).toBe(1);
  });

  it('forgets every factor when the library changes', async () => {
    const sizes = useSizeFactorsStore();
    sizes.want(1, 'xHeight');
    await flushPromises();
    expect(sizes.get(1, 'xHeight')).not.toBe(1);
    await useLibraryStore().load();
    await flushPromises();
    expect(sizes.get(1, 'xHeight')).toBe(1);
    sizes.want(1, 'xHeight');
    await flushPromises();
    expect(calls).toHaveLength(2);
  });
});
