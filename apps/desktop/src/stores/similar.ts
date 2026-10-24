import { defineStore } from 'pinia';
import { ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary, SimilarFace } from '../api/types';
import { useFontListStore } from './fontList';

/** Faces the "Similar to …" panel asks `findSimilar` for. */
export const SIMILAR_LIMIT = 12;

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
    error.value = null;
    picked.value = null;
    loading.value = true;
    open.value = true;
    try {
      const got = await useClient().findSimilar({ face: face.id, limit: SIMILAR_LIMIT });
      if (mine === request) results.value = got;
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

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

  return { open, target, results, error, loading, picked, openFor, pick, close };
});

/** A score (0–1) as a whole percent; null (not a number) when the API gave none. */
export function scorePercent(score: number | null): number | null {
  return score === null || !Number.isFinite(score) ? null : Math.round(score * 100);
}
