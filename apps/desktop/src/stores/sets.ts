import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, ProjectSet, SetId } from '../api/types';

/**
 * The project sets of the sidebar and the current set: the one the user picked
 * there, which "Deactivate everything not in the current set" keeps active.
 * fm-api has no set events yet, so the list is read on load.
 */
export const useSetsStore = defineStore('sets', () => {
  const sets = ref<ProjectSet[]>([]);
  const error = ref<ApiError | null>(null);
  const currentId = ref<SetId | null>(null);
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  const current = computed(() => sets.value.find((s) => s.id === currentId.value) ?? null);

  async function load(): Promise<void> {
    const mine = ++round;
    try {
      const list = await useClient().listSets();
      if (mine !== round) return;
      sets.value = list;
      error.value = null;
    } catch (e) {
      if (mine === round) error.value = e as ApiError;
    }
  }

  /** Makes `id` the current set; picking the current set again clears it. */
  function pick(id: SetId): void {
    currentId.value = currentId.value === id ? null : id;
  }

  return { sets, error, current, load, pick };
});
