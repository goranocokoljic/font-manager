import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { FaceLoader, catalogFamily, setFaceLoader } from '../preview/faceLoader';
import { CATALOG_VIEW, useCatalogsStore } from '../stores/catalogs';
import { usePreviewStore } from '../stores/preview';
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
});
