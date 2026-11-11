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
  GlyphReference,
  GlyphTraitInfo,
  LanguageHint,
  LanguagePreset,
  RuleGroup,
  TraitFilter,
} from '../api/types';
import { hasFilters } from '../search/chips';
import { chooseTrait, parseTraitWords, withoutTrait } from '../search/traits';
import { useLibraryStore } from './library';

/** Typing waits this long for the next key before it searches (spec: 80 ms). */
export const SEARCH_DEBOUNCE_MS = 80;

/**
 * The reference of a likeness sort (E49.9, "Fonts with an R like this"): the face and the
 * character, with what the chip and the count line call it.
 */
export type LikeGlyph = GlyphReference & {
  /** The character as typed: `R`. */
  glyph: string;
  /** The reference face's family name: `Inter`. */
  family: string;
};

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
  /**
   * The glyph traits every result must have (E49.6), chosen on the Letters preset's letters
   * or typed as `R:straight`; one value per trait. Not a `Filters` field: a smart collection
   * does not take them.
   */
  const traits = shallowRef<TraitFilter[]>([]);
  /**
   * The likeness sort (E49.9): while set the API orders the results by how close each
   * family's drawing of the character is to the reference face's. It replaces the view's
   * sort without changing it, so removing it brings that sort back. Not a filter.
   */
  const likeGlyph = shallowRef<LikeGlyph | null>(null);
  /** The glyph trait catalog from the API, read once (`loadTraitCatalog`). */
  const traitCatalog = shallowRef<GlyphTraitInfo[]>([]);
  let catalogLoad: Promise<void> | null = null;
  const hits = shallowRef<FamilyHit[] | null>(null);
  const counts = shallowRef<FilterCounts>({});
  const total = ref(0);
  const error = ref<ApiError | null>(null);
  const languages = shallowRef<LanguagePreset[]>([]);
  /** The text the count line checks the results against (E43.1); empty for none. */
  const coverText = ref('');
  /** How many of the results render `coverText` completely; null while unknown or off. */
  const coverTotal = ref<number | null>(null);
  /**
   * How many families the results have before the switch hides any (E43.3): the search
   * without the text rule. Null while unknown or off.
   */
  const coverBase = ref<number | null>(null);
  /** The languages the API detects in `coverText`, best first (E43.3); empty for none. */
  const detected = shallowRef<LanguageHint[]>([]);
  /** The "Hide the N with missing glyphs" switch is on (E43.3); it acts only with `coverText`. */
  const hideMissing = ref(false);
  let coverTimer: ReturnType<typeof setTimeout> | undefined;
  /** Bumped by every cover count; answers to an older one are dropped. */
  let coverGeneration = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  /** Bumped by every search; answers to an older one are dropped. */
  let generation = 0;
  /**
   * A search changed the results and the cover counts are not redone yet. A newer search
   * that drops it (the switch's `run(false)`) still owes the recount.
   */
  let recountOwed = false;

  /** The switch hides the families that cannot render `coverText` now. */
  const hiding = computed(() => hideMissing.value && coverText.value !== '');

  const active = computed(
    () =>
      query.value.trim() !== '' ||
      hasFilters(filters.value) ||
      traits.value.length > 0 ||
      likeGlyph.value !== null ||
      hiding.value,
  );

  /** The trait filters of a request: left out when there are none. */
  const traitPart = (list: readonly TraitFilter[]) =>
    list.length ? { glyphTraits: [...list] } : {};

  /** The rule that keeps only the families rendering `text` completely. */
  const rendersText = (text: string): RuleGroup => ({
    op: 'and',
    rules: [{ kind: 'filters', filters: { supports: { kind: 'text', text } } }],
  });

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
    return q === '' && (f.supports || hiding.value) ? { by: 'name', descending: false } : null;
  }

  /**
   * Runs the search for the current query and filters now. `recount` false keeps the cover
   * counts: the switch changes what the list shows, not how many render the text.
   */
  async function run(recount = true): Promise<void> {
    if (recount) recountOwed = true;
    clearTimeout(timer);
    timer = undefined;
    const mine = ++generation;
    const q = query.value.trim();
    const f = filters.value;
    const rule = hiding.value ? rendersText(coverText.value) : null;
    const chosenTraits = traits.value;
    const like = likeGlyph.value;
    const searching =
      q !== '' || hasFilters(f) || chosenTraits.length > 0 || rule !== null || like !== null;
    // A likeness sort decides the order itself.
    const order = like ? null : orderOf(q, f, searching);
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
          ...traitPart(chosenTraits),
          ...(rule ? { rule } : {}),
          ...(order ? { order } : {}),
          ...(like ? { likeGlyph: { face: like.face, codePoint: like.codePoint } } : {}),
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
    if (mine === generation && recountOwed) await runCover();
  }

  /**
   * Counts the families of the current results that render `coverText` completely: the
   * same search with a text-coverage rule on top, so the query and every filter (a
   * language filter included) still apply. The API does the coverage. It also counts the
   * results without that rule, for "N of M" and the switch's "Hide the M−N" while the switch
   * hides them, and asks the API which languages the text is in.
   */
  async function runCover(): Promise<void> {
    clearTimeout(coverTimer);
    coverTimer = undefined;
    const mine = ++coverGeneration;
    // This count reads the query and filters as they are now: it pays what a search owed.
    recountOwed = false;
    const text = coverText.value;
    if (!text) {
      coverTotal.value = null;
      coverBase.value = null;
      detected.value = [];
      return;
    }
    const q = query.value.trim();
    const client = useClient();
    const count = (rule: RuleGroup | null) =>
      client
        .search({
          query: q,
          filters: filters.value,
          ...traitPart(traits.value),
          ...(rule ? { rule } : {}),
          page: { offset: 0, limit: 1 },
        })
        .then((page) => page.total);
    const [covered, base, hints] = await Promise.all([
      count(rendersText(text)).catch(() => null),
      count(null).catch(() => null),
      client.detectLanguages({ text }).catch(() => [] as LanguageHint[]),
    ]);
    if (mine !== coverGeneration) return;
    // No count is better than a wrong one: the line shows the plain family count.
    coverTotal.value = covered !== null && base !== null ? covered : null;
    coverBase.value = coverTotal.value !== null ? base : null;
    detected.value = hints;
  }

  /**
   * The text to count the results against; empty turns the count off. Typing waits for the
   * next key as the search box does.
   */
  function setCoverText(next: string): void {
    if (next === coverText.value) return;
    const wasHiding = hiding.value;
    coverText.value = next;
    clearTimeout(coverTimer);
    if (!next) {
      coverGeneration++;
      coverTotal.value = null;
      coverBase.value = null;
      detected.value = [];
      // The switch hid families for the old text: they come back.
      if (wasHiding) void run();
      return;
    }
    coverTimer = setTimeout(() => {
      // The switch hides by the text: the results change with it (run counts it too).
      if (hiding.value || wasHiding) void run();
      else void runCover();
    }, SEARCH_DEBOUNCE_MS);
  }

  /**
   * Turns the "Hide the N with missing glyphs" switch on or off and searches at once, as a
   * chip click does. It hides nothing while there is no text to check.
   */
  function setHideMissing(on: boolean): void {
    if (on === hideMissing.value) return;
    const before = hiding.value;
    hideMissing.value = on;
    if (hiding.value !== before) void run(false);
  }

  /**
   * The box changed: searches once typing pauses for `SEARCH_DEBOUNCE_MS`. Words like
   * `R:straight` that name a glyph trait and a value of it (E49.6) leave the box and become
   * trait filters, shown as chips.
   */
  function setText(next: string): void {
    text.value = next;
    clearTimeout(timer);
    timer = setTimeout(() => void commitText(), SEARCH_DEBOUNCE_MS);
  }

  async function commitText(): Promise<void> {
    if (text.value.includes(':')) {
      await loadTraitCatalog();
      const parsed = parseTraitWords(text.value, traitCatalog.value);
      if (parsed.traits.length) {
        text.value = parsed.text;
        let next = traits.value;
        for (const f of parsed.traits) next = [...withoutTrait(next, f.traitId), f];
        traits.value = next;
      }
    }
    query.value = text.value;
    void run();
  }

  /** Reads the glyph trait catalog once; an error leaves it empty (no letter is a control). */
  function loadTraitCatalog(): Promise<void> {
    catalogLoad ??= useClient()
      .listGlyphTraits()
      .then((list) => {
        traitCatalog.value = list;
      })
      .catch(() => {
        catalogLoad = null;
      });
    return catalogLoad;
  }

  /** Chooses `value` for a trait in place of its old one; `null` ("Any") takes it off. */
  function chooseTraitValue(traitId: string, value: string | null): void {
    traits.value = chooseTrait(traits.value, traitId, value);
    void run();
  }

  /** Replaces every trait filter and searches. */
  function setTraits(next: TraitFilter[]): void {
    traits.value = next;
    void run();
  }

  /**
   * How many families each value of a trait gives in the current result: the same search
   * with that value chosen for the trait in place of the one chosen now (the API counts;
   * one search per value, as the cover count does). A value whose count fails is left out.
   */
  async function countTraitValues(info: GlyphTraitInfo): Promise<Map<string, number>> {
    const q = query.value.trim();
    const f = filters.value;
    const rule = hiding.value ? rendersText(coverText.value) : null;
    const others = withoutTrait(traits.value, info.id);
    const client = useClient();
    const totals = await Promise.all(
      info.values.map((v) =>
        client
          .search({
            query: q,
            filters: f,
            glyphTraits: [...others, { kind: 'value', traitId: info.id, value: v.id }],
            ...(rule ? { rule } : {}),
            page: { offset: 0, limit: 1 },
          })
          .then((page) => [v.id, page.total] as const)
          .catch(() => null),
      ),
    );
    return new Map(totals.filter((x) => x !== null));
  }

  /**
   * Sorts the results by the glyph `like.glyph` of face `like.face` (E49.9); `null` ends the
   * likeness sort and the view's own sort applies again. Searches at once.
   */
  function setLikeGlyph(like: LikeGlyph | null): void {
    const now = likeGlyph.value;
    if (like === now) return;
    if (like && now && like.face === now.face && like.codePoint === now.codePoint) return;
    likeGlyph.value = like;
    void run();
  }

  /** Replaces the chosen filters and searches at once (a chip click is not typing). */
  function setFilters(next: Filters): void {
    filters.value = next;
    void run();
  }

  /** Clears the text and every filter, the glyph traits too. */
  function clear(): void {
    text.value = '';
    query.value = '';
    traits.value = [];
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
    traits,
    likeGlyph,
    traitCatalog,
    hits,
    counts,
    total,
    error,
    languages,
    coverText,
    coverTotal,
    coverBase,
    detected,
    hideMissing,
    hiding,
    active,
    matchingFaces,
    run,
    setText,
    setFilters,
    setTraits,
    setLikeGlyph,
    chooseTraitValue,
    countTraitValues,
    loadTraitCatalog,
    setCoverText,
    setHideMissing,
    clear,
    loadLanguages,
    saveAsSmartCollection,
  };
});
