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

  it('asks for the library order when it searches, and not for the counts alone', async () => {
    const search = await setup();
    const library = useLibraryStore();
    await library.setOrder({ by: 'name', descending: true });
    await flushPromises();
    // No search: the one family is for the counts, an order costs for nothing.
    expect(requests.at(-1)?.order).toBeUndefined();
    search.setFilters({ slant: 'italic' });
    await flushPromises();
    expect(requests.at(-1)?.order).toEqual({ by: 'name', descending: true });
    expect(search.hits!.map((h) => h.family.name)).toEqual(['Sample 2', 'Sample 1']);
    // The order changes while filtered: the results follow it.
    await library.setOrder({ by: 'name', descending: false });
    await flushPromises();
    expect(search.hits!.map((h) => h.family.name)).toEqual(['Sample 1', 'Sample 2']);
  });

  it('asks for name order under a language filter with nothing typed, where the line says so', async () => {
    const search = await setup();
    const languages = { supports: { kind: 'languages' as const, codes: ['pl'] } };
    search.setFilters(languages);
    await flushPromises();
    expect(requests.at(-1)?.order).toEqual({ by: 'name', descending: false });
    // Another filter is in name order already, and a text search is ranked: no order.
    search.setFilters({ slant: 'italic' });
    await flushPromises();
    expect(requests.at(-1)?.order).toBeUndefined();
    search.setFilters(languages);
    search.query = 'sample';
    await search.run();
    expect(requests.at(-1)?.order).toBeUndefined();
    // A chosen order wins over it.
    search.query = '';
    await useLibraryStore().setOrder({ by: 'styleCount', descending: true });
    await flushPromises();
    expect(requests.at(-1)?.order).toEqual({ by: 'styleCount', descending: true });
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
    // Best of 3: a shared CI runner can stall one run by a few ms (main went red at 56 ms).
    let best = Infinity;
    for (let run = 0; run < 3; run++) {
      const t0 = performance.now();
      await search.run();
      best = Math.min(best, performance.now() - t0);
      expect(list.rows).toHaveLength(10_000);
    }
    expect(best).toBeLessThan(50);
  });
});

describe('the renders-your-text switch (E43.3)', () => {
  const TEXT = 'čađavog štos';

  it('counts with and without the text rule and asks which language the text is in', async () => {
    vi.useFakeTimers();
    const store = await setup();
    const asked: string[] = [];
    client.detectLanguages = async (req) => {
      asked.push(req.text);
      return [{ code: 'sr-Latn', confidence: 0.8 }];
    };
    store.setCoverText(TEXT);
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    await flushPromises();
    expect(asked).toEqual([TEXT]);
    expect(store.detected).toEqual([{ code: 'sr-Latn', confidence: 0.8 }]);
    expect(store.coverBase).toBe(store.total);
    expect(store.coverTotal).not.toBeNull();
    expect(requests.filter((r) => r.rule).length).toBe(1);
    expect(requests.filter((r) => !r.rule).length).toBe(1);
  });

  it('puts the text rule on the list search at once when on, and takes it off', async () => {
    vi.useFakeTimers();
    const store = await setup();
    store.setCoverText(TEXT);
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    await flushPromises();
    requests = [];
    store.setHideMissing(true);
    await flushPromises();
    const list = requests.find((r) => (r.page?.limit ?? 0) > 1)!;
    expect(list.rule).toEqual({
      op: 'and',
      rules: [{ kind: 'filters', filters: { supports: { kind: 'text', text: TEXT } } }],
    });
    expect(list.order).toEqual({ by: 'name', descending: false });
    // The switch changes what the list shows, not the counts: no recount.
    expect(requests.filter((r) => r.page?.limit === 1)).toEqual([]);
    expect(store.active).toBe(true);
    expect(store.hits).not.toBeNull();
    expect(store.total).toBe(store.coverTotal);

    requests = [];
    store.setHideMissing(false);
    await flushPromises();
    expect(requests.some((r) => (r.page?.limit ?? 0) > 1)).toBe(false);
    expect(store.hits).toBeNull();
  });

  it('a failed detection leaves the counts and names no language', async () => {
    vi.useFakeTimers();
    const store = await setup();
    client.detectLanguages = async () => {
      throw new Error('down');
    };
    store.setCoverText(TEXT);
    vi.advanceTimersByTime(SEARCH_DEBOUNCE_MS);
    await flushPromises();
    expect(store.detected).toEqual([]);
    expect(store.coverTotal).not.toBeNull();
  });
});

describe('the switch interrupting a search (E43.3 review COR-1)', () => {
  const TEXT = 'čađavog štos';
  const settle = () => new Promise((done) => setTimeout(done, 150));

  it('still recounts when the switch drops a search that was going to recount', async () => {
    // A client that answers a little later, as the Tauri IPC does.
    const mock = createMockClient({ extraFaces: 2, stylesPerFamily: 4 });
    const search = mock.search.bind(mock);
    mock.search = (req) => new Promise((done) => setTimeout(() => done(search(req)), 5));
    const store = await setup(mock);
    store.setCoverText(TEXT);
    await settle();
    store.setHideMissing(true);
    await settle();
    requests = [];

    // The drawer's Clear: the filters change, then the switch goes off before the answer.
    store.setFilters({ classification: ['mono'] });
    store.setHideMissing(false);
    await settle();
    const counts = requests.filter((r) => r.page?.limit === 1);
    expect(counts.length).toBeGreaterThan(0);
    expect(counts.every((r) => r.filters?.classification?.[0] === 'mono')).toBe(true);
    const base = await search({
      query: '',
      filters: { classification: ['mono'] },
      page: { offset: 0, limit: 1 },
    });
    expect(store.coverBase).toBe((await base).total);
  });
});
