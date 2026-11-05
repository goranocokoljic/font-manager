import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from '../stores/library';
import { useSearchStore } from '../stores/search';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { useViewsStore } from '../stores/views';
import WallCountLine from './WallCountLine.vue';

let wrapper: VueWrapper | undefined;

/** The line over a library whose search finds `total` families and knows Serbian (Latin). */
async function mountLine(total = 212) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const client = createMockClient();
  const search = client.search.bind(client);
  client.search = async (req) => ({ ...(await search(req)), total });
  client.listLanguages = async () => [
    { code: 'sr-Latn', name: 'Serbian (Latin)', script: 'Latn', pangram: null, sampleGlyphs: null },
  ];
  setClient(client);
  await useLibraryStore().load();
  const store = useSearchStore();
  await store.loadLanguages();
  wrapper = mount(WallCountLine, {
    props: { view: ALL_FONTS_VIEW },
    global: { plugins: [pinia] },
    attachTo: document.body,
  });
  await flushPromises();
  return { w: wrapper, store };
}

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('the count line above the wall', () => {
  it('reads the count, the filters and the sort', async () => {
    const { w, store } = await mountLine();
    store.setFilters({
      classification: ['sans'],
      supports: { kind: 'languages', codes: ['sr-Latn'] },
    });
    await flushPromises();
    expect(w.get('[role="status"]').text()).toBe('212 families · Sans serif · Serbian (Latin)');
    expect(w.get('.sort').text().replace(/\s+/g, ' ')).toBe('Sorted by name · change');
  });

  it('has no separators and no filter names with no filter', async () => {
    const { w } = await mountLine();
    expect(w.get('[role="status"]').text()).not.toContain('·');
    expect(w.get('[role="status"]').text()).toMatch(/famil(y|ies)$/);
  });

  it('follows the view sort, and names a text search best match until one is chosen', async () => {
    const { w, store } = await mountLine();
    const sort = () => w.get('.sort span').text();
    store.setText('sample');
    store.query = 'sample';
    await flushPromises();
    expect(sort()).toBe('Sorted by best match');
    useViewsStore().setSort(ALL_FONTS_VIEW, 'added', true);
    await flushPromises();
    expect(sort()).toBe('Sorted by date added, descending');
  });

  it('offers each API key once, Style and Weight being one', async () => {
    const { w } = await mountLine();
    await w.get('button.change').trigger('click');
    await flushPromises();
    const rows = w.findAll('.menu label').map((l) => l.text());
    expect(rows.filter((r) => r === 'Weight' || r === 'Style')).toHaveLength(1);
    expect(rows).toContain('Name');
    expect(rows).not.toContain('Notes');
  });

  it('marks the Weight key current for a table sorted by the Style column', async () => {
    const { w } = await mountLine();
    useViewsStore().sortBy(ALL_FONTS_VIEW, 'weightLine');
    await w.get('button.change').trigger('click');
    await flushPromises();
    const checked = w.findAll('.menu input[type="radio"]').filter((r) => {
      return (r.element as HTMLInputElement).checked;
    });
    expect(checked).toHaveLength(1);
    expect(checked[0]!.element.closest('label')!.textContent!.trim()).toBe('Weight');
  });

  it('closes on Escape and gives focus back to change', async () => {
    const { w } = await mountLine();
    await w.get('button.change').trigger('click');
    await flushPromises();
    await w.get('.menu input').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(w.find('.menu').exists()).toBe(false);
    expect(document.activeElement).toBe(w.get('button.change').element);
  });
});
