import { defineStore } from 'pinia';
import { shallowReactive, watch } from 'vue';
import { useClient } from '../api/client';
import type { FaceId, Metrics } from '../api/types';
import { useLibraryStore } from './library';

/** Faces kept; past this many the oldest are forgotten (and asked again when needed). */
export const MAX_FACE_METRICS = 5000;

/**
 * Vertical metrics of the faces on screen, for the metrics overlay (E16.1). fm-core
 * reads them at scan time and stores them with the face (E16.2); `get_face` returns
 * them from the index without opening the file. Rows ask only while the overlay is
 * on, and each face is asked once until the library changes; a face whose answer
 * failed draws no lines and is not asked again until then.
 */
export const useFaceMetricsStore = defineStore('faceMetrics', () => {
  /** Metrics by face. Reading one key tracks it. */
  const metrics = shallowReactive(new Map<FaceId, Metrics>());
  /** Asked and not answered yet, or failed. */
  const asked = new Set<FaceId>();
  /** Bumped by every reset; answers to an older round are dropped. */
  let round = 0;

  /** The metrics of `face`; absent until they have arrived. */
  function get(face: FaceId): Metrics | undefined {
    return metrics.get(face);
  }

  /** Asks for `face`'s metrics unless they are known or asked already. */
  function want(face: FaceId): void {
    // Reading `metrics` here lets a caller's effect ask again after a reset.
    if (metrics.has(face) || asked.has(face)) return;
    asked.add(face);
    const mine = round;
    void useClient()
      .getFace(face)
      .then(
        (detail) => {
          if (mine !== round) return;
          asked.delete(face);
          if (metrics.size >= MAX_FACE_METRICS) metrics.delete(metrics.keys().next().value!);
          metrics.set(face, detail.metrics);
        },
        () => undefined,
      );
  }

  /** Forgets every face's metrics: the library changed, so a face's file may have. */
  function reset(): void {
    round++;
    asked.clear();
    metrics.clear();
  }

  watch(() => useLibraryStore().families, reset);

  return { get, want, reset };
});
