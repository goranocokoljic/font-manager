import { defineStore } from 'pinia';
import { computed, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FoundryInfo } from '../api/types';
import { t } from '../i18n';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useToastStore } from './toast';

/** How many foundries the sidebar lists before "All N foundries…" (E50.12). */
export const FOUNDRIES_SHOWN = 10;

/**
 * The most families first, then A to Z: the order of the sidebar's short list. Only the
 * API's counts are read; the unknown foundry is left out (it is no foundry to show off).
 */
export function largestFoundries(list: readonly FoundryInfo[], n: number): FoundryInfo[] {
  return list
    .filter((f) => !f.unknown)
    .sort((a, b) => b.families - a.families || a.name.localeCompare(b.name))
    .slice(0, n);
}

/**
 * The library's foundries (E50.12) for the sidebar: the API's list under the normalised
 * names (E50.11), the ten largest of them, and the user's merges with an undo.
 */
export const useFoundriesStore = defineStore('foundries', () => {
  const library = useLibraryStore();
  const search = useSearchStore();
  const toast = useToastStore();
  /** Every foundry, A to Z, the unknown one last, as `listFoundries` gives them. */
  const list = shallowRef<FoundryInfo[]>([]);
  const error = shallowRef<ApiError | null>(null);
  let generation = 0;

  /** The foundries with a name: the count the folded heading shows. */
  const named = computed(() => list.value.filter((f) => !f.unknown));
  /** The short list of the sidebar. */
  const largest = computed(() => largestFoundries(list.value, FOUNDRIES_SHOWN));

  /** The listed foundry called `name`, without case (the API compares so). */
  function find(name: string): FoundryInfo | undefined {
    const key = name.toLowerCase();
    return list.value.find((f) => f.name.toLowerCase() === key);
  }

  function take(next: FoundryInfo[]): void {
    generation++;
    list.value = next;
    error.value = null;
  }

  async function refresh(): Promise<void> {
    const mine = ++generation;
    try {
      const next = await useClient().listFoundries();
      if (mine === generation) take(next);
    } catch (e) {
      if (mine === generation) error.value = e as ApiError;
    }
  }

  /** Undoes the merge of `from`: listed under its own name again. */
  async function unmerge(from: string): Promise<void> {
    try {
      take(await useClient().unmergeFoundry({ name: from }));
      await search.run();
      toast.show(t('foundries.unmerged', { name: from }));
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  /**
   * Merges `from` into `into` (E50.11's override); a toast offers the undo. A filter on
   * `from` follows it to `into`, so the wall keeps those families. False when it failed.
   */
  async function merge(from: string, into: string): Promise<boolean> {
    try {
      take(await useClient().mergeFoundries({ from, into }));
    } catch (e) {
      error.value = e as ApiError;
      return false;
    }
    if (search.isFoundry(from)) search.setFoundry(into);
    else await search.run();
    toast.show(t('foundries.merged', { from, into }), {
      label: t('common.undo'),
      run: () => unmerge(from),
    });
    return true;
  }

  // A scan or a removal changes the foundries and their counts.
  watch(
    () => library.families,
    () => void refresh(),
  );

  return { list, error, named, largest, find, refresh, merge, unmerge };
});
