import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, ImportSetResponse, ProjectSet, SetId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useToastStore } from './toast';

/**
 * The project sets of the sidebar and the current set: the one the user picked
 * there, which "Deactivate everything not in the current set" keeps active.
 * fm-api has no set events yet, so the list is read on load.
 */
export const useSetsStore = defineStore('sets', () => {
  const sets = ref<ProjectSet[]>([]);
  const error = ref<ApiError | null>(null);
  const currentId = ref<SetId | null>(null);
  /** The last "Import set…": the new set and the fonts this library does not have. */
  const imported = ref<ImportSetResponse | null>(null);
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

  /** "Export set…": saves the set file of `id` where the user picks (E28.2). */
  async function exportSet(id: SetId): Promise<void> {
    const toast = useToastStore();
    const name = sets.value.find((s) => s.id === id)?.name ?? '';
    // The suggested file name; characters no file system allows become "-".
    const fileName = `${name || 'Set'}.fmset.json`.replace(/[\\/:*?"<>|]/g, '-');
    try {
      const saved = await useClient().saveSetFile(id, fileName);
      if (saved) toast.show(t('sets.saved', { count: saved.fonts }));
    } catch (e) {
      toast.show(errorMessage(e));
    }
  }

  /**
   * "Import set…": reads a set file the user picks (E28.2) and keeps the answer in
   * `imported`, so the sidebar lists the fonts this library does not have.
   */
  async function importSet(): Promise<void> {
    try {
      const got = await useClient().loadSetFile();
      if (!got) return;
      imported.value = got;
      await load();
    } catch (e) {
      useToastStore().show(errorMessage(e));
    }
  }

  function dismissImport(): void {
    imported.value = null;
  }

  return { sets, error, current, imported, load, pick, exportSet, importSet, dismissImport };
});
