import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, MOCK_SUGGESTED_DOWNLOAD_FOLDER } from '../api/mock';
import { poolSize } from '../list/virtual';
import { FaceLoader, catalogFamily, setFaceLoader } from '../preview/faceLoader';
import { CATALOG_VIEW, useCatalogsStore } from '../stores/catalogs';
import { previewRowHeight, usePreviewStore } from '../stores/preview';
import { useSettingsStore } from '../stores/settings';
import { useViewsStore } from '../stores/views';
import CatalogBrowser from './CatalogBrowser.vue';
import OnlineCatalogsSidebar from './OnlineCatalogsSidebar.vue';

let wrappers: VueWrapper[] = [];
/** The font files the face loader was asked for, by CSS family name. */
let loaded: Map<string, string>;

beforeEach(() => {
  setActivePinia(createPinia());
  loaded = new Map();
  setFaceLoader(
    new FaceLoader({
      fonts: { add: () => undefined, delete: () => true },
      create: (family, source) => {
        loaded.set(family, source);
        return { load: () => Promise.resolve() };
      },
      url: (face) => `/fmfont/${face}`,
    }),
  );
});

afterEach(() => {
  wrappers.forEach((w) => w.unmount());
  wrappers = [];
  setClient(undefined);
  setFaceLoader(undefined);
});

function mountIt(component: typeof CatalogBrowser | typeof OnlineCatalogsSidebar) {
  const w = mount(component, { attachTo: document.body });
  wrappers.push(w);
  return w;
}

const rows = (w: VueWrapper) => w.findAll('[data-testid="catalog-family"]');
const names = (w: VueWrapper) => rows(w).map((r) => r.attributes('data-family'));
const ALL = ['Inter', 'Lato', 'Lora', 'Space Mono'];
const refreshFails = async () => {
  throw { code: 'io', message: 'Google Fonts could not be refreshed.', detail: null };
};

describe('OnlineCatalogsSidebar with catalogs', () => {
  it('lists Google Fonts with a "Turn on" button that says it connects', async () => {
    setClient(createMockClient());
    const sidebar = mountIt(OnlineCatalogsSidebar);
    await flushPromises();
    expect(sidebar.get('[data-testid="catalog-googleFonts"]').text()).toBe('Google Fonts');
    const on = sidebar.get('[data-testid="catalog-turn-on-googleFonts"]');
    expect(on.text()).toBe('Turn on');
    expect(on.attributes('aria-label')).toBe('Turn on Google Fonts');
    const hint = sidebar.get(`#${on.attributes('aria-describedby')}`);
    expect(hint.text()).toBe('Connects to the internet');
    // Every catalog of list_catalogs is listed, each with its own switch (E22.5 added Fontshare).
    expect(sidebar.get('[data-testid="catalog-fontshare"]').text()).toBe('Fontshare');
    expect(sidebar.get('[data-testid="catalog-turn-on-fontshare"]').attributes('aria-label')).toBe(
      'Turn on Fontshare',
    );

    await on.trigger('click');
    await flushPromises();
    expect(sidebar.find('[data-testid="catalog-turn-on-googleFonts"]').exists()).toBe(false);
    expect(useCatalogsStore().catalogs[0]?.familyCount, 'its list was fetched').toBe(4);
  });
});

describe('CatalogBrowser', () => {
  async function opened(options: Parameters<typeof createMockClient>[0] = {}) {
    setClient(createMockClient(options));
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    const w = mountIt(CatalogBrowser);
    await flushPromises();
    return { w, store };
  }

  it('an off catalog shows a "Turn on" button and no list', async () => {
    const { w } = await opened();
    expect(w.find('[data-testid="catalog-off"]').exists()).toBe(true);
    expect(w.find('[data-testid="catalog-list"]').exists()).toBe(false);
    expect(w.get('[data-testid="catalog-refresh"]').attributes('disabled')).toBeDefined();
    expect(w.get('[data-testid="catalog-refreshed"]').text()).toBe('Not refreshed yet');
    await w.get('[data-testid="catalog-off"] button').trigger('click');
    await flushPromises();
    expect(names(w)).toEqual(ALL);
    expect(w.get('[data-testid="catalog-refreshed"]').text()).toMatch(/^Refreshed /);
  });

  it('previews each family in its own file through the face loader, as live text', async () => {
    const { w } = await opened({ catalog: 'offline' });
    expect(rows(w)).toHaveLength(4);
    const name = catalogFamily('googleFonts', 'Lato');
    expect(loaded.get(name)).toBe('url("/fmfont/catalog-googleFonts-4c61746f")');
    const lato = rows(w).find((r) => r.attributes('data-family') === 'Lato')!;
    expect(lato.get('.sample').attributes('style')).toContain(`"${name}"`);
    expect(w.find('img').exists(), 'no pre-baked image').toBe(false);
  });

  it('the search box filters through the core', async () => {
    const { w, store } = await opened({ catalog: 'offline' });
    await w.get('[data-testid="catalog-search"]').setValue('lo');
    await flushPromises();
    expect(store.query).toBe('lo');
    expect(names(w)).toEqual(['Lora']);
    expect(w.get('.count').text()).toBe('1 family');
    await w.get('[data-testid="catalog-search"]').setValue('zzz');
    await flushPromises();
    expect(w.text()).toContain('No family matches the search.');
  });

  it('the sample text and size of the catalog view apply to every row', async () => {
    const { w } = await opened({ catalog: 'offline' });
    usePreviewStore().update(CATALOG_VIEW, {
      sample: 'custom',
      customText: 'Hamburgefonstiv',
      sizePx: 40,
    });
    await flushPromises();
    for (const row of rows(w)) {
      expect(row.get('.sample').text()).toBe('Hamburgefonstiv');
      expect(row.get('.sample').attributes('style')).toContain('font-size: 40px');
    }
    usePreviewStore().update(CATALOG_VIEW, { sample: 'fontName' });
    await flushPromises();
    expect(rows(w).map((r) => r.get('.sample').text())).toEqual(ALL);
  });

  it('switches between List, Grid and Waterfall', async () => {
    const { w } = await opened({ catalog: 'offline' });
    const mode = (label: string) => w.findAll('.modes button').find((b) => b.text() === label)!;
    await mode('Waterfall').trigger('click');
    expect(mode('Waterfall').attributes('aria-pressed')).toBe('true');
    expect(rows(w)[0]!.findAll('.line')).toHaveLength(6);
    await mode('Grid').trigger('click');
    expect(rows(w)[0]!.classes()).toContain('grid');
    await mode('List').trigger('click');
    expect(rows(w)[0]!.classes()).toContain('list');
  });

  it('a failed refresh keeps the cached list with the offline note', async () => {
    const { w } = await opened({ catalog: 'offline' });
    expect(w.find('[data-testid="catalog-offline"]').exists()).toBe(false);
    await w.get('[data-testid="catalog-refresh"]').trigger('click');
    await flushPromises();
    expect(w.get('[data-testid="catalog-offline"]').text()).toBe(
      'Offline - showing the list from Oct 1, 2026',
    );
    expect(names(w)).toEqual(ALL);
  });

  it('a failed refresh with no list to fall back on says why', async () => {
    setClient({ ...createMockClient(), refreshCatalog: refreshFails });
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    const w = mountIt(CatalogBrowser);
    await store.setEnabled('googleFonts', true);
    await flushPromises();
    expect(w.get('[role="alert"]').text()).toBe('Google Fonts could not be refreshed.');
    expect(w.find('[data-testid="catalog-offline"]').exists()).toBe(false);
  });

  it('renders only the rows on screen of a long list and reads its pages as it scrolls', async () => {
    const client = createMockClient({ catalog: 'offline', catalogFamilies: 996 });
    const offsets: number[] = [];
    setClient({
      ...client,
      listCatalogFamilies: (req) => {
        offsets.push(req.page?.offset ?? 0);
        return client.listCatalogFamilies(req);
      },
    });
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    const w = mount(CatalogBrowser, { props: { overscan: 2 }, attachTo: document.body });
    wrappers.push(w);
    await flushPromises();
    expect(store.total).toBe(1000);
    expect(offsets, 'only the first page is read').toEqual([0]);

    const rowPx = previewRowHeight(
      usePreviewStore().settingsFor(CATALOG_VIEW),
      useSettingsStore().density,
    );
    const slots = () => w.findAll('[role="listitem"]');
    // 600 px fallback viewport: the rows on screen, one partly visible, 2 overscan each side.
    expect(slots()).toHaveLength(poolSize(600, rowPx, 2));
    expect(slots()[0]!.attributes('aria-setsize')).toBe('1000');
    expect(w.get('.spacer').attributes('style')).toContain(`height: ${1000 * rowPx}px`);

    const scrollTo = async (top: number) => {
      const list = w.get('[data-testid="catalog-list"]');
      (list.element as HTMLElement).scrollTop = top;
      await list.trigger('scroll');
      await flushPromises();
    };
    await scrollTo(1000 * rowPx);
    expect(offsets, 'the last page is read when the list reaches it').toEqual([0, 800]);
    expect(slots()).toHaveLength(poolSize(600, rowPx, 2));
    const last = slots().find((r) => r.attributes('aria-posinset') === '1000');
    expect(last?.attributes('aria-label')).toBe('Zz Catalog 00996');

    await scrollTo(250 * rowPx);
    expect(offsets).toEqual([0, 800, 200]);
    const row = slots().find((r) => r.attributes('aria-posinset') === '251');
    expect(row?.attributes('aria-label')).toBe('Zz Catalog 00247');
    expect(row?.find('[data-testid="catalog-family"]').exists()).toBe(true);
  });
});

describe('CatalogBrowser: Install (E22.7)', () => {
  async function opened(options: Parameters<typeof createMockClient>[0] = {}) {
    setClient(createMockClient({ catalog: 'offline', scanStepMs: 2, ...options }));
    const store = useCatalogsStore();
    await store.load();
    store.show('googleFonts');
    const w = mountIt(CatalogBrowser);
    await flushPromises();
    return { store, w };
  }
  const row = (w: VueWrapper, family: string) => w.get(`[data-family="${family}"]`);
  const status = (w: VueWrapper) => w.get('[data-testid="catalog-install-status"]').text();

  it('every row, card and waterfall block has Install next to Download…', async () => {
    const { w } = await opened();
    for (const mode of ['list', 'grid', 'waterfall'] as const) {
      useViewsStore().setMode(CATALOG_VIEW, mode);
      await flushPromises();
      const install = row(w, 'Lato').get('[data-testid="catalog-install-open"]');
      expect(install.text(), mode).toBe('Install');
      expect(install.attributes('aria-label')).toBe('Install Lato');
      const next = install.element.nextElementSibling as HTMLElement;
      expect(next.dataset.testid, 'Download… right after it').toBe('catalog-download-open');
    }
  });

  it('installs at once with a folder set: the line follows it, both buttons off meanwhile', async () => {
    const { store, w } = await opened({ catalogDownloadMs: 30 });
    await useSettingsStore().update({ downloadFolder: '/Users/demo/Installed' });
    await row(w, 'Inter').get('[data-testid="catalog-install-open"]').trigger('click');
    await flushPromises();
    expect(status(w)).toBe('Downloading Inter…');
    expect(w.get('[data-testid="catalog-install-status"]').attributes('role')).toBe('status');
    for (const b of w.findAll(
      '[data-testid="catalog-install-open"], [data-testid="catalog-download-open"]',
    ))
      expect(b.attributes('disabled'), 'every button is off').toBeDefined();
    expect(w.find('[data-testid="catalog-install-close"]').exists()).toBe(false);

    await new Promise((r) => setTimeout(r, 120));
    await flushPromises();
    const count = store.install!.result!.faces.length;
    expect(status(w)).toBe(
      count === 1 ? 'Installed Inter: 1 style active' : `Installed Inter: ${count} styles active`,
    );
    expect(
      row(w, 'Inter').get('[data-testid="catalog-install-open"]').attributes('disabled'),
    ).toBeUndefined();
    await w.get('[data-testid="catalog-install-close"]').trigger('click');
    expect(w.find('[data-testid="catalog-install"]').exists()).toBe(false);

    // Again: nothing is downloaded, and the line says so.
    await row(w, 'Inter').get('[data-testid="catalog-install-open"]').trigger('click');
    await flushPromises();
    expect(status(w)).toContain('Inter is already in your library, so nothing was downloaded');
  });

  it('the first Install proposes the folder; Use this folder saves it and installs', async () => {
    const { w } = await opened();
    await row(w, 'Inter').get('[data-testid="catalog-install-open"]').trigger('click');
    await flushPromises();
    const line = w.get('[data-testid="catalog-install"]');
    expect(line.get('h2').text()).toBe('Where should installed fonts go?');
    expect(status(w)).toContain(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    expect(line.get('[data-testid="catalog-install-choose"]').text()).toBe('Choose…');
    await line.get('[data-testid="catalog-install-use"]').trigger('click');
    await new Promise((r) => setTimeout(r, 60));
    await flushPromises();
    expect(useSettingsStore().settings?.downloadFolder).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    expect(status(w)).toMatch(/^Installed Inter: \d+ styles? active$/);
  });

  it('a refused install says why in an alert', async () => {
    const { w } = await opened();
    await useSettingsStore().update({ downloadFolder: '/System/Library/Fonts/Mine' });
    await row(w, 'Lato').get('[data-testid="catalog-install-open"]').trigger('click');
    await flushPromises();
    const s = w.get('[data-testid="catalog-install-status"]');
    expect(s.attributes('role')).toBe('alert');
    expect(s.text()).toBe(
      'Lato was not installed. Fonts cannot be installed into a system font folder. Choose another folder under Settings → Online catalogs.',
    );
  });
});
