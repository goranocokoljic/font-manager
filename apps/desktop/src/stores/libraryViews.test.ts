import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useLibraryStore } from './library';
import { RECENT_DAYS, useLibraryViewsStore } from './libraryViews';
import { useSearchStore } from './search';

async function setUp(favoriteFaces: number[] = [1]) {
  setActivePinia(createPinia());
  const client = createMockClient({ favoriteFaces, activeFaces: [1, 6] });
  setClient(client);
  await useLibraryStore().load();
  const store = useLibraryViewsStore();
  await store.refresh();
  await flushPromises();
  return { client, store, search: useSearchStore() };
}

beforeEach(() => localStorage.clear());
afterEach(() => setClient(undefined));

describe('the library views', () => {
  it('counts each view with what the API returns for its filters', async () => {
    const { client, store } = await setUp();
    const page = { offset: 0, limit: 1 };
    const total = async (filters: object) => (await client.search({ filters, page })).total;
    expect(store.counts).toEqual({
      all: await total({}),
      recent: await total({ addedWithinDays: RECENT_DAYS }),
      activated: await total({ active: true }),
      favorites: await total({ favorite: true }),
    });
    expect(store.counts.favorites).toBe(1);
    expect(store.counts.activated).toBe(2);
    expect(store.counts.all).toBeGreaterThan(2);
  });

  it('shows a view by its filters and keeps the filters chosen on top', async () => {
    const { store, search } = await setUp();
    search.setFilters({ variable: true, collection: 7 });
    expect(store.current).toBeNull();
    store.show('favorites');
    expect(search.filters).toMatchObject({ variable: true, favorite: true, collection: null });
    expect(store.current).toBe('favorites');
    expect(store.currentLabel).toBe('Favorites');

    // Moving on takes the last view's filter away.
    store.show('activated');
    expect(search.filters).toMatchObject({ variable: true, active: true, favorite: null });
    expect(store.current).toBe('activated');
    store.show('recent');
    expect(search.filters).toMatchObject({ addedWithinDays: RECENT_DAYS, active: null });
    expect(store.current).toBe('recent');
    store.show('all');
    expect(search.filters).toMatchObject({ variable: true, addedWithinDays: null });
    expect(store.current).toBe('all');
    expect(store.currentLabel).toBe('All fonts');
  });

  it('a value the view does not set is not that view', async () => {
    const { store, search } = await setUp();
    search.setFilters({ active: false, favorite: false, addedWithinDays: 7 });
    expect(store.current).toBe('all');
  });

  it('keeps the counts when a refresh fails', async () => {
    const { client, store } = await setUp();
    const before = { ...store.counts };
    setClient({ ...client, search: () => Promise.reject(new Error('no')) });
    await store.refresh();
    expect(store.counts).toEqual(before);
  });
});
