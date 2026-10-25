import { defineStore } from 'pinia';
import { shallowRef } from 'vue';
import type { FaceSummary } from '../api/types';

/** Quick look (Space): one face in a large preview over the list. */
export const useQuickLookStore = defineStore('quickLook', () => {
  const face = shallowRef<FaceSummary | null>(null);

  function open(f: FaceSummary): void {
    face.value = f;
  }

  function close(): void {
    face.value = null;
  }

  /** Space opens the quick look on `f`, and closes it when it is open. */
  function toggle(f: FaceSummary): void {
    if (face.value) close();
    else open(f);
  }

  return { face, open, close, toggle };
});
