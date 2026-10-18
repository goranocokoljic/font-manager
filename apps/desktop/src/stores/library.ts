import { defineStore } from 'pinia';
import { ref, shallowReactive, shallowRef } from 'vue';
import { useClient, type FmClient, type Unsubscribe } from '../api/client';
import type { ApiError, Event, FaceId, FaceSummary, FamilyId, FamilySummary } from '../api/types';

/** Families fetched per `listFamilies` call. */
const PAGE = 500;

/**
 * The families of the library, in the API's order, and the faces of the families
 * the list has needed so far. Only family summaries load up front; the faces of a
 * family are fetched when its row comes on screen or it is expanded, so 10,000
 * families cost 20 requests, not 10,000.
 */
export const useLibraryStore = defineStore('library', () => {
  const families = shallowRef<FamilySummary[]>([]);
  /** Faces by family, filled by `facesOf`. Reading one key tracks only that family. */
  const faces = shallowReactive(new Map<FamilyId, FaceSummary[]>());
  /** Families whose faces could not be fetched; not asked again until a refresh. */
  const failed = shallowReactive(new Set<FamilyId>());
  const loaded = ref(false);
  const error = ref<ApiError | null>(null);
  let unsubscribe: Promise<Unsubscribe> | undefined;
  /** Bumped by every refresh; answers to an older one are dropped. */
  let generation = 0;
  /** Bumped by every activation change; answers to an older one are dropped. */
  let activationRound = 0;
  const pending = new Map<FamilyId, Promise<FaceSummary[]>>();

  async function listAll(client: FmClient): Promise<FamilySummary[]> {
    const list: FamilySummary[] = [];
    for (let offset = 0; ; offset += PAGE) {
      const page = await client.listFamilies({ page: { offset, limit: PAGE } });
      list.push(...page.items);
      if (!page.items.length || list.length >= page.total) break;
    }
    return list;
  }

  async function refresh(): Promise<void> {
    const mine = ++generation;
    const client = useClient();
    try {
      const list = await listAll(client);
      if (mine !== generation) return;
      families.value = list;
      faces.clear();
      failed.clear();
      pending.clear();
      error.value = null;
    } catch (e) {
      if (mine === generation) error.value = e as ApiError;
    } finally {
      if (mine === generation) loaded.value = true;
    }
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

  /**
   * Faces were activated or deactivated: reloads the family summaries (their
   * activation state) and the loaded faces of the families concerned, in place,
   * so expanded families keep their rows and the list keeps its cursor.
   */
  async function refreshActivation(changed: readonly FaceId[]): Promise<void> {
    const mine = generation;
    const round = ++activationRound;
    const current = () => mine === generation && round === activationRound;
    const client = useClient();
    const ids = new Set(changed);
    try {
      const list = await listAll(client);
      if (!current()) return;
      families.value = list;
      const stale = [...faces].filter(([, fs]) => fs.some((f) => ids.has(f.id))).map(([id]) => id);
      await Promise.all(
        stale.map(async (id) => {
          const family = await client.getFamily(id);
          if (current()) faces.set(id, family.faces);
        }),
      );
    } catch {
      // The list keeps the state it had; the next refresh brings it up to date.
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'libraryChanged' || event.type === 'scanFinished') void refresh();
    else if (event.type === 'activationChanged')
      void refreshActivation([...event.activated, ...event.deactivated]);
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

  return { families, faces, failed, loaded, error, facesOf, load, stop, refresh };
});
