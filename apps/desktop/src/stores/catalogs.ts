import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Catalog, CatalogFamily, CatalogId } from '../api/types';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';

/**
 * The entries of the sidebar's "Online catalogs" group: each catalog of `list_catalogs`
 * (E22.2), and Adobe Fonts, a note only (E22.6).
 */
export type CatalogEntry = CatalogId | 'adobeFonts';

/** The preview-settings and view-mode id the catalog browser uses. */
export const CATALOG_VIEW = 'catalog';

/** Families one `list_catalog_families` call reads (the API caps a page at 500). */
export const CATALOG_PAGE = 200;

function toApiError(e: unknown): ApiError {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) return e as ApiError;
  return { code: 'internal', message: String(e), detail: null };
}

/**
 * The online catalogs: which are on and when their list was refreshed, which one the
 * center shows instead of the font list, and that catalog's families for the search
 * text. The families are read page by page as the list scrolls; the core filters them
 * (`list_catalog_families`). Only `refresh` and turning a never-refreshed catalog on go
 * to the network, both a user action.
 */
export const useCatalogsStore = defineStore('catalogs', () => {
  const client = () => useClient();
  const quick = useQuickCollectionStore();

  const catalogs = ref<Catalog[]>([]);
  const open = ref<CatalogEntry | null>(null);
  const query = ref('');
  /** Families matching `query`, by index; a page arrives when the list reaches it. */
  const families = shallowRef<(CatalogFamily | undefined)[]>([]);
  const total = ref(0);
  const listed = ref(false);
  const busy = ref<CatalogId | null>(null);
  const refreshing = ref(false);
  /** The last failed refresh of the open catalog; its cached list stays. */
  const refreshError = ref<ApiError | null>(null);
  /** The last failure of anything else (the list, a switch). */
  const error = ref<ApiError | null>(null);
  /** The webview's own idea of the connection; `false` shows the offline note. */
  const online = ref(globalThis.navigator?.onLine ?? true);
  /** Bumped by every new list, so pages of an older one are dropped. */
  let generation = 0;
  const pending = new Set<number>();

  if (typeof window !== 'undefined') {
    window.addEventListener('online', () => (online.value = true));
    window.addEventListener('offline', () => (online.value = false));
  }

  const current = computed<Catalog | null>(
    () => catalogs.value.find((c) => c.id === open.value) ?? null,
  );
  /** The cached list is shown though it could not be brought up to date. */
  const offline = computed(
    () => !!current.value?.refreshedAt && (!online.value || refreshError.value?.code === 'io'),
  );

  function put(catalog: Catalog): void {
    const i = catalogs.value.findIndex((c) => c.id === catalog.id);
    catalogs.value =
      i < 0 ? [...catalogs.value, catalog] : catalogs.value.map((c, j) => (j === i ? catalog : c));
  }

  async function load(): Promise<void> {
    try {
      catalogs.value = await client().listCatalogs();
      error.value = null;
      // A catalog opened before the list arrived reads its families now.
      if (current.value && !listed.value) await relist();
    } catch (e) {
      error.value = toApiError(e);
    }
  }

  /** Reads the page holding family `index` of the open catalog. */
  async function readPage(index: number): Promise<void> {
    const id = current.value?.id;
    const page = Math.floor(index / CATALOG_PAGE);
    if (!id || pending.has(page)) return;
    pending.add(page);
    const gen = generation;
    try {
      const got = await client().listCatalogFamilies({
        id,
        query: query.value,
        page: { offset: page * CATALOG_PAGE, limit: CATALOG_PAGE },
      });
      if (gen !== generation) return;
      const next = families.value.slice();
      next.length = got.total;
      got.items.forEach((f, i) => (next[page * CATALOG_PAGE + i] = f));
      families.value = next;
      total.value = got.total;
      listed.value = true;
      error.value = null;
    } catch (e) {
      if (gen === generation) error.value = toApiError(e);
    } finally {
      if (gen === generation) pending.delete(page);
    }
  }

  /** Starts the open catalog's list again from its first page. */
  async function relist(): Promise<void> {
    generation += 1;
    pending.clear();
    families.value = [];
    total.value = 0;
    listed.value = false;
    if (current.value?.enabled) await readPage(0);
  }

  /** Makes sure the families from `start` to `end` (exclusive) are read. */
  function want(start: number, end: number): void {
    for (let i = start; i < end; i += CATALOG_PAGE) if (!families.value[i]) void readPage(i);
    if (end > start && !families.value[end - 1]) void readPage(end - 1);
  }

  function show(entry: CatalogEntry): void {
    quick.close();
    if (open.value === entry) return;
    open.value = entry;
    query.value = '';
    refreshError.value = null;
    if (entry !== 'adobeFonts') void relist();
  }

  function close(): void {
    open.value = null;
  }

  /** The search text of the open catalog; the core filters by it. */
  function setQuery(text: string): void {
    if (text === query.value) return;
    query.value = text;
    void relist();
  }

  /** Fetches the open catalog's list. A failure keeps the cached list and says why. */
  async function refresh(id: CatalogId | undefined = current.value?.id): Promise<boolean> {
    if (!id || refreshing.value) return false;
    refreshing.value = true;
    try {
      put(await client().refreshCatalog({ id }));
      refreshError.value = null;
      if (open.value === id) await relist();
      return true;
    } catch (e) {
      if (open.value === id) refreshError.value = toApiError(e);
      else error.value = toApiError(e);
      return false;
    } finally {
      refreshing.value = false;
    }
  }

  /**
   * The "Turn on" switch of the sidebar and Settings. Turning on a catalog that was
   * never refreshed fetches its list at once: the switch says it connects.
   */
  async function setEnabled(id: CatalogId, enabled: boolean): Promise<void> {
    busy.value = id;
    try {
      const catalog = await client().setCatalogEnabled({ id, enabled });
      put(catalog);
      error.value = null;
      if (enabled && !catalog.refreshedAt) await refresh(id);
      else if (open.value === id) await relist();
    } catch (e) {
      error.value = toApiError(e);
    } finally {
      busy.value = null;
    }
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

  return {
    catalogs,
    open,
    current,
    query,
    families,
    total,
    listed,
    busy,
    refreshing,
    refreshError,
    error,
    online,
    offline,
    load,
    show,
    close,
    setQuery,
    want,
    refresh,
    setEnabled,
  };
});
