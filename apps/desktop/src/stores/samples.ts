import { defineStore } from 'pinia';
import { shallowReactive, watch } from 'vue';
import { useClient } from '../api/client';
import type { FaceId, SampleText } from '../api/types';
import { useLibraryStore } from './library';

/** Answers kept; past this many the oldest are forgotten (and asked again when needed). */
export const MAX_SAMPLE_ANSWERS = 5000;

const key = (face: FaceId, text: string) => `${face}\u0000${text}`;

/**
 * The text each face previews for a sample text. The API decides (fm-core): the
 * sample when the face maps it, else text from the face's own coverage, marked as a
 * fallback. Rows ask for the faces on screen only; asks made in the same tick go out
 * as one call per sample text. Answers are kept per (face, sample) until the library
 * changes; a face whose answer failed previews the sample.
 */
export const useSamplesStore = defineStore('samples', () => {
  /** Answers by (face, sample). Reading one key tracks it. */
  const answers = shallowReactive(new Map<string, SampleText>());
  /** Asked and not answered yet, or failed: not asked again until a reset. */
  const asked = new Set<string>();
  let queue = new Map<string, Set<FaceId>>();
  let queued = false;
  /** Bumped by every reset; answers to an older round are dropped. */
  let round = 0;

  /** The answer for `face` and the sample `text`, if it has arrived. */
  function get(face: FaceId, text: string): SampleText | undefined {
    return answers.get(key(face, text));
  }

  /** Asks for `face`'s preview text for `text` unless it is known or asked already. */
  function want(face: FaceId, text: string): void {
    if (!text) return;
    const k = key(face, text);
    // Reading `answers` here lets a caller's effect ask again after a reset.
    if (answers.has(k) || asked.has(k)) return;
    asked.add(k);
    let faces = queue.get(text);
    if (!faces) queue.set(text, (faces = new Set()));
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
    for (const [text, faces] of batch) {
      void useClient()
        .sampleTexts({ faces: [...faces], text })
        .then(
          (got) => {
            if (mine !== round) return;
            if (answers.size + got.length > MAX_SAMPLE_ANSWERS) answers.clear();
            for (const s of got) {
              const k = key(s.face, text);
              asked.delete(k);
              answers.set(k, s);
            }
          },
          () => undefined,
        );
    }
  }

  /** Forgets every answer: the library changed, so coverage may have. */
  function reset(): void {
    round++;
    asked.clear();
    queue = new Map();
    answers.clear();
  }

  watch(() => useLibraryStore().families, reset);

  return { get, want, reset };
});
