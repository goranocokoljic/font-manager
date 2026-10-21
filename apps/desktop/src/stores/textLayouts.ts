import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Layout, LayoutPreset } from '../api/types';

/** The body-text layout presets, as fm-api returns them (loaded once). */
export const useTextLayoutsStore = defineStore('textLayouts', () => {
  const presets = ref<Layout[]>([]);
  const error = ref<ApiError | null>(null);
  let pending: Promise<void> | null = null;

  /** Fetches the presets; a second call while one is in flight waits for it. */
  function load(): Promise<void> {
    if (presets.value.length) return Promise.resolve();
    pending ??= useClient()
      .layoutPresets()
      .then(
        (list) => {
          presets.value = list;
          error.value = null;
        },
        (e: unknown) => {
          error.value = e as ApiError;
        },
      )
      .finally(() => {
        pending = null;
      });
    return pending;
  }

  /** The layout of a preset, once loaded. */
  const byPreset = (preset: LayoutPreset): Layout | null =>
    presets.value.find((l) => l.preset === preset) ?? null;

  return { presets, error, load, byPreset };
});
