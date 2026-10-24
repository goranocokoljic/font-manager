import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, MOCK_CATALOG_CACHED_AT, MOCK_NOW } from '../api/mock';
import { useCatalogsStore } from './catalogs';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
});

describe('catalogs store', () => {
  it('shows and closes a catalog', () => {
    const store = useCatalogsStore();
    expect(store.open).toBeNull();
    store.show('adobeFonts');
    expect(store.open).toBe('adobeFonts');
    store.close();
    expect(store.open).toBeNull();
  });

  it('opening a catalog ends the Quick Collection review, and the review closes it', async () => {
    const store = useCatalogsStore();
    const quick = useQuickCollectionStore();
    quick.show();
    store.show('adobeFonts');
    expect(quick.open).toBe(false);
    quick.show();
    await flushPromises();
    expect(store.open).toBeNull();
  });

  it('opening a collection or All fonts closes the catalog', async () => {
    const store = useCatalogsStore();
    store.show('adobeFonts');
    useSearchStore().setFilters({ collection: null });
    await flushPromises();
    expect(store.open).toBeNull();
  });
});

describe('catalogs store: online catalogs (E22.2)', () => {
  /** The mock client, recording the catalog calls it gets. */
  function recording(client = createMockClient()) {
    const calls: string[] = [];
    const wrap =
      <A extends unknown[], R>(name: string, f: (...a: A) => R) =>
      (...a: A) => {
        calls.push(name);
        return f(...a);
      };
    setClient({
      ...client,
      setCatalogEnabled: wrap('setCatalogEnabled', client.setCatalogEnabled),
      refreshCatalog: wrap('refreshCatalog', client.refreshCatalog),
      listCatalogFamilies: wrap('listCatalogFamilies', client.listCatalogFamilies),
    });
    return calls;
  }

  it('lists the catalogs off, and turning one on fetches its list once', async () => {
    const calls = recording();
    const store = useCatalogsStore();
    await store.load();
    expect(store.catalogs.map((c) => [c.id, c.enabled])).toEqual([['googleFonts', false]]);

    store.show('googleFonts');
    await flushPromises();
    expect(calls, 'an off catalog reads nothing').toEqual([]);
    expect(store.total).toBe(0);

    await store.setEnabled('googleFonts', true);
    await flushPromises();
    expect(calls).toEqual(['setCatalogEnabled', 'refreshCatalog', 'listCatalogFamilies']);
    expect(store.current?.refreshedAt).toBe(MOCK_NOW);
    expect(store.total).toBe(4);
    expect(store.families.map((f) => f?.name)).toEqual(['Inter', 'Lato', 'Lora', 'Space Mono']);

    calls.length = 0;
    await store.setEnabled('googleFonts', false);
    await store.setEnabled('googleFonts', true);
    expect(calls, 'a refreshed catalog is not fetched again').not.toContain('refreshCatalog');
  });

  it('searches through the core and drops the answer of an older search', async () => {
    const client = createMockClient();
    let release: () => void = () => undefined;
    const slow = new Promise<void>((r) => (release = r));
    setClient({
      ...client,
      listCatalogFamilies: async (req) => {
        if (req.query === 'l') await slow;
        return client.listCatalogFamilies(req);
      },
    });
    const store = useCatalogsStore();
    await store.load();
    await store.setEnabled('googleFonts', true);
    store.show('googleFonts');
    await flushPromises();
    store.setQuery('l');
    store.setQuery('lo');
    await flushPromises();
    expect(store.families.map((f) => f?.name)).toEqual(['Lora']);
    release();
    await flushPromises();
    expect(
      store.families.map((f) => f?.name),
      'the older answer is dropped',
    ).toEqual(['Lora']);
    expect(store.total).toBe(1);
  });

  it('a failed refresh keeps the cached list and reads as offline', async () => {
    setClient(createMockClient({ catalog: 'offline' }));
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    await flushPromises();
    expect(store.total).toBe(4);
    expect(store.offline).toBe(false);

    expect(await store.refresh()).toBe(false);
    expect(store.refreshError?.code).toBe('io');
    expect(store.offline).toBe(true);
    expect(store.total, 'the cached list stays').toBe(4);
    expect(store.current?.refreshedAt).toBe(MOCK_CATALOG_CACHED_AT);
  });

  it('the webview going offline shows the cached list as offline', async () => {
    setClient(createMockClient({ catalog: 'offline' }));
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    window.dispatchEvent(new Event('offline'));
    expect(store.offline).toBe(true);
    window.dispatchEvent(new Event('online'));
    expect(store.offline).toBe(false);
  });
});

describe('catalogs store: opened before the list arrives', () => {
  it('reads the families once the catalogs are loaded', async () => {
    setClient(createMockClient({ catalog: 'offline' }));
    const store = useCatalogsStore();
    store.show('googleFonts');
    await flushPromises();
    expect(store.total).toBe(0);
    await store.load();
    await flushPromises();
    expect(store.total).toBe(4);
  });
});
