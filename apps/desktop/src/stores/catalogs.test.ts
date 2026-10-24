import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useCatalogsStore } from './catalogs';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
});

describe('catalogs store', () => {
  it('shows and closes a catalog', () => {
    const store = useCatalogsStore();
    expect(store.open).toBeNull();
    store.show('adobeFonts');
    expect(store.open).toBe('adobeFonts');
    store.close();
    expect(store.open).toBeNull();
  });

  it('opening a catalog ends the Quick Collection review, and the review closes it', async () => {
    const store = useCatalogsStore();
    const quick = useQuickCollectionStore();
    quick.show();
    store.show('adobeFonts');
    expect(quick.open).toBe(false);
    quick.show();
    await flushPromises();
    expect(store.open).toBeNull();
  });

  it('opening a collection or All fonts closes the catalog', async () => {
    const store = useCatalogsStore();
    store.show('adobeFonts');
    useSearchStore().setFilters({ collection: null });
    await flushPromises();
    expect(store.open).toBeNull();
  });
});
