import { defineStore } from 'pinia';
import { shallowReactive, watch } from 'vue';
import { useClient } from '../api/client';
import type { FaceId, SizeBasis } from '../api/types';
import { useLibraryStore } from './library';

/** Answers kept; past this many the oldest are forgotten (and asked again when needed). */
export const MAX_SIZE_FACTORS = 5000;

const key = (face: FaceId, basis: SizeBasis) => `${face}\u0000${basis}`;

/**
 * Auto size factors (E16.2): how much each face's preview size is scaled so every
 * face's x-height (or cap height) matches. The API computes them (fm-core) from the
 * metrics the scan stored. Rows ask for the faces on screen only; asks made in the same
 * tick go out as one call per basis, so a page of rows is one call, not one per row.
 * Answers are kept until the library changes; a face not answered yet, or whose answer
 * failed, keeps factor 1.
 */
export const useSizeFactorsStore = defineStore('sizeFactors', () => {
  /** Factors by (face, basis). Reading one key tracks it. */
  const factors = shallowReactive(new Map<string, number>());
  /** Asked and not answered yet, or failed: not asked again until a reset. */
  const asked = new Set<string>();
  let queue = new Map<SizeBasis, Set<FaceId>>();
  let queued = false;
  /** Bumped by every reset; answers to an older round are dropped. */
  let round = 0;

  /** The factor of `face` for `basis`; 1 until it has arrived. */
  function get(face: FaceId, basis: SizeBasis): number {
    return factors.get(key(face, basis)) ?? 1;
  }

  /** Asks for `face`'s factor for `basis` unless it is known or asked already. */
  function want(face: FaceId, basis: SizeBasis): void {
    const k = key(face, basis);
    // Reading `factors` here lets a caller's effect ask again after a reset.
    if (factors.has(k) || asked.has(k)) return;
    asked.add(k);
    let faces = queue.get(basis);
    if (!faces) queue.set(basis, (faces = new Set()));
    faces.add(face);
    if (!queued) {
      queued = true;
      queueMicrotask(flush);
    }
  }

  function flush(): void {
    queued = false;
    const batch = queue;
    queue = new Map();
    const mine = round;
    for (const [basis, faces] of batch) {
      void useClient()
        .sizeFactors({ faces: [...faces], basis })
        .then(
          (got) => {
            if (mine !== round) return;
            if (factors.size + got.length > MAX_SIZE_FACTORS) factors.clear();
            for (const f of got) {
              const k = key(f.face, basis);
              asked.delete(k);
              factors.set(k, typeof f.factor === 'number' && f.factor > 0 ? f.factor : 1);
            }
          },
          () => undefined,
        );
    }
  }

  /** Forgets every factor: the library changed, so a face's file may have. */
  function reset(): void {
    round++;
    asked.clear();
    queue = new Map();
    factors.clear();
  }

  watch(() => useLibraryStore().families, reset);

  return { get, want, reset };
});
