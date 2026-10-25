import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceId } from '../api/types';
import { useFaceMetricsStore } from './faceMetrics';
import { useLibraryStore } from './library';

let calls: FaceId[];
let fail: boolean;

beforeEach(() => {
  setActivePinia(createPinia());
  calls = [];
  fail = false;
  const mock = createMockClient();
  const client: FmClient = {
    ...mock,
    getFace: (face) => {
      calls.push(face);
      return fail
        ? Promise.reject({ code: 'internal', message: 'x', detail: null })
        : mock.getFace(face);
    },
  };
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('face metrics store', () => {
  it("gives each face's own metrics from its detail", async () => {
    const store = useFaceMetricsStore();
    store.want(1);
    store.want(6);
    expect(store.get(1)).toBeUndefined();
    await flushPromises();
    expect(store.get(1)).toMatchObject({ unitsPerEm: 1000, xHeight: 520, capHeight: 720 });
    expect(store.get(6)).toMatchObject({ xHeight: 550, capHeight: 730 });
  });

  it('asks once per face, also while the first ask is in flight', async () => {
    const store = useFaceMetricsStore();
    store.want(7);
    store.want(7);
    await flushPromises();
    store.want(7);
    await flushPromises();
    expect(calls).toEqual([7]);
    // Face 7 declares no x-height: the overlay draws no x-height line for it.
    expect(store.get(7)?.xHeight).toBeNull();
  });

  it('keeps no metrics when the call fails', async () => {
    fail = true;
    const store = useFaceMetricsStore();
    store.want(1);
    await flushPromises();
    expect(store.get(1)).toBeUndefined();
  });

  it('forgets every face when the library changes', async () => {
    const store = useFaceMetricsStore();
    store.want(1);
    await flushPromises();
    expect(store.get(1)).toBeDefined();
    await useLibraryStore().load();
    await flushPromises();
    expect(store.get(1)).toBeUndefined();
    store.want(1);
    await flushPromises();
    expect(calls).toEqual([1, 1]);
  });
});
