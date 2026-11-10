import { defineStore } from 'pinia';
import { ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary, SimilarFace } from '../api/types';

/** Faces the Compare sheet's 'Similar to <base>' section shows at most (E48.3). */
export const COMPARE_SIMILAR_LIMIT = 8;

/** The rows of one shared reason, best first; `reasons` is empty when fm-core gave none. */
export interface SimilarGroup {
  /** The reasons, joined; the group's key. */
  key: string;
  reasons: string[];
  rows: SimilarFace[];
}

/**
 * Groups `findSimilar`'s answer (best first) under the reasons the rows share, so a reason
 * is named once per group and never row by row. A group comes where its best row ranks;
 * its rows keep the rank order. A face listed twice, or the base itself, is shown once / not.
 */
export function groupByReason(
  results: readonly SimilarFace[],
  base: FaceSummary | null = null,
): SimilarGroup[] {
  const groups = new Map<string, SimilarGroup>();
  const seen = new Set<number>();
  for (const r of results) {
    if (r.face.id === base?.id || seen.has(r.face.id)) continue;
    seen.add(r.face.id);
    const key = r.reasons.join(' · ');
    const g = groups.get(key);
    if (g) g.rows.push(r);
    else groups.set(key, { key, reasons: [...r.reasons], rows: [r] });
  }
  return [...groups.values()];
}

/**
 * 'Similar to <base>' in the Compare sheet (E48.3): the faces `findSimilar` ranks as closest
 * to the base (the first pin, E48.5), up to `COMPARE_SIMILAR_LIMIT`. A new base asks again;
 * an answer to an older base is dropped.
 */
export const useCompareSimilarStore = defineStore('compareSimilar', () => {
  /** The face the results are for; null when nothing is pinned. */
  const base = shallowRef<FaceSummary | null>(null);
  const results = shallowRef<SimilarFace[]>([]);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** Bumped by every `load`; an answer to an older one is dropped. */
  let request = 0;

  /** Asks the API for the faces that look like `face`; null clears the section. */
  async function load(face: FaceSummary | null): Promise<void> {
    const mine = ++request;
    base.value = face;
    results.value = [];
    error.value = null;
    loading.value = face !== null;
    if (!face) return;
    try {
      const got = await useClient().findSimilar({ face: face.id, limit: COMPARE_SIMILAR_LIMIT });
      if (mine === request) results.value = got.slice(0, COMPARE_SIMILAR_LIMIT);
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** Faces that left the library (E48.8): their rows go; the rest keep their order. */
  function drop(removed: readonly FaceId[]): void {
    const gone = new Set(removed);
    if (results.value.some((r) => gone.has(r.face.id))) {
      results.value = results.value.filter((r) => !gone.has(r.face.id));
    }
  }

  return { base, results, error, loading, load, drop };
});
