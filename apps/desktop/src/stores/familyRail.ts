import { defineStore } from 'pinia';
import { computed, ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  FaceId,
  FaceLanguageCoverage,
  Filters,
  LanguagePreset,
  PairingSuggestion,
} from '../api/types';
import { chosenLanguages } from '../search/chips';

/** The language chips the rail shows before "+N more". */
export const RAIL_LANGUAGE_CHIPS = 6;
/** The pairings the rail shows: the top two of `suggestPairings`. */
export const RAIL_PAIRINGS = 2;

/** The languages fm-core counts the face as covering in full, in its order. */
export function completeLanguages(
  languages: readonly FaceLanguageCoverage[],
): FaceLanguageCoverage[] {
  return languages.filter((l) => l.total > 0 && l.covered >= l.total);
}

/** `filters` with the language `code` among the ones the `supports` filter asks for. */
export function withLanguage(filters: Filters, code: string): Filters {
  const now = chosenLanguages(filters);
  if (now.includes(code)) return filters;
  return { ...filters, supports: { kind: 'languages', codes: [...now, code] } };
}

/**
 * The family page's rail (E42.9): the languages the face covers in full, the language the
 * page previews in, the top two pairings and the offer to filter the wall by the language.
 * The chosen language stays while the arrows change the family when the next one covers it
 * too; otherwise the page goes back to the user's text.
 */
export const useFamilyRailStore = defineStore('familyRail', () => {
  const face = ref<FaceId | null>(null);
  const languages = shallowRef<FaceLanguageCoverage[]>([]);
  const pairings = shallowRef<PairingSuggestion[]>([]);
  const error = ref<ApiError | null>(null);
  /** The chips past the first six show. */
  const expanded = ref(false);
  /** The language the page previews in; null shows the user's own text. */
  const language = ref<string | null>(null);
  /** Going back to the wall filters it by `language` ("Show only fonts that can do …"). */
  const filterWall = ref(false);
  /** Bumped by every `load`; an answer to an older one is dropped. */
  let request = 0;

  const complete = computed(() => completeLanguages(languages.value));
  const shown = computed(() =>
    expanded.value ? complete.value : complete.value.slice(0, RAIL_LANGUAGE_CHIPS),
  );
  const more = computed(() => Math.max(0, complete.value.length - shown.value.length));

  /** Reads the coverage and the pairings of `id`; null clears the rail. */
  async function load(id: FaceId | null): Promise<void> {
    if (id === face.value) return;
    const mine = ++request;
    face.value = id;
    languages.value = [];
    pairings.value = [];
    error.value = null;
    expanded.value = false;
    if (id === null) {
      choose(null);
      return;
    }
    const client = useClient();
    const [coverage, pairs] = await Promise.allSettled([
      client.faceCoverage(id),
      client.suggestPairings({ face: id, limit: RAIL_PAIRINGS }),
    ]);
    if (mine !== request) return;
    if (coverage.status === 'fulfilled') languages.value = coverage.value.languages;
    else error.value = coverage.reason as ApiError;
    // Pairings are extra: without them the languages and facts still show.
    if (pairs.status === 'fulfilled') pairings.value = pairs.value.slice(0, RAIL_PAIRINGS);
    // A language this face does not cover in full would show tofu: back to the user's text.
    if (language.value && !complete.value.some((l) => l.code === language.value)) choose(null);
  }

  /** Previews the page in `code`; the chosen one again, or null, goes back to the user's text. */
  function choose(code: string | null): void {
    language.value = code === language.value ? null : code;
    if (language.value === null) filterWall.value = false;
  }

  /** The text the page previews in: the pangram, else the distinctive glyphs; null for none. */
  function textOf(presets: readonly LanguagePreset[]): string | null {
    if (language.value === null) return null;
    const preset = presets.find((p) => p.code === language.value);
    return preset?.pangram || preset?.sampleGlyphs || null;
  }

  /**
   * The language the wall should filter by on the way back, once: it is taken, so a later
   * visit starts without the offer chosen.
   */
  function takeWallLanguage(): string | null {
    const code = filterWall.value ? language.value : null;
    filterWall.value = false;
    return code;
  }

  return {
    face,
    languages,
    pairings,
    error,
    expanded,
    language,
    filterWall,
    complete,
    shown,
    more,
    load,
    choose,
    textOf,
    takeWallLanguage,
  };
});
