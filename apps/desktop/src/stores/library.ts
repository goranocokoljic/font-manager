import { defineStore } from 'pinia';
import { ref, shallowReactive, shallowRef } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  ApiError,
  Event,
  FaceSummary,
  FamilyId,
  FamilyOrder,
  FamilySummary,
} from '../api/types';

/** Families fetched per `listFamilies` call. */
const PAGE = 500;

/**
 * The families of the library, in the API's order, and the faces of the families
 * the list has needed so far. Only family summaries load up front; the faces of a
 * family are fetched when its row comes on screen or it is expanded, so 10,000
 * families cost 20 requests, not 10,000. `order` is the API order the families
 * come in (the detail list's sort column); `null` is the API's default.
 */
export const useLibraryStore = defineStore('library', () => {
  const families = shallowRef<FamilySummary[]>([]);
  /** Faces by family, filled by `facesOf`. Reading one key tracks only that family. */
  const faces = shallowReactive(new Map<FamilyId, FaceSummary[]>());
  /** Families whose faces could not be fetched; not asked again until a refresh. */
  const failed = shallowReactive(new Set<FamilyId>());
  const loaded = ref(false);
  const error = ref<ApiError | null>(null);
  const order = shallowRef<FamilyOrder | null>(null);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every refresh; face answers to an older one are dropped. */
  let generation = 0;
  /** Bumped by every family list request; answers to an older one are dropped. */
  let listGeneration = 0;
  /** The library changed: the next family list that arrives also forgets the faces. */
  let facesStale = false;
  const pending = new Map<FamilyId, Promise<FaceSummary[]>>();

  /**
   * Fetches the family list in the current order. After a library change it also
   * forgets the faces; a new order alone keeps them, the families did not change.
   */
  async function fetchFamilies(): Promise<void> {
    const mine = ++listGeneration;
    const client = useClient();
    const by = order.value;
    try {
      const list: FamilySummary[] = [];
      for (let offset = 0; ; offset += PAGE) {
        const page = await client.listFamilies({
          page: { offset, limit: PAGE },
          ...(by ? { order: by } : {}),
        });
        list.push(...page.items);
        if (!page.items.length || list.length >= page.total) break;
      }
      if (mine !== listGeneration) return;
      families.value = list;
      if (facesStale) {
        facesStale = false;
        faces.clear();
        failed.clear();
        pending.clear();
      }
      error.value = null;
    } catch (e) {
      if (mine === listGeneration) error.value = e as ApiError;
    } finally {
      if (mine === listGeneration) loaded.value = true;
    }
  }

  async function refresh(): Promise<void> {
    generation += 1;
    facesStale = true;
    await fetchFamilies();
  }

  /** Orders the families by `next`; fetches them again once the store has loaded. */
  async function setOrder(next: FamilyOrder | null): Promise<void> {
    const now = order.value;
    if (now?.by === next?.by && !!now?.descending === !!next?.descending) return;
    order.value = next;
    if (unsubscribe) await fetchFamilies();
  }

  /** The faces of a family, fetched once (per library version) however often it is asked. */
  function facesOf(id: FamilyId): Promise<FaceSummary[]> {
    const known = faces.get(id);
    if (known) return Promise.resolve(known);
    let request = pending.get(id);
    if (!request) {
      const mine = generation;
      request = useClient()
        .getFamily(id)
        .then(
          (family) => {
            if (mine === generation) {
              faces.set(id, family.faces);
              pending.delete(id);
            }
            return family.faces;
          },
          (e: unknown) => {
            if (mine === generation) {
              failed.add(id);
              pending.delete(id);
            }
            throw e;
          },
        );
      pending.set(id, request);
    }
    return request;
  }

  function onEvent(event: Event): void {
    if (event.type === 'libraryChanged' || event.type === 'scanFinished') void refresh();
  }

  /** Subscribes to library changes (once) and loads the families. */
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

  return {
    families,
    faces,
    failed,
    loaded,
    error,
    order,
    facesOf,
    load,
    stop,
    refresh,
    setOrder,
  };
});
