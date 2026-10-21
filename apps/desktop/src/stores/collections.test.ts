import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockPickedSaves, mockSavedInventories } from '../api/mock';
import { useCollectionsStore } from './collections';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useToastStore } from './toast';

let client: FmClient;

async function setup() {
  client = createMockClient({ extraFaces: 1, stylesPerFamily: 3 });
  setClient(client);
  await useLibraryStore().load();
  const store = useCollectionsStore();
  await store.refresh();
  await flushPromises();
  return store;
}

/** The sidebar tree as `name` indented by depth, with its count. */
const tree = (store: ReturnType<typeof useCollectionsStore>) =>
  store.rows.map((r) => `${'  '.repeat(r.depth)}${r.collection.name} ${r.collection.faceCount}`);

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('collections store', () => {
  it('creates, renames, nests, reorders and deletes collections', async () => {
    const store = await setup();
    expect(await store.create('Clients')).toBe(true);
    await store.create('Brand');
    const clients = store.collections[0]!.id;
    const brand = store.collections[1]!.id;
    await store.create('Acme', clients);
    expect(tree(store)).toEqual(['Clients 0', '  Acme 0', 'Brand 0']);
    expect(store.rows.map((r) => [r.position, r.siblings])).toEqual([
      [0, 2],
      [0, 1],
      [1, 2],
    ]);

    expect(await store.rename(brand, 'Marque')).toBe(true);
    await store.move(brand, null, 0);
    expect(tree(store)).toEqual(['Marque 0', 'Clients 0', '  Acme 0']);
    await store.move(brand, clients, 1);
    expect(tree(store)).toEqual(['Clients 0', '  Acme 0', '  Marque 0']);

    // The API's refusals show as the store's error and change nothing.
    expect(await store.create('Acme', clients)).toBe(false);
    expect(store.error?.code).toBe('conflict');
    await store.move(clients, brand, 0);
    expect(store.error?.code).toBe('invalidRequest');
    expect(tree(store)).toEqual(['Clients 0', '  Acme 0', '  Marque 0']);

    await store.remove(clients);
    expect(store.rows).toEqual([]);
    expect(store.error).toBeNull();
  });

  it('adds faces and whole families, a face to many, with the counts from the API', async () => {
    const store = await setup();
    await store.create('A');
    await store.create('B');
    const [a, b] = store.collections.map((c) => c.id);
    const extra = useLibraryStore().families.find((f) => f.name.startsWith('Sample'))!;
    await store.add(a!, { faces: [1], families: [extra.id] });
    await store.add(b!, { faces: [1], families: [] });
    expect(tree(store)).toEqual(['A 4', 'B 1']); // three styles of the family and face 1
    await store.add(a!, { faces: [], families: [] }); // nothing to add: no request
    expect(tree(store)).toEqual(['A 4', 'B 1']);
  });

  it('shows a collection in the list and goes back to all fonts when it is deleted', async () => {
    const store = await setup();
    const search = useSearchStore();
    await store.create('Mono');
    const mono = store.collections[0]!.id;
    await store.add(mono, { faces: [6], families: [] });
    store.show(mono);
    await flushPromises();
    expect(store.open?.name).toBe('Mono');
    expect(search.hits?.map((h) => h.family.name)).toEqual(['JetBrains Mono']);

    // Adding to the open collection shows the new fonts at once.
    await store.add(mono, { faces: [7], families: [] });
    await flushPromises();
    expect(search.hits?.map((h) => h.family.name).sort()).toEqual([
      'JetBrains Mono',
      'Legacy Grotesk',
    ]);

    await store.remove(mono);
    await flushPromises();
    expect(store.openId).toBeNull();
    expect(search.hits).toBeNull();
  });

  it('exports the license inventory of a collection or the library to the picked file', async () => {
    const store = await setup();
    const toast = useToastStore();
    mockSavedInventories.length = 0;
    mockPickedSaves.length = 0;
    await store.create('Client: A/B');
    const id = store.collections[0]!.id;

    await store.exportLicenseInventory(id);
    expect(mockSavedInventories.at(-1)).toEqual({
      path: '/Users/demo/Documents/Client- A-B license inventory.csv',
      collection: id,
      rows: 0,
    });
    expect(toast.toast?.message).toBe('Saved the license inventory: 0 families.');

    await store.exportLicenseInventory(null);
    const all = mockSavedInventories.at(-1)!;
    expect(all.path).toBe('/Users/demo/Documents/License inventory.csv');
    expect(all.collection).toBeNull();
    expect(all.rows).toBeGreaterThan(1);
    expect(toast.toast?.message).toBe(`Saved the license inventory: ${all.rows} families.`);

    // A cancelled dialog saves nothing and says nothing.
    toast.dismiss();
    mockPickedSaves.push(null);
    await store.exportLicenseInventory(id);
    expect(mockSavedInventories).toHaveLength(2);
    expect(toast.toast).toBeNull();

    // A refusal from the API is shown.
    await store.exportLicenseInventory(9999);
    expect(toast.toast?.message).toBe('Collection 9999 was not found.');
    expect(mockSavedInventories).toHaveLength(2);
  });
});
