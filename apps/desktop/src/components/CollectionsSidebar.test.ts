import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { FONTS_MIME } from '../list/drag';
import { useCollectionsStore } from '../stores/collections';
import { useCommandsStore } from '../stores/commands';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import CollectionsSidebar from './CollectionsSidebar.vue';

let wrapper: VueWrapper | undefined;
let client: FmClient;

/** A `DataTransfer` stand-in for jsdom, which has none. */
function transfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    effectAllowed: 'none',
    dropEffect: 'none',
    get types() {
      return [...data.keys()];
    },
    setData: (type: string, value: string) => void data.set(type, value),
    getData: (type: string) => data.get(type) ?? '',
  } as unknown as DataTransfer;
}

async function mountSidebar() {
  const pinia = createPinia();
  setActivePinia(pinia);
  client = createMockClient();
  setClient(client);
  await useLibraryStore().load();
  for (const name of ['Clients', 'Brand']) await client.createCollection({ name });
  wrapper = mount(CollectionsSidebar, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

const item = (w: VueWrapper, name: string) => w.get(`li[aria-label="${name}"]`);
const count = (w: VueWrapper, name: string) => item(w, name).get('.count').attributes('aria-label');
const names = () => useCollectionsStore().rows.map((r) => `${r.depth}:${r.collection.name}`);

beforeEach(() => localStorage.clear());
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('collections sidebar', () => {
  it('lists the collections with their counts, and makes one from the New button', async () => {
    const w = await mountSidebar();
    expect(count(w, 'Clients')).toBe('0 fonts');
    await w.get('button[title="Make a collection"]').trigger('click');
    await flushPromises();
    const input = w.get('input[aria-label="New collection name"]');
    expect(document.activeElement).toBe(input.element);
    await input.setValue('  Display  ');
    await w.get('form:has(input[aria-label="New collection name"])').trigger('submit');
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '0:Brand', '0:Display']);
    expect(w.find('input[aria-label="New collection name"]').exists()).toBe(false);
  });

  it('renames, nests, reorders and deletes with the row buttons', async () => {
    const w = await mountSidebar();
    await w.get('button[aria-label="Rename Brand"]').trigger('click');
    await flushPromises();
    await w.get('input[aria-label="New name for Brand"]').setValue('Marque');
    await w.get('li[aria-label="Brand"] form').trigger('submit');
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '0:Marque']);

    await w.get('button[aria-label="New collection inside Clients"]').trigger('click');
    await flushPromises();
    await w.get('input[aria-label="New collection name inside Clients"]').setValue('Acme');
    await w
      .get('form:has(input[aria-label="New collection name inside Clients"])')
      .trigger('submit');
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '1:Acme', '0:Marque']);

    await w.get('button[aria-label="Move Marque up"]').trigger('click');
    await flushPromises();
    expect(names()).toEqual(['0:Marque', '0:Clients', '1:Acme']);
    expect(w.get('button[aria-label="Move Marque up"]').attributes('disabled')).toBeDefined();

    await w.get('button[aria-label="Move Acme out of its parent"]').trigger('click');
    await flushPromises();
    expect(names()).toEqual(['0:Marque', '0:Clients', '0:Acme']);

    await w.get('button[aria-label="Delete Clients"]').trigger('click');
    await flushPromises();
    expect(names()).toEqual(['0:Marque', '0:Acme']);
  });

  it('nests an existing collection with the In button, no drag needed', async () => {
    const w = await mountSidebar();
    // The first collection has no sibling above it to go into.
    expect(w.find('button[aria-label="Move Clients into the collection above"]').exists()).toBe(
      false,
    );
    await w.get('button[aria-label="Move Brand into the collection above"]').trigger('click');
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '1:Brand']);
    // In and Out undo each other.
    await w.get('button[aria-label="Move Brand out of its parent"]').trigger('click');
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '0:Brand']);
  });

  it('nests or reorders a collection dragged onto another', async () => {
    const w = await mountSidebar();
    const dataTransfer = transfer();
    // The middle of a row: inside it. jsdom boxes are 0 high, which reads as the middle.
    await item(w, 'Brand').trigger('dragstart', { dataTransfer });
    await item(w, 'Clients').trigger('dragover', { dataTransfer });
    await item(w, 'Clients').trigger('drop', { dataTransfer });
    await flushPromises();
    expect(names()).toEqual(['0:Clients', '1:Brand']);

    // The top edge: before it, at its level.
    const box = { top: 0, height: 20, bottom: 20, left: 0, right: 100, width: 100 } as DOMRect;
    item(w, 'Clients').element.getBoundingClientRect = () => box;
    await item(w, 'Brand').trigger('dragstart', { dataTransfer });
    await item(w, 'Clients').trigger('dragover', { dataTransfer, clientY: 2 });
    expect(item(w, 'Clients').classes()).toContain('drop-before');
    await item(w, 'Clients').trigger('drop', { dataTransfer });
    await flushPromises();
    expect(names()).toEqual(['0:Brand', '0:Clients']);
  });

  it('adds fonts dropped from the list, and counts them', async () => {
    const w = await mountSidebar();
    const dataTransfer = transfer();
    dataTransfer.setData(FONTS_MIME, JSON.stringify({ faces: [6], families: [1] }));
    await item(w, 'Brand').trigger('dragover', { dataTransfer });
    await item(w, 'Brand').trigger('drop', { dataTransfer });
    await flushPromises();
    expect(count(w, 'Brand')).toBe('2 fonts');
    expect(count(w, 'Clients')).toBe('0 fonts');
  });

  it('adds the selected fonts from the palette, one command per collection', async () => {
    const w = await mountSidebar();
    const commands = useCommandsStore();
    const list = useFontListStore();
    const id = 'collections.add.' + useCollectionsStore().collections[0]!.id;
    expect(commands.items.find((i) => i.action.id === id)?.action.title).toBe(
      'Add selected fonts to “Clients”',
    );
    expect(commands.run(id)).toBe(false); // nothing selected yet
    list.click('family:3');
    list.click('family:4', 'toggle');
    expect(commands.run(id)).toBe(true);
    await flushPromises();
    expect(count(w, 'Clients')).toBe('2 fonts');
  });
});
