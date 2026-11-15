import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FoundryInfo } from '../api/types';
import { FOUNDRIES_SHOWN, largestFoundries, useFoundriesStore } from './foundries';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useToastStore } from './toast';

let client: FmClient;

async function setup() {
  client = createMockClient({ library: 'wall' });
  setClient(client);
  await useLibraryStore().load();
  const store = useFoundriesStore();
  await store.refresh();
  return store;
}

const info = (name: string, families: number, unknown = false): FoundryInfo => ({
  name,
  families,
  rawNames: [],
  merged: [],
  unknown,
});

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('largestFoundries', () => {
  it('orders by the most families, then A to Z, leaves the unknown one out and cuts at n', () => {
    const list = [
      info('Beta', 2),
      info('alpha', 2),
      info('Gamma', 9),
      info('Delta', 1),
      info('Unknown foundry', 50, true),
    ];
    expect(largestFoundries(list, 3).map((f) => f.name)).toEqual(['Gamma', 'alpha', 'Beta']);
    expect(largestFoundries(list, 10).map((f) => f.name)).not.toContain('Unknown foundry');
  });
});

describe('the foundries store', () => {
  it('lists the ten largest of the library, with the API counts', async () => {
    const store = await setup();
    expect(store.named.length).toBe(46);
    expect(store.largest).toHaveLength(FOUNDRIES_SHOWN);
    expect(store.largest.slice(0, 4).map((f) => [f.name, f.families])).toEqual([
      ['Omnibus-Type', 5],
      ['Font Diner', 3],
      ['Impallari Type', 3],
      ['Sorkin Type', 3],
    ]);
  });

  it('merges with an undo toast; the filter follows the merged foundry; undo splits it again', async () => {
    const store = await setup();
    const search = useSearchStore();
    search.setFoundry('Font Diner');
    await flushPromises();
    expect(search.hits).toHaveLength(3);

    expect(await store.merge('Font Diner', 'Omnibus-Type')).toBe(true);
    await flushPromises();
    expect(store.find('Font Diner')).toBeUndefined();
    expect(store.find('Omnibus-Type')!.families).toBe(8);
    expect(search.foundry).toBe('Omnibus-Type');
    expect(search.hits).toHaveLength(8);

    const toast = useToastStore();
    expect(toast.toast!.message).toBe('Merged “Font Diner” into “Omnibus-Type”.');
    expect(toast.toast!.action!.label).toBe('Undo');
    toast.act();
    await flushPromises();
    expect(store.find('Font Diner')!.families).toBe(3);
    expect(store.find('Omnibus-Type')!.families).toBe(5);
    expect(search.hits).toHaveLength(5);
  });

  it('keeps the list and reports the error when the API refuses a merge', async () => {
    const store = await setup();
    const before = store.list;
    expect(await store.merge('Unknown foundry', 'Omnibus-Type')).toBe(false);
    expect(store.list).toBe(before);
    expect(store.error).not.toBeNull();
    expect(useToastStore().toast).toBeNull();
  });
});
