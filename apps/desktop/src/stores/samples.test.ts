import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { SampleTextRequest } from '../api/types';
import { useLibraryStore } from './library';
import { useSamplesStore } from './samples';

let calls: SampleTextRequest[];
let fail: boolean;

beforeEach(() => {
  setActivePinia(createPinia());
  calls = [];
  fail = false;
  const mock = createMockClient();
  const client: FmClient = {
    ...mock,
    sampleTexts: (req) => {
      calls.push(structuredClone(req));
      return fail
        ? Promise.reject({ code: 'internal', message: 'x', detail: null })
        : mock.sampleTexts(req);
    },
  };
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('samples store', () => {
  it('batches the asks of one tick into one call per sample text', async () => {
    const samples = useSamplesStore();
    samples.want(1, 'Љубав');
    samples.want(7, 'Љубав');
    samples.want(7, 'Love');
    samples.want(1, 'Љубав');
    await flushPromises();
    expect(calls).toEqual([
      { faces: [1, 7], text: 'Љубав' },
      { faces: [7], text: 'Love' },
    ]);
    expect(samples.get(1, 'Љубав')).toMatchObject({ text: 'Љубав', fallback: null });
    expect(samples.get(7, 'Љубав')).toMatchObject({
      text: 'abcdefghijklmnopqrstuvwxyz',
      fallback: { kind: 'script', script: 'Latn' },
      missing: ['Љ', 'у', 'б', 'а', 'в'],
    });
    expect(samples.get(7, 'Love')?.fallback).toBeNull();
  });

  it('does not ask again for a known answer or an empty sample', async () => {
    const samples = useSamplesStore();
    samples.want(7, 'Љубав');
    await flushPromises();
    samples.want(7, 'Љубав');
    samples.want(7, '');
    await flushPromises();
    expect(calls).toHaveLength(1);
  });

  it('a failed ask leaves the sample in place and is not repeated', async () => {
    fail = true;
    const samples = useSamplesStore();
    samples.want(7, 'Љубав');
    await flushPromises();
    expect(samples.get(7, 'Љубав')).toBeUndefined();
    samples.want(7, 'Љубав');
    await flushPromises();
    expect(calls).toHaveLength(1);
  });

  it('forgets every answer when the library changes', async () => {
    const library = useLibraryStore();
    await library.load();
    const samples = useSamplesStore();
    samples.want(7, 'Љубав');
    await flushPromises();
    expect(samples.get(7, 'Љубав')).toBeDefined();
    await library.refresh();
    await flushPromises();
    expect(samples.get(7, 'Љубав')).toBeUndefined();
    samples.want(7, 'Љубав');
    await flushPromises();
    expect(calls).toHaveLength(2);
  });

  it('drops answers that arrive after a reset', async () => {
    const samples = useSamplesStore();
    samples.want(7, 'Љубав');
    await Promise.resolve();
    samples.reset();
    await flushPromises();
    expect(samples.get(7, 'Љубав')).toBeUndefined();
  });
});
