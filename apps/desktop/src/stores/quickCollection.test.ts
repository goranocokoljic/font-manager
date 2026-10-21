import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useCollectionsStore } from './collections';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';

let client: FmClient;

async function setup() {
  client = createMockClient({ extraFaces: 1, stylesPerFamily: 3 });
  setClient(client);
  await useLibraryStore().load();
  const store = useQuickCollectionStore();
  await store.load();
  await flushPromises();
  return store;
}

const familyRow = (name: string) => {
  const row = useFontListStore().rows.find((r) => r.kind === 'family' && r.family.name === name);
  if (!row) throw new Error(`no row ${name}`);
  return row;
};

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('quick collection store', () => {
  it('one toggle adds the selected rows, the next removes them', async () => {
    const store = await setup();
    const inter = familyRow('Inter');
    const sample = familyRow('Sample 1');
    await store.toggleRows([inter]);
    expect(store.count).toBe(1);
    expect(store.holds(inter)).toBe(true);
    expect(store.holds(sample)).toBe(false);

    // A family row brings every face of the family; what is in already stays once.
    await store.toggleRows([inter, sample]);
    expect(store.count).toBe(4);
    expect((await client.quickCollection()).faces.map((f) => f.familyName)).toEqual([
      'Inter',
      'Sample 1',
      'Sample 1',
      'Sample 1',
    ]);

    // Everything picked is in: the toggle takes it out.
    await store.toggleRows([sample]);
    expect(store.faces.map((f) => f.familyName)).toEqual(['Inter']);
    expect(store.holds(sample)).toBe(false);
  });

  it('drops and clears without touching the library', async () => {
    const store = await setup();
    await store.toggleRows([familyRow('Inter'), familyRow('JetBrains Mono')]);
    const [first, second] = store.faces;
    await store.drop(first!.id);
    expect(store.faces.map((f) => f.id)).toEqual([second!.id]);
    await store.clear();
    expect(store.count).toBe(0);
    expect((await client.quickCollection()).faces).toEqual([]);
    expect(useLibraryStore().families.some((f) => f.name === 'Inter')).toBe(true);
  });

  it('saves the kept fonts as a manual collection and tags them', async () => {
    const store = await setup();
    await store.toggleRows([familyRow('Inter'), familyRow('Sample 1')]);
    const kept = store.faces.filter((f) => f.familyName === 'Sample 1').map((f) => f.id);
    await store.drop(store.faces.find((f) => f.familyName === 'Inter')!.id);

    expect(await store.saveAsCollection('  Keepers ')).toBe(true);
    const collections = useCollectionsStore();
    const made = collections.collections.find((c) => c.name === 'Keepers');
    expect(made?.rules ?? null).toBeNull();
    expect(made?.faceCount).toBe(kept.length);
    const hits = await client.search({
      query: '',
      filters: { collection: made!.id },
      page: { offset: 0, limit: 50 },
    });
    expect(hits.families.flatMap((h) => h.faces).sort()).toEqual([...kept].sort());

    expect(await store.tagAll('shortlist')).toBe(true);
    for (const id of kept) expect((await client.getFace(id)).tags).toEqual(['shortlist']);
    const inter = (await client.getFamily(familyRow('Inter').family.id)).faces[0]!;
    expect((await client.getFace(inter.id)).tags).toEqual([]);
  });

  it('opening a collection or All fonts ends the review', async () => {
    const store = await setup();
    store.show();
    expect(store.open).toBe(true);
    useSearchStore().setFilters({ collection: null });
    await nextTick();
    expect(store.open).toBe(false);
  });
});
