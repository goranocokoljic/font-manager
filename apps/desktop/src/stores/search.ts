import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  Collection,
  FaceId,
  FamilyHit,
  FamilyId,
  FilterCounts,
  Filters,
  LanguagePreset,
} from '../api/types';
import { hasFilters } from '../search/chips';
import { useLibraryStore } from './library';

/** Typing waits this long for the next key before it searches (spec: 80 ms). */
export const SEARCH_DEBOUNCE_MS = 80;

/** Family groups fetched per `search` call. */
const PAGE = 500;

/**
 * The search box and the filter chips of the font list: the text, the chosen
 * filters, the family groups the API returns for them and the count of every
 * filter value. With no text and no filter the list shows the whole library;
 * `hits` is `null` then. Counts are always fetched, so the chips have them.
 */
export const useSearchStore = defineStore('search', () => {
  const library = useLibraryStore();
  /** The text in the box, as typed. */
  const text = ref('');
  /** The text the current results are for (after the debounce). */
  const query = ref('');
  const filters = shallowRef<Filters>({});
  const hits = shallowRef<FamilyHit[] | null>(null);
  const counts = shallowRef<FilterCounts>({});
  const total = ref(0);
  const error = ref<ApiError | null>(null);
  const languages = shallowRef<LanguagePreset[]>([]);
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** Bumped by every search; answers to an older one are dropped. */
  let generation = 0;

  const active = computed(() => query.value.trim() !== '' || hasFilters(filters.value));

  /** The matching faces per family of the current results. */
  const matchingFaces = computed(() => {
    const map = new Map<FamilyId, ReadonlySet<FaceId>>();
    for (const hit of hits.value ?? []) map.set(hit.family.id, new Set(hit.faces));
    return map;
  });

  /** Runs the search for the current query and filters now. */
  async function run(): Promise<void> {
    clearTimeout(timer);
    timer = undefined;
    const mine = ++generation;
    const q = query.value.trim();
    const f = filters.value;
    const searching = q !== '' || hasFilters(f);
    const client = useClient();
    try {
      const list: FamilyHit[] = [];
      let found = 0;
      let firstCounts: FilterCounts | null | undefined;
      // Without a search the list shows the library; one family is enough for the counts.
      for (let offset = 0; ; offset += PAGE) {
        const page = await client.search({
          query: q,
          filters: f,
          page: { offset, limit: searching ? PAGE : 1 },
        });
        if (mine !== generation) return;
        firstCounts ??= page.counts;
        found = page.total;
        list.push(...page.families);
        if (!searching || !page.families.length || list.length >= page.total) break;
      }
      counts.value = firstCounts ?? {};
      total.value = found;
      hits.value = searching ? list : null;
      error.value = null;
    } catch (e) {
      if (mine === generation) error.value = e as ApiError;
    }
  }

  /** The box changed: searches once typing pauses for `SEARCH_DEBOUNCE_MS`. */
  function setText(next: string): void {
    text.value = next;
    clearTimeout(timer);
    timer = setTimeout(() => {
      query.value = text.value;
      void run();
    }, SEARCH_DEBOUNCE_MS);
  }

  /** Replaces the chosen filters and searches at once (a chip click is not typing). */
  function setFilters(next: Filters): void {
    filters.value = next;
    void run();
  }

  /** Clears the text and every filter. */
  function clear(): void {
    text.value = '';
    query.value = '';
    setFilters({});
  }

  async function loadLanguages(): Promise<void> {
    if (languages.value.length) return;
    try {
      languages.value = await useClient().listLanguages();
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  /** Saves the chosen filters as a smart collection named `name`. */
  async function saveAsSmartCollection(name: string): Promise<Collection> {
    return useClient().createCollection({ name, rules: filters.value });
  }

  // The library loaded or changed (a scan, an activation): the results and counts are stale.
  watch(
    () => library.families,
    () => void run(),
    { immediate: library.loaded },
  );

  return {
    text,
    query,
    filters,
    hits,
    counts,
    total,
    error,
    languages,
    active,
    matchingFaces,
    run,
    setText,
    setFilters,
    clear,
    loadLanguages,
    saveAsSmartCollection,
  };
});
