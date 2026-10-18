import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useSelectionStore } from './selection';
import { useTagsStore } from './tags';

let client: FmClient;

async function setup() {
  client = createMockClient({ faceTags: { 6: ['Brand'] } });
  setClient(client);
  await useLibraryStore().load();
  const tags = useTagsStore();
  await tags.refresh();
  const selection = useSelectionStore();
  await selection.select(1);
  return { tags, selection };
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('tags, favorite and rating', () => {
  it('tags the selected face, reuses a name in another case, and drops an unused tag', async () => {
    const { tags, selection } = await setup();
    expect(tags.tags.map((t) => t.name)).toEqual(['Brand']);
    expect(await selection.addTag('  display ')).toBe(true);
    expect(await selection.addTag('BRAND')).toBe(true);
    expect(selection.detail!.tags).toEqual(['display', 'Brand']);
    expect(tags.tags.map((t) => [t.name, t.faceCount])).toEqual([
      ['Brand', 2],
      ['display', 1],
    ]);
    expect(tags.nameOf(tags.idOf('Display')!)).toBe('display');

    expect(await selection.removeTag('display')).toBe(true);
    expect(selection.detail!.tags).toEqual(['Brand']);
    expect(tags.tags.map((t) => t.name)).toEqual(['Brand']);
  });

  it('sets and clears favorite and rating, and shows a refusal', async () => {
    const { selection } = await setup();
    await selection.setFavorite(true);
    await selection.setRating(4);
    expect([selection.detail!.favorite, selection.detail!.rating]).toEqual([true, 4]);
    await selection.setRating(0);
    await selection.setFavorite(false);
    expect([selection.detail!.favorite, selection.detail!.rating]).toEqual([false, null]);
    expect(await selection.addTag('   ')).toBe(false);
    expect(selection.editError?.code).toBe('invalidRequest');
    await selection.setRating(2);
    expect(selection.editError).toBeNull();
  });

  it('the list filters by tag, favorite and rating once they change', async () => {
    const { tags, selection } = await setup();
    const search = useSearchStore();
    await selection.addTag('display');
    await selection.setFavorite(true);
    await selection.setRating(5);
    await flushPromises();
    const display = tags.idOf('display')!;
    expect(search.counts.tags).toContainEqual({ value: display, count: 1 });
    expect(search.counts.favorite).toContainEqual({ value: true, count: 1 });
    expect(search.counts.minRating).toContainEqual({ value: 5, count: 1 });
    for (const filters of [{ tags: [display] }, { favorite: true }, { minRating: 4 }]) {
      search.setFilters(filters);
      await flushPromises();
      expect(search.hits?.map((h) => h.family.id)).toEqual([1]);
    }
  });
});
