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
    expect(store.catalogs.map((c) => [c.id, c.enabled])).toEqual([
      ['googleFonts', false],
      ['fontshare', false],
    ]);

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

  it("a download follows its own rescan's events to the end, not another scan's", async () => {
    const client = createMockClient({ catalog: 'offline', scanStepMs: 2 });
    setClient(client);
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    await flushPromises();
    const lato = store.families.find((f) => f?.name === 'Lato')!;
    store.askDownload(lato);
    expect(store.download?.phase).toBe('choosing');

    // Another scan runs at the same time; its events are not the download's.
    const other = await client.scan({ source: null });
    const seen: number[] = [];
    await client.onEvent((e) => {
      if (e.type === 'scanProgress' && e.job === store.download?.result?.scan) seen.push(e.done);
    });
    await store.startDownload(1, 'static');
    expect(store.download?.phase).toBe('scanning');
    expect(store.download?.result?.scan).not.toBe(other);
    expect(store.downloading).toBe(true);
    store.closeDownload();
    expect(store.download, 'kept open while it runs').not.toBeNull();

    await new Promise((r) => setTimeout(r, 60));
    expect(store.download?.phase).toBe('done');
    expect(store.download?.scan).toEqual({ done: 7, total: 7 });
    expect(seen.length).toBeGreaterThan(1);
    expect(store.download?.error).toBeNull();
    store.closeDownload();
    expect(store.download).toBeNull();
  });

  it('switching to another catalog closes a finished or unstarted download panel', async () => {
    setClient(createMockClient({ catalog: 'offline' }));
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    await flushPromises();
    store.askDownload(store.families[0]!);
    store.show('fontshare');
    expect(store.download).toBeNull();
  });
});

describe('catalogs store: update check (E22.4)', () => {
  /** The mock with three earlier downloads, failing the test on any check at startup. */
  function counting(options = {}) {
    const client = createMockClient({ catalogUpdates: true, ...options });
    const checks: boolean[] = [];
    setClient({
      ...client,
      checkUpdates: (req) => {
        checks.push(req.cachedOnly ?? false);
        return client.checkUpdates(req);
      },
    });
    return checks;
  }

  it('checks nothing on start; the button refreshes and finds the updates', async () => {
    const checks = counting();
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    await flushPromises();
    expect(checks, 'nothing checks on start').toEqual([]);
    expect(store.updates).toBeNull();
    expect(store.libraryUpdate('Inter')).toBeNull();

    await store.setEnabled('googleFonts', true);
    expect(checks, 'the first Refresh compares with the new list, no request').toEqual([true]);
    expect(store.updateOf('googleFonts', 'Inter')?.availableVersion).toBe('2026-09-01');

    expect(await store.checkUpdates()).toBe(true);
    expect(checks).toEqual([true, false]);
    expect(store.updates?.map((u) => [u.family, u.status])).toEqual([
      ['Inter', 'updateAvailable'],
      ['Retired Sans', 'noLongerListed'],
    ]);
    expect(store.libraryUpdate('Inter')?.downloadedVersion).toBe('2026-03-01');
    expect(store.libraryUpdate('Lato'), 'the same version').toBeNull();
    expect(store.libraryUpdate('Retired Sans'), 'no longer listed is no update').toBeNull();
    expect(store.updateOf('fontshare', 'Inter')).toBeNull();
  });

  it('a failed check says why and keeps the last answer', async () => {
    counting({ catalog: 'offline' });
    const store = useCatalogsStore();
    await store.load();
    expect(await store.checkUpdates()).toBe(false);
    expect(store.checkError?.code).toBe('io');
    expect(store.updates).toBeNull();
    expect(store.checking).toBe(false);
  });
});
