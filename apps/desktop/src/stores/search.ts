import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  Collection,
  FaceId,
  FamilyHit,
  FamilyId,
  FamilyOrder,
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
  /** The text the count line checks the results against (E43.1); empty for none. */
  const coverText = ref('');
  /** How many of the results render `coverText` completely; null while unknown or off. */
  const coverTotal = ref<number | null>(null);
  let coverTimer: ReturnType<typeof setTimeout> | undefined;
  /** Bumped by every cover count; answers to an older one are dropped. */
  let coverGeneration = 0;
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

  /**
   * The order to ask for: the chosen one. With a language or text filter and nothing
   * typed, the API would rank by fewest missing characters; the count line says "by
   * name" there, so it asks for that. Without a search only the counts are read: no
   * order to pay for.
   */
  function orderOf(q: string, f: Filters, searching: boolean): FamilyOrder | null {
    if (!searching) return null;
    if (library.order) return library.order;
    return q === '' && f.supports ? { by: 'name', descending: false } : null;
  }

  /** Runs the search for the current query and filters now. */
  async function run(): Promise<void> {
    clearTimeout(timer);
    timer = undefined;
    const mine = ++generation;
    const q = query.value.trim();
    const f = filters.value;
    const searching = q !== '' || hasFilters(f);
    const order = orderOf(q, f, searching);
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
          ...(order ? { order } : {}),
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
    if (mine === generation) await runCover();
  }

  /**
   * Counts the families of the current results that render `coverText` completely: the
   * same search with a text-coverage rule on top, so the query and every filter (a
   * language filter included) still apply. The API does the coverage.
   */
  async function runCover(): Promise<void> {
    clearTimeout(coverTimer);
    coverTimer = undefined;
    const mine = ++coverGeneration;
    const text = coverText.value;
    if (!text) {
      coverTotal.value = null;
      return;
    }
    const q = query.value.trim();
    try {
      const page = await useClient().search({
        query: q,
        filters: filters.value,
        rule: {
          op: 'and',
          rules: [{ kind: 'filters', filters: { supports: { kind: 'text', text } } }],
        },
        page: { offset: 0, limit: 1 },
      });
      if (mine === coverGeneration) coverTotal.value = page.total;
    } catch {
      // No count is better than a wrong one: the line shows the plain family count.
      if (mine === coverGeneration) coverTotal.value = null;
    }
  }

  /**
   * The text to count the results against; empty turns the count off. Typing waits for the
   * next key as the search box does.
   */
  function setCoverText(next: string): void {
    if (next === coverText.value) return;
    coverText.value = next;
    clearTimeout(coverTimer);
    if (!next) {
      coverGeneration++;
      coverTotal.value = null;
      return;
    }
    coverTimer = setTimeout(() => void runCover(), SEARCH_DEBOUNCE_MS);
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
    coverText,
    coverTotal,
    active,
    matchingFaces,
    run,
    setText,
    setFilters,
    setCoverText,
    clear,
    loadLanguages,
    saveAsSmartCollection,
  };
});
