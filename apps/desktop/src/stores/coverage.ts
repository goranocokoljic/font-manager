import { defineStore } from 'pinia';
import { ref, shallowReactive } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceCoverage, FaceId } from '../api/types';

/**
 * The coverage list of the Glyphs tab (E11.3), as fm-core computes it from the index:
 * per Unicode block the characters the face maps against the block's assigned code
 * points, and per language preset how much the face covers, complete ones first. The
 * characters a language lacks come from fm-core's `check_coverage` when the user asks.
 */
export const useCoverageStore = defineStore('coverage', () => {
  const face = ref<FaceId | null>(null);
  const coverage = ref<FaceCoverage | null>(null);
  const error = ref<ApiError | null>(null);
  const loading = ref(false);
  /** The characters each asked-for language lacks, by preset code. */
  const missing = shallowReactive(new Map<string, string[]>());
  /** Why the missing characters of a language could not be listed, by preset code. */
  const missingErrors = shallowReactive(new Map<string, ApiError>());

  // Answers about an earlier face are dropped.
  let request = 0;

  /** Shows the coverage of `id`. */
  async function open(id: FaceId): Promise<void> {
    if (id === face.value) return;
    const mine = ++request;
    face.value = id;
    coverage.value = null;
    error.value = null;
    missing.clear();
    missingErrors.clear();
    loading.value = true;
    try {
      const c = await useClient().faceCoverage(id);
      if (mine === request) coverage.value = c;
    } catch (e) {
      if (mine === request) error.value = e as ApiError;
    } finally {
      if (mine === request) loading.value = false;
    }
  }

  /** Asks fm-core which characters of the preset `code` the face lacks (once). */
  async function loadMissing(code: string): Promise<void> {
    if (face.value === null || missing.has(code)) return;
    const mine = request;
    missingErrors.delete(code);
    try {
      const report = await useClient().checkCoverage({
        face: face.value,
        target: { kind: 'languages', codes: [code] },
      });
      if (mine === request) missing.set(code, report.languages[0]?.missing ?? report.missing);
    } catch (e) {
      if (mine === request) missingErrors.set(code, e as ApiError);
    }
  }

  return { face, coverage, error, loading, missing, missingErrors, open, loadMissing };
});
