import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';

/** The online catalogs the sidebar lists. Adobe Fonts is a note only (E22.6). */
export type CatalogId = 'adobeFonts';

/**
 * Which online catalog the center shows instead of the font list. Opening one
 * reads nothing from the API: the Adobe Fonts entry is a note and a link.
 */
export const useCatalogsStore = defineStore('catalogs', () => {
  const open = ref<CatalogId | null>(null);
  const quick = useQuickCollectionStore();

  function show(id: CatalogId): void {
    quick.close();
    open.value = id;
  }

  function close(): void {
    open.value = null;
  }

  // The Quick Collection review, a collection or All fonts takes the center back.
  watch(
    () => quick.open,
    (reviewing) => reviewing && close(),
  );
  watch(
    () => useSearchStore().filters,
    () => close(),
  );

  return { open, show, close };
});
