import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useCollectionsStore } from '../stores/collections';
import { useLibraryStore } from '../stores/library';
import { useSearchStore } from '../stores/search';
import CollectionHeader from './CollectionHeader.vue';

let wrapper: VueWrapper | undefined;
let client: FmClient;

/** The strip's host with `count` families in one collection, which is `color`. */
async function setup(count: number, color?: string) {
  const pinia = createPinia();
  setActivePinia(pinia);
  client = createMockClient({ extraFaces: 8 });
  setClient(client);
  const library = useLibraryStore();
  await library.load();
  const families = library.families.slice(0, count);
  const made = await client.createCollection({ name: 'Brand', color });
  if (families.length)
    await client.addToCollection({
      collection: made.id,
      faces: [],
      families: families.map((f) => f.id),
    });
  const collections = useCollectionsStore();
  await collections.refresh();
  wrapper = mount(CollectionHeader, { global: { plugins: [pinia] } });
  await flushPromises();
  return { collections, id: made.id, families };
}

const strip = () => wrapper!.find('[data-testid="collection-strip"]');

afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('CollectionHeader', () => {
  it('is not there under All fonts', async () => {
    await setup(2, 'teal');
    expect(strip().exists()).toBe(false);
  });

  it('shows the open collection in its colour, its names in their own faces', async () => {
    const { collections, id, families } = await setup(2, 'teal');
    collections.show(id);
    await useSearchStore().run();
    await flushPromises();
    const el = strip().element as HTMLElement;
    expect(el.classList.contains('tinted')).toBe(true);
    expect(el.style.getPropertyValue('--strip-color')).toBe('var(--fm-collection-teal)');
    expect(el.getAttribute('aria-label')).toBe('Fonts in Brand');
    const items = strip().findAll('[data-testid="card-live-text"]');
    expect(items.map((i) => i.text())).toEqual(families.map((f) => f.name));
    for (const item of items)
      expect((item.element as HTMLElement).style.fontFamily).toContain('fm-face-');
  });

  it('has no tint for a collection without a colour', async () => {
    const { collections, id } = await setup(1);
    collections.show(id);
    await useSearchStore().run();
    await flushPromises();
    const el = strip().element as HTMLElement;
    expect(el.classList.contains('tinted')).toBe(false);
    expect(el.style.getPropertyValue('--strip-color')).toBe('');
  });

  it('shows at most six families', async () => {
    const { collections, id, families } = await setup(8, 'coral');
    expect(families.length).toBeGreaterThan(6);
    collections.show(id);
    await useSearchStore().run();
    await flushPromises();
    expect(strip().findAll('li')).toHaveLength(6);
  });

  it('is not there for an empty collection', async () => {
    const { collections, id } = await setup(0, 'coral');
    collections.show(id);
    await useSearchStore().run();
    await flushPromises();
    expect(strip().exists()).toBe(false);
  });
});
