import { defineStore } from 'pinia';
import { ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceSummary, PairingSuggestion } from '../api/types';
import { useCompareStore, type PinResult } from './compare';

/** Suggestions the Pairings panel asks `suggestPairings` for. */
export const PAIRING_LIMIT = 8;

/**
 * Pairings (E23.4): the fonts fm-core's rules pair with the first pinned font of
 * Compare, best first, each with its reasons. "Add to compare" pins one.
 */
export const usePairingsStore = defineStore('pairings', () => {
  /** The face the suggestions are for; null when nothing is pinned. */
  const base = shallowRef<FaceSummary | null>(null);
  const results = shallowRef<PairingSuggestion[]>([]);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** Bumped by every `load`; an answer to an older one is dropped. */
  let request = 0;

  /** Asks the API for the faces that pair with `face`; null clears the panel. */
  async function load(face: FaceSummary | null): Promise<void> {
    const mine = ++request;
    base.value = face;
    results.value = [];
    error.value = null;
    loading.value = face !== null;
    if (!face) return;
    try {
      const got = await useClient().suggestPairings({ face: face.id, limit: PAIRING_LIMIT });
      if (mine === request) results.value = got;
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** Pins a suggestion to Compare; the outcome is told in a toast. */
  function add(face: FaceSummary): PinResult {
    return useCompareStore().pin(face);
  }

  return { base, results, error, loading, load, add };
});
