import { defineStore } from 'pinia';
import { computed, ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary, SimilarEmpty, SimilarFace } from '../api/types';
import { useFontListStore } from './fontList';

/** Faces the "Similar to …" panel asks `findSimilar` for. */
export const SIMILAR_LIMIT = 12;

/**
 * One row of the panel (E54.10): a family's closest face, its score as a whole percent,
 * how many more of its styles were found and the reasons that set it apart from the other
 * rows. The store derives it from `findSimilar`'s answer, so the panel computes nothing.
 */
export interface SimilarRow {
  result: SimilarFace;
  /** The score as a whole percent; null when the API gave none. */
  percent: number | null;
  /** The other styles of the family in the answer. */
  moreStyles: number;
  /** The result's reasons that not every row shares. */
  reasons: string[];
}

/**
 * "Find similar" (E23.3): the face the panel compares against and `findSimilar`'s
 * answer for it, ranked by fm-core. Picking a result selects it in the font list; the
 * panel tells the inspector through its `select` event.
 */
export const useSimilarStore = defineStore('similar', () => {
  const fontList = useFontListStore();

  const open = ref(false);
  /** The face the results are similar to. */
  const target = shallowRef<FaceSummary | null>(null);
  const results = shallowRef<SimilarFace[]>([]);
  /** Why the answer is empty, from fm-core (E54.11): a code and a message to show as it is. */
  const empty = ref<SimilarEmpty | null>(null);
  const emptyMessage = ref<string | null>(null);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** The result last picked, shown as current. */
  const picked = ref<FaceId | null>(null);
  /** Bumped by every `openFor`; an answer to an older one is dropped. */
  let request = 0;

  /** Opens the panel for `face` and asks the API for the faces that look like it. */
  async function openFor(face: FaceSummary): Promise<void> {
    const mine = ++request;
    target.value = face;
    results.value = [];
    empty.value = null;
    emptyMessage.value = null;
    error.value = null;
    picked.value = null;
    loading.value = true;
    open.value = true;
    try {
      const got = await useClient().findSimilarAnswer({ face: face.id, limit: SIMILAR_LIMIT });
      if (mine === request) {
        results.value = got.faces;
        empty.value = got.faces.length ? null : (got.empty ?? null);
        emptyMessage.value = got.faces.length ? null : (got.message ?? null);
      }
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** The rows the panel shows: one per family, in the answer's order of first appearance. */
  const rows = computed<SimilarRow[]>(() => groupRows(results.value));

  /** Selects a result in the font list; false when the list does not show its family. */
  async function pick(face: FaceSummary): Promise<boolean> {
    picked.value = face.id;
    return fontList.reveal(face);
  }

  function close(): void {
    request++;
    open.value = false;
    loading.value = false;
  }

  return {
    open,
    target,
    results,
    empty,
    emptyMessage,
    error,
    loading,
    picked,
    rows,
    openFor,
    pick,
    close,
  };
});

/** A score (0–1) as a whole percent; null (not a number) when the API gave none. */
export function scorePercent(score: number | null): number | null {
  return score === null || !Number.isFinite(score) ? null : Math.round(score * 100);
}

/**
 * One row per family: its closest face (highest score) and how many more of its styles
 * the answer has. A reason every row shares says nothing about which one is closer, so
 * each row keeps only the reasons that differ.
 */
export function groupRows(results: readonly SimilarFace[]): SimilarRow[] {
  const byFamily = new Map<string, { best: SimilarFace; extra: number }>();
  for (const r of results) {
    const g = byFamily.get(r.face.familyName);
    if (!g) byFamily.set(r.face.familyName, { best: r, extra: 0 });
    else {
      g.extra += 1;
      if ((r.score ?? -1) > (g.best.score ?? -1)) g.best = r;
    }
  }
  const groups = [...byFamily.values()];
  const shared = new Set<string>();
  if (groups.length > 1) {
    const [first, ...rest] = groups.map((g) => g.best.reasons);
    for (const x of first ?? []) if (rest.every((o) => o.includes(x))) shared.add(x);
  }
  return groups.map(({ best, extra }) => ({
    result: best,
    percent: scorePercent(best.score),
    moreStyles: extra,
    reasons: best.reasons.filter((x) => !shared.has(x)),
  }));
}
