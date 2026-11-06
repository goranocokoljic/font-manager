import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { useClient } from '../api/client';
import type { Filters } from '../api/types';
import type { IconName } from '../components/ui/icons';
import { t } from '../i18n';
import { useActivationStore } from './activation';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useSelectionStore } from './selection';

/** What "Recently added" means, in days: the window of fm-core's "Recently" rules. */
export const RECENT_DAYS = 30;

export type LibraryViewId = 'all' | 'recent' | 'activated' | 'favorites';

export interface LibraryView {
  id: LibraryViewId;
  icon: IconName;
  /** What the view sets on the list's filters; the API does the filtering. */
  filters: Filters;
}

/** The rows at the top of the sidebar, in order. */
export const LIBRARY_VIEWS: readonly LibraryView[] = [
  { id: 'all', icon: 'grid', filters: {} },
  { id: 'recent', icon: 'clock', filters: { addedWithinDays: RECENT_DAYS } },
  { id: 'activated', icon: 'plus', filters: { active: true } },
  { id: 'favorites', icon: 'heart', filters: { favorite: true } },
];

/** No view chosen: the filters every view replaces. */
const NO_VIEW: Filters = { collection: null, addedWithinDays: null, active: null, favorite: null };

/**
 * The sidebar's top rows: All fonts, Recently added, Activated and Favorites. Each is a set
 * of list filters, and each count is the API's family count for those filters (`search`),
 * never worked out here.
 */
export const useLibraryViewsStore = defineStore('libraryViews', () => {
  const library = useLibraryStore();
  const search = useSearchStore();
  const activation = useActivationStore();
  const selection = useSelectionStore();
  const counts = ref<Partial<Record<LibraryViewId, number>>>({});
  /** Bumped by every refresh; answers to an older one are dropped. */
  let generation = 0;

  /** The view the list shows, or null when a collection is open. */
  const current = computed<LibraryViewId | null>(() => {
    const f = search.filters;
    if (f.collection != null) return null;
    const on = LIBRARY_VIEWS.find(
      (v) =>
        v.id !== 'all' &&
        Object.entries(v.filters).every(([k, value]) => f[k as keyof Filters] === value),
    );
    return on?.id ?? 'all';
  });

  /** The name of the view the list shows. */
  const currentLabel = computed(() => t(`libraryViews.${current.value ?? 'all'}`));

  /** Shows a view, keeping the filters chosen on top of it, like a collection does. */
  function show(id: LibraryViewId): void {
    const view = LIBRARY_VIEWS.find((v) => v.id === id)!;
    search.setFilters({ ...search.filters, ...NO_VIEW, ...view.filters });
  }

  async function refresh(): Promise<void> {
    const mine = ++generation;
    const page = { offset: 0, limit: 1 };
    const client = useClient();
    try {
      const [everything, recent] = await Promise.all([
        client.search({ filters: {}, page }),
        client.search({ filters: { addedWithinDays: RECENT_DAYS }, page }),
      ]);
      if (mine !== generation) return;
      const yes = (values?: { value: boolean; count: number }[]) =>
        values?.find((c) => c.value)?.count ?? 0;
      counts.value = {
        all: everything.total,
        recent: recent.total,
        activated: yes(everything.counts?.active),
        favorites: yes(everything.counts?.favorite),
      };
    } catch {
      // The counts keep their values; the next change reads them again.
    }
  }

  // A scan moves every count, an activation and a favorite one of them.
  watch(
    [() => library.families, () => activation.count, () => selection.detail?.favorite],
    () => void refresh(),
  );

  return { counts, current, currentLabel, show, refresh };
});
