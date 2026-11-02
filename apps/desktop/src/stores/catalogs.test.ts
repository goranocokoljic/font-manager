import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { watch } from 'vue';
import type { CatalogDownload } from '../api/types';
import {
  createMockClient,
  MOCK_CATALOG_CACHED_AT,
  MOCK_NOW,
  MOCK_SUGGESTED_DOWNLOAD_FOLDER,
  mockPickedFolders,
} from '../api/mock';
import { installedActive, useCatalogsStore } from './catalogs';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';
import { useSettingsStore } from './settings';

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
  mockPickedFolders.length = 0;
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
    expect(checks, 'turning a catalog on fetches its list and checks nothing').toEqual([]);
    expect(store.updates).toBeNull();

    expect(await store.refresh()).toBe(true);
    expect(checks, 'Refresh compares with the new list, no request').toEqual([true]);
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

describe('catalogs store: install (E22.7)', () => {
  async function opened(options: Parameters<typeof createMockClient>[0] = {}) {
    const client = createMockClient({ catalog: 'offline', scanStepMs: 2, ...options });
    setClient(client);
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    await flushPromises();
    const inter = store.families.find((f) => f?.name === 'Inter')!;
    return { client, store, inter };
  }

  it('the first Install proposes the Documents folder, saves it when used, then installs', async () => {
    const { client, store, inter } = await opened();
    const settings = useSettingsStore();
    await store.askInstall(inter);
    expect(store.install?.phase).toBe('proposing');
    expect(store.install?.proposed).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    expect(settings.settings?.downloadFolder, 'not saved before the user says so').toBeNull();
    expect((await client.listCatalogDownloads()).length, 'nothing downloaded yet').toBe(0);

    const phases: string[] = [];
    const stop = watch(
      () => store.install?.phase,
      (p) => p && phases.push(p),
      { flush: 'sync' },
    );
    await store.useProposedFolder();
    stop();
    expect(phases).toEqual(['downloading', 'scanning', 'done']);
    expect(settings.settings?.downloadFolder).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    const result = store.install!.result!;
    expect(result.alreadyInstalled).toBe(false);
    expect(result.download.folder).toBe(`${MOCK_SUGGESTED_DOWNLOAD_FOLDER}/Inter`);
    expect(result.faces.length).toBeGreaterThan(0);
    expect(installedActive(result)).toBe(result.faces.length);
    const active = (await client.listActive()).map((a) => a.face);
    expect(result.faces.every((f) => active.includes(f))).toBe(true);
    expect(
      (await client.listSources()).some((s) => s.path === MOCK_SUGGESTED_DOWNLOAD_FOLDER),
    ).toBe(true);
  });

  it('a later Install installs at once, and a family already there downloads nothing', async () => {
    const { client, store, inter } = await opened();
    await useSettingsStore().update({ downloadFolder: '/Users/demo/Installed' });
    await store.askInstall(inter);
    expect(store.install?.phase).toBe('done');
    expect(store.install?.result?.download.folder).toBe('/Users/demo/Installed/Inter');
    store.closeInstall();
    expect(store.install).toBeNull();

    await store.askInstall(inter);
    const again = store.install!.result!;
    expect(again.alreadyInstalled).toBe(true);
    expect(again.outcomes, 'already active: nothing sent again').toEqual([]);
    expect(installedActive(again)).toBe(again.faces.length);
    expect((await client.listCatalogDownloads()).length).toBe(1);
  });

  it('Choose… installs into the folder picked; a cancelled picker keeps the proposal', async () => {
    const { store, inter } = await opened();
    await store.askInstall(inter);
    mockPickedFolders.push(null);
    await store.chooseInstallFolder();
    expect(store.install?.phase).toBe('proposing');
    expect(useSettingsStore().settings?.downloadFolder).toBeNull();

    mockPickedFolders.push('/Users/demo/My fonts');
    await store.chooseInstallFolder();
    expect(store.install?.phase).toBe('done');
    expect(useSettingsStore().settings?.downloadFolder).toBe('/Users/demo/My fonts');
    expect(store.install?.result?.download.folder).toBe('/Users/demo/My fonts/Inter');
  });

  it('an install turns both buttons off while it runs and says why it failed', async () => {
    const { store, inter } = await opened({ catalogDownloadMs: 20 });
    await useSettingsStore().update({ downloadFolder: '/Users/demo/Installed' });
    const running = store.askInstall(inter);
    await flushPromises();
    expect(store.install?.phase).toBe('downloading');
    expect(store.downloading).toBe(true);
    store.closeInstall();
    expect(store.install, 'kept while it runs').not.toBeNull();
    await running;
    expect(store.downloading).toBe(false);

    store.closeInstall();
    await useSettingsStore().update({ downloadFolder: '/System/Library/Fonts/Mine' });
    await store.askInstall(inter);
    expect(store.install?.phase).toBe('failed');
    expect(store.install?.error?.code).toBe('conflict');
    expect(store.install?.error?.message).toContain('system font folder');
  });

  it('Settings "Change…" saves the folder picked and keeps it on a cancel', async () => {
    await opened();
    const store = useCatalogsStore();
    mockPickedFolders.push('/Users/demo/Other');
    expect(await store.changeDownloadFolder()).toBe(true);
    expect(useSettingsStore().settings?.downloadFolder).toBe('/Users/demo/Other');
    mockPickedFolders.push(null);
    expect(await store.changeDownloadFolder()).toBe(false);
    expect(useSettingsStore().settings?.downloadFolder).toBe('/Users/demo/Other');
  });

  it('counts as active only the faces that are active after the install', () => {
    const download: CatalogDownload = {
      catalog: 'googleFonts',
      family: 'X',
      catalogFamilyId: 'X',
      version: null,
      kind: 'static',
      downloadedAt: MOCK_NOW,
      source: 1,
      folder: '/x',
      files: [],
    };
    expect(
      installedActive({
        download,
        faces: [1, 2, 3, 4],
        outcomes: [
          { face: 1, outcome: { status: 'done' } },
          { face: 2, outcome: { status: 'conflict', with: { kind: 'system', path: '/a' } } },
          { face: 3, outcome: { status: 'skipped', reason: 'broken' } },
        ],
        alreadyInstalled: false,
        error: null,
      }),
      'face 4 was active already; 2 clashed and 3 was skipped',
    ).toBe(2);
  });
});
