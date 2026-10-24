import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockSources } from '../api/mock';
import type { DownloadFamilyResponse } from '../api/types';
import { FaceLoader, setFaceLoader } from '../preview/faceLoader';
import { useCatalogsStore } from '../stores/catalogs';
import CatalogBrowser from './CatalogBrowser.vue';

let wrappers: VueWrapper[] = [];

beforeEach(() => {
  setActivePinia(createPinia());
  setFaceLoader(
    new FaceLoader({
      fonts: { add: () => undefined, delete: () => true },
      create: () => ({ load: () => Promise.resolve() }),
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

/** The browser on Google Fonts with its cached list, over `client`. */
async function opened(client: FmClient = createMockClient({ catalog: 'offline' })) {
  setClient(client);
  const store = useCatalogsStore();
  await store.load();
  store.show('googleFonts');
  const w = mount(CatalogBrowser, { attachTo: document.body });
  wrappers.push(w);
  await flushPromises();
  return { w, store, client };
}

const row = (w: VueWrapper, family: string) =>
  w.get(`[data-testid="catalog-family"][data-family="${family}"]`);
const panel = (w: VueWrapper) => w.find('[data-testid="catalog-download"]');
const status = (w: VueWrapper) => w.get('[data-testid="catalog-download-status"]').text();

async function openFor(w: VueWrapper, family: string) {
  await row(w, family).get('[data-testid="catalog-download-open"]').trigger('click');
  await flushPromises();
}

describe('CatalogBrowser "Download..."', () => {
  it('every row has a labelled "Download..." button that opens the panel on the source picker', async () => {
    const { w } = await opened();
    const button = row(w, 'Lato').get('[data-testid="catalog-download-open"]');
    expect(button.element.tagName).toBe('BUTTON');
    expect(button.text()).toBe('Download…');
    expect(button.attributes('aria-label')).toBe('Download Lato…');
    expect(panel(w).exists()).toBe(false);

    await openFor(w, 'Lato');
    const section = panel(w);
    expect(section.attributes('aria-labelledby')).toBe('catalog-download-title');
    expect(section.get('#catalog-download-title').text()).toBe('Download Lato');
    const select = section.get('[data-testid="catalog-download-source"]');
    expect(document.activeElement, 'the source picker takes the focus').toBe(select.element);
    expect(section.get('label.field').text()).toContain('Download into');
    // Only folders the user added, never the system fonts.
    const options = select.findAll('option').map((o) => o.text());
    expect(options).toEqual([mockSources[0]!.path]);
    expect(section.text()).toContain('The files go into a folder named Lato inside it.');
    expect(section.find('fieldset').exists(), 'Lato has no variable font').toBe(false);
  });

  it('downloads into the chosen folder, follows the rescan and shows the result', async () => {
    const { w, client } = await opened();
    await openFor(w, 'Lato');
    await w.get('form').trigger('submit');
    await flushPromises();
    await new Promise((r) => setTimeout(r, 5));
    await flushPromises();

    expect(status(w)).toBe('Lato: 5 files downloaded into /Users/demo/Fonts/Lato.');
    const close = w.get('[data-testid="catalog-download"] .buttons button');
    expect(close.text()).toBe('Close');
    expect(document.activeElement, 'the result takes the focus').toBe(close.element);
    const downloads = await client.listCatalogDownloads();
    expect(downloads.map((d) => [d.family, d.source, d.kind])).toEqual([['Lato', 1, 'static']]);

    await close.trigger('click');
    await flushPromises();
    expect(panel(w).exists()).toBe(false);
    expect(document.activeElement).toBe(w.get('[data-testid="catalog-list"]').element);
  });

  it('a family with axes asks static or variable, variable first', async () => {
    const { w, client } = await opened();
    await openFor(w, 'Inter');
    const fieldset = w.get('[data-testid="catalog-download"] fieldset');
    expect(fieldset.get('legend').text()).toBe('Files');
    const radios = fieldset.findAll<HTMLInputElement>('input[type="radio"]');
    expect(radios.map((r) => r.element.parentElement?.textContent?.trim())).toEqual([
      'Variable font',
      'One file per style',
    ]);
    expect(radios[0]!.element.checked).toBe(true);
    await radios[1]!.setValue(true);
    await w.get('form').trigger('submit');
    await flushPromises();
    const [inter] = await client.listCatalogDownloads();
    expect(inter?.kind).toBe('static');
    expect(inter?.files.map((f) => f.name)).toEqual([
      'Inter-100.ttf',
      'Inter-400.ttf',
      'Inter-900.ttf',
    ]);
  });

  it('shows the download while it runs and keeps the panel open until it ends', async () => {
    const base = createMockClient({ catalog: 'offline' });
    let release!: () => void;
    const client: FmClient = {
      ...base,
      downloadFamily: (req) =>
        new Promise<DownloadFamilyResponse>((resolve, reject) => {
          release = () => base.downloadFamily(req).then(resolve, reject);
        }),
    };
    const { w, store } = await opened(client);
    await openFor(w, 'Lora');
    await w.get('form').trigger('submit');
    await flushPromises();

    expect(status(w)).toBe('Downloading Lora…');
    expect(w.find('[data-testid="catalog-download"] progress').exists()).toBe(true);
    expect(w.find('[data-testid="catalog-download"] form').exists()).toBe(false);
    expect(
      row(w, 'Lato').get('[data-testid="catalog-download-open"]').attributes('disabled'),
      'one download at a time',
    ).toBeDefined();
    await w.get('[data-testid="catalog-download"]').trigger('keydown', { key: 'Escape' });
    store.closeDownload();
    await flushPromises();
    expect(panel(w).exists(), 'a running download keeps its panel').toBe(true);

    release();
    await flushPromises();
    await new Promise((r) => setTimeout(r, 5));
    await flushPromises();
    expect(status(w)).toBe('Lora: 2 files downloaded into /Users/demo/Fonts/Lora.');
    expect(w.find('[data-testid="catalog-download"] progress').exists()).toBe(false);
  });

  it('a download that stopped after some files says which were written, never "No file was written"', async () => {
    const { w, client } = await opened(
      createMockClient({ catalog: 'offline', catalogDownload: 'stopped' }),
    );
    await openFor(w, 'Lato');
    await w.get('form').trigger('submit');
    await flushPromises();
    await new Promise((r) => setTimeout(r, 5));
    await flushPromises();

    expect(status(w)).toBe('Lato: 1 file downloaded into /Users/demo/Fonts/Lato.');
    const alert = w.get('[data-testid="catalog-download-error"]');
    expect(alert.attributes('role')).toBe('alert');
    expect(alert.text()).toBe(
      'The download stopped before the end: Lato could not be downloaded. The downloaded fonts could not be saved. Only the files counted above were written, and they are kept.',
    );
    expect(alert.text()).not.toContain('No file was written');
    expect(
      w.find('[data-testid="catalog-download-start"]').exists(),
      'no retry over kept files',
    ).toBe(false);
    const [lato] = await client.listCatalogDownloads();
    expect(lato?.files.map((f) => f.name)).toEqual(['Lato-100.ttf']);
  });

  it('a failed download says nothing was written and offers to try again', async () => {
    const { w, client } = await opened(
      createMockClient({ catalog: 'offline', catalogDownload: 'fail' }),
    );
    await openFor(w, 'Lato');
    await w.get('form').trigger('submit');
    await flushPromises();
    const alert = w.get('[data-testid="catalog-download-error"]');
    expect(alert.attributes('role')).toBe('alert');
    expect(alert.text()).toBe(
      'Lato could not be downloaded. The catalog could not be reached. Check the connection and try again. No file was written.',
    );
    expect(w.get('[data-testid="catalog-download-start"]').text()).toBe('Try again');
    expect(await client.listCatalogDownloads()).toEqual([]);
  });

  it('Escape and Cancel close the panel and give the focus back to the list', async () => {
    const { w } = await opened();
    await openFor(w, 'Lato');
    await w.get('[data-testid="catalog-download-source"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(panel(w).exists()).toBe(false);
    expect(document.activeElement).toBe(w.get('[data-testid="catalog-list"]').element);

    await openFor(w, 'Lato');
    const cancel = w
      .findAll('[data-testid="catalog-download"] button')
      .find((b) => b.text() === 'Cancel')!;
    await cancel.trigger('click');
    await flushPromises();
    expect(panel(w).exists()).toBe(false);
  });

  it('without a folder of the user it asks to add one and offers no download', async () => {
    const base = createMockClient({ catalog: 'offline' });
    const client: FmClient = {
      ...base,
      listSources: async () => [mockSources[1]!],
    };
    const { w } = await opened(client);
    await openFor(w, 'Lato');
    expect(w.get('[data-testid="catalog-download-no-folder"]').text()).toBe(
      'Add a folder to the library first. Downloads go into a folder you added.',
    );
    expect(w.find('[data-testid="catalog-download-start"]').exists()).toBe(false);
  });
});
