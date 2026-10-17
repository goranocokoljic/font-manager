import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Event, FaceSummary } from '../api/types';

/** Families fetched per `listFamilies` call. */
const PAGE = 500;

/**
 * The faces the preview list shows, family by family in the API's order.
 * Interim feed for E4.1: the virtualized family list (E5.2) replaces it with
 * paged families that expand to their styles.
 */
export const useLibraryStore = defineStore('library', () => {
  const faces = ref<FaceSummary[]>([]);
  const loaded = ref(false);
  const error = ref<ApiError | null>(null);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every refresh; an older refresh that ends late is dropped. */
  let generation = 0;

  async function refresh(): Promise<void> {
    const mine = ++generation;
    const client = useClient();
    try {
      const families = [];
      for (let offset = 0; ; offset += PAGE) {
        const page = await client.listFamilies({ page: { offset, limit: PAGE } });
        families.push(...page.items);
        if (!page.items.length || families.length >= page.total) break;
      }
      const all = await Promise.all(families.map((f) => client.getFamily(f.id)));
      if (mine !== generation) return;
      faces.value = all.flatMap((f) => f.faces);
      error.value = null;
    } catch (e) {
      if (mine === generation) error.value = e as ApiError;
    } finally {
      if (mine === generation) loaded.value = true;
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'libraryChanged' || event.type === 'scanFinished') void refresh();
  }

  /** Subscribes to library changes (once) and loads the faces. */
  async function load(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
    await refresh();
  }

  /** Stops listening to library changes. */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return { faces, loaded, error, load, stop, refresh };
});
