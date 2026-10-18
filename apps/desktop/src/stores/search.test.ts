import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import type { FamilyHit, SearchRequest, SearchResponse } from '../api/types';
import { faceKey, familyKey } from '../list/rows';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { SEARCH_DEBOUNCE_MS, useSearchStore } from './search';

let client: FmClient;
let requests: SearchRequest[];

async function setup(c: FmClient = createMockClient({ extraFaces: 2, stylesPerFamily: 4 })) {
  client = c;
  requests = [];
  const search = client.search.bind(client);
  client.search = (req) => {
    requests.push(req);
    return search(req);
  };
  setClient(client);
  await useLibraryStore().load();
  const store = useSearchStore();
  await flushPromises();
  requests = [];
  return store;
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => {
  setClient(undefined);
  vi.useRealTimers();
});

describe('search store', () => {
  it('has counts for the chips before any search, and shows the library', async () => {
    const search = await setup();
    expect(search.hits).toBeNull();
    expect(search.counts.formats?.length).toBeGreaterThan(0);
  });

  it(`waits ${SEARCH_DEBOUNCE_MS} ms after the last key, then searches once`, async () => {
    const search = await setup();
    vi.useFakeTimers();
    search.setText('m');
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS - 1);
    search.setText('mo');
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS - 1);
    search.setText('mono');
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS - 1);
    expect(requests).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(requests.map((r) => r.query)).toEqual(['mono']);
    vi.useRealTimers();
    await flushPromises();
    expect(search.hits?.map((h) => h.family.name)).toEqual(['JetBrains Mono']);
    expect(search.total).toBe(1);
  });

  it('a chip searches at once, with the filters as rules', async () => {
    const search = await setup();
    search.setFilters({ slant: 'italic' });
    expect(requests.at(-1)?.filters).toEqual({ slant: 'italic' });
    await flushPromises();
    // Only the extra families have italics.
    expect(search.hits?.map((h) => h.family.id)).toEqual([EXTRA_ID_BASE, EXTRA_ID_BASE + 1]);
    search.clear();
    await flushPromises();
    expect(search.hits).toBeNull();
    expect(search.active).toBe(false);
  });

  it('drops the answer to an older search', async () => {
    const search = await setup();
    let release: () => void = () => undefined;
    const slow = client.search;
    client.search = (req) =>
      req.query === 'inter'
        ? new Promise<SearchResponse>((ok) => (release = () => void slow(req).then(ok)))
        : slow(req);
    search.query = 'inter';
    const first = search.run();
    search.query = 'mono';
    await search.run();
    release();
    await first;
    await flushPromises();
    expect(search.hits?.map((h) => h.family.name)).toEqual(['JetBrains Mono']);
  });

  it('a failed search keeps the list and reports the error', async () => {
    const search = await setup();
    client.search = () => Promise.reject({ code: 'internal', message: 'Index busy', detail: null });
    search.setFilters({ slant: 'italic' });
    await flushPromises();
    expect(search.error?.message).toBe('Index busy');
    expect(search.hits).toBeNull();
  });

  it('saves the chosen filters as a smart collection', async () => {
    const search = await setup();
    const create = vi.spyOn(client, 'createCollection');
    search.setFilters({ formats: ['ttf'], supports: { kind: 'languages', codes: ['pl'] } });
    const c = await search.saveAsSmartCollection('Polish TTF');
    expect(create).toHaveBeenCalledWith({
      name: 'Polish TTF',
      rules: { formats: ['ttf'], supports: { kind: 'languages', codes: ['pl'] } },
    });
    expect(c.rules).toEqual({ formats: ['ttf'], supports: { kind: 'languages', codes: ['pl'] } });
    await expect(search.saveAsSmartCollection('  ')).rejects.toMatchObject({
      code: 'invalidRequest',
    });
  });

  it('searches again when the library changes', async () => {
    const search = await setup();
    search.setFilters({ active: true });
    await flushPromises();
    expect(search.hits).toEqual([]);
    await client.activate({ target: { kind: 'faces', faces: [1] }, mode: 'session' });
    await flushPromises();
    expect(search.hits?.map((h) => h.family.name)).toEqual(['Inter']);
  });
});

describe('font list with a search', () => {
  it('shows the result families and only the matching styles of an expanded one', async () => {
    const search = await setup();
    const list = useFontListStore();
    search.setFilters({ slant: 'italic' });
    await flushPromises();
    list.expand(EXTRA_ID_BASE);
    await flushPromises();
    // Family A's styles are Regular, Bold, Italic, Bold Italic; the italic ones are A+2 and A+3.
    expect(list.rows.slice(0, 4).map((r) => r.key)).toEqual([
      familyKey(EXTRA_ID_BASE),
      faceKey(EXTRA_ID_BASE + 2),
      faceKey(EXTRA_ID_BASE + 3),
      familyKey(EXTRA_ID_BASE + 1),
    ]);
    search.clear();
    await flushPromises();
    expect(list.families).toHaveLength(5);
  });

  it('applies 10,000 result families within 50 ms of the search firing', async () => {
    // The UI's share of the budget: the API answers at once, the list must follow fast.
    const base = createMockClient();
    const families = (await base.listFamilies({ page: { offset: 0, limit: 1 } })).items[0]!;
    const hits: FamilyHit[] = Array.from({ length: 10_000 }, (_, i) => ({
      family: { ...families, id: EXTRA_ID_BASE + i, name: `Sample ${i}` },
      faces: [EXTRA_ID_BASE + i],
      score: 1,
      missing: null,
    }));
    const search = await setup(base);
    const list = useFontListStore();
    client.search = async (req) => {
      const offset = req.page?.offset ?? 0;
      const limit = req.page?.limit ?? 100;
      return { families: hits.slice(offset, offset + limit), total: hits.length, counts: {} };
    };
    search.query = 'sample';
    const t0 = performance.now();
    await search.run();
    expect(list.rows).toHaveLength(10_000);
    const ms = performance.now() - t0;
    expect(ms).toBeLessThan(50);
  });
});
