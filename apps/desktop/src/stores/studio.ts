import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary, FamilyHit } from '../api/types';

/** Families a block font picker lists per search. */
export const STUDIO_RESULTS = 20;

/**
 * Pairing Studio's per-block fonts (E24.2): a block of a layout can be set in its own face
 * instead of the panel's. The choice lives here for the session only; it is never written
 * into the layout template file, which keeps its fm-api shape. A block is named by its
 * layout id and index, the only identity a block has. The pickers search the library
 * through fm-core's search (grouped by family); choosing a family takes its best face.
 */
export const useStudioStore = defineStore('studio', () => {
  /** Block fonts by `layoutId#index`. */
  const blockFaces = ref<Record<string, FaceSummary>>({});
  /** The picker that searched last (`layoutId#index`), so only it shows the results. */
  const searchFor = ref<string | null>(null);
  const query = ref('');
  const results = ref<FamilyHit[]>([]);
  const error = ref<ApiError | null>(null);

  // Answers to an older search, or to an older pick for the same block, are dropped.
  let searchRequest = 0;
  const picks = new Map<string, number>();

  const key = (layoutId: string, index: number) => `${layoutId}#${index}`;

  /** The face block `index` of layout `layoutId` is set in; null uses the panel's. */
  function faceOf(layoutId: string | null | undefined, index: number): FaceSummary | null {
    return layoutId ? (blockFaces.value[key(layoutId, index)] ?? null) : null;
  }

  /** Searches the library by name for the picker of one block. */
  async function search(layoutId: string, index: number, text: string): Promise<void> {
    searchFor.value = key(layoutId, index);
    query.value = text;
    const mine = ++searchRequest;
    try {
      const r = await useClient().search({
        query: text.trim(),
        page: { offset: 0, limit: STUDIO_RESULTS },
      });
      if (mine !== searchRequest) return;
      results.value = r.families;
      error.value = null;
    } catch (e) {
      if (mine === searchRequest) error.value = e as ApiError;
    }
  }

  /** Sets block `index` in face `face`. */
  async function setBlockFace(layoutId: string, index: number, face: FaceId): Promise<void> {
    const k = key(layoutId, index);
    const mine = (picks.get(k) ?? 0) + 1;
    picks.set(k, mine);
    try {
      const detail = await useClient().getFace(face);
      if (picks.get(k) !== mine) return;
      blockFaces.value = { ...blockFaces.value, [k]: detail.face };
      error.value = null;
    } catch (e) {
      if (picks.get(k) === mine) error.value = e as ApiError;
    }
  }

  /** Sets block `index` in the best face of a search result. */
  async function pickFamily(layoutId: string, index: number, hit: FamilyHit): Promise<void> {
    const best = hit.faces[0];
    if (best !== undefined) await setBlockFace(layoutId, index, best);
  }

  /** "Use panel font": block `index` follows the panel's face again. */
  function resetBlock(layoutId: string, index: number): void {
    const k = key(layoutId, index);
    picks.set(k, (picks.get(k) ?? 0) + 1);
    const rest = { ...blockFaces.value };
    delete rest[k];
    blockFaces.value = rest;
  }

  return {
    blockFaces,
    searchFor,
    query,
    results,
    error,
    faceOf,
    search,
    setBlockFace,
    pickFamily,
    resetBlock,
  };
});
