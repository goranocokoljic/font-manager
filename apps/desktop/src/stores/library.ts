import { defineStore } from 'pinia';
import { ref, shallowReactive, shallowRef } from 'vue';
import { useClient, type FmClient, type Unsubscribe } from '../api/client';
import type {
  ApiError,
  Event,
  FaceId,
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
  /** Bumped by every activation change; answers to an older one are dropped. */
  let activationRound = 0;
  /** Faces changed by activation rounds that have not finished yet. */
  const unsettled = new Set<FaceId>();
  const pending = new Map<FamilyId, Promise<FaceSummary[]>>();
  /**
   * Bumped for one family when a watcher change makes its faces stale (E51.2): an
   * answer asked before is dropped, as a refresh drops every older answer.
   */
  const familyRound = new Map<FamilyId, number>();
  /** Bumped by every watcher change; an older one's family list is dropped. */
  let watchRound = 0;
  /** Faces removed or changed by watcher changes whose list has not landed yet. */
  const watchTouched = new Set<FaceId>();

  /** Every family, in the order `by` (the API default when `null`). */
  async function listAll(client: FmClient, by: FamilyOrder | null): Promise<FamilySummary[]> {
    const list: FamilySummary[] = [];
    for (let offset = 0; ; offset += PAGE) {
      const page = await client.listFamilies({
        page: { offset, limit: PAGE },
        ...(by ? { order: by } : {}),
      });
      list.push(...page.items);
      if (!page.items.length || list.length >= page.total) break;
    }
    return list;
  }

  /**
   * Fetches the family list in the current order. After a library change it also
   * forgets the faces; a new order alone keeps them, the families did not change.
   */
  async function fetchFamilies(): Promise<void> {
    const mine = ++listGeneration;
    const client = useClient();
    try {
      let list: FamilySummary[];
      let round: number;
      // An activation change during the fetch may have been missed by its pages; fetch again.
      do {
        round = activationRound;
        list = await listAll(client, order.value);
      } while (round !== activationRound && mine === listGeneration);
      if (mine !== listGeneration) return;
      families.value = list;
      if (facesStale) {
        facesStale = false;
        faces.clear();
        failed.clear();
        unsettled.clear();
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
    watchTouched.clear();
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

  /** Asks for the faces of a family; the answer is kept unless something newer superseded it. */
  function request(id: FamilyId): Promise<FaceSummary[]> {
    const mine = generation;
    const round = familyRound.get(id) ?? 0;
    const current = () => mine === generation && round === (familyRound.get(id) ?? 0);
    const asked: Promise<FaceSummary[]> = useClient()
      .getFamily(id)
      .then(
        (family) => {
          if (current()) {
            faces.set(id, family.faces);
            failed.delete(id);
            if (pending.get(id) === asked) pending.delete(id);
          }
          return family.faces;
        },
        (e: unknown) => {
          if (current()) {
            failed.add(id);
            if (pending.get(id) === asked) pending.delete(id);
          }
          throw e;
        },
      );
    pending.set(id, asked);
    return asked;
  }

  /** The faces of a family, fetched once (per library version) however often it is asked. */
  function facesOf(id: FamilyId): Promise<FaceSummary[]> {
    const known = faces.get(id);
    if (known) return Promise.resolve(known);
    return pending.get(id) ?? request(id);
  }

  /** Whether a family's row changed in a way that changes its faces. */
  function facesMoved(before: FamilySummary | undefined, after: FamilySummary): boolean {
    if (!before) return true;
    return (
      before.styleCount !== after.styleCount ||
      JSON.stringify(before.styles ?? null) !== JSON.stringify(after.styles ?? null)
    );
  }

  /**
   * The folder watcher added, removed or changed faces (E51.2): reloads the family list and,
   * in place, the faces of only the families concerned. Every other family keeps its loaded
   * faces, so an expanded family keeps its style rows, the selection keeps them and the
   * view's scroll anchor stays where it was. A family is concerned when it held a removed or
   * changed face, or its row now counts other styles (a face was added to it).
   */
  async function applyWatcherChange(
    removed: readonly FaceId[],
    updated: readonly FaceId[],
  ): Promise<void> {
    // A refresh on its way forgets every face anyway.
    if (facesStale) return refresh();
    const mine = generation;
    const round = ++watchRound;
    const listRound = ++listGeneration;
    const by = order.value;
    // A newer change supersedes this one, so it must also refetch this one's families.
    for (const id of [...removed, ...updated]) watchTouched.add(id);
    const touched = watchTouched;
    // Activation rounds that started before this change; one landing during the fetch may have
    // been missed by its pages, so the list is fetched again (as `fetchFamilies` does).
    let activated: number;
    try {
      let list: FamilySummary[];
      do {
        activated = activationRound;
        list = await listAll(useClient(), by);
      } while (activated !== activationRound && mine === generation && round === watchRound);
      if (mine !== generation || round !== watchRound) return;
      // A new order fetched the list again meanwhile: that answer cannot tell which faces
      // this change made stale, so the library is read afresh.
      if (listRound !== listGeneration) return refresh();
      const before = new Map(families.value.map((f) => [f.id, f]));
      const after = new Map(list.map((f) => [f.id, f]));
      families.value = list;
      error.value = null;
      loaded.value = true;
      const known = new Set<FamilyId>([...faces.keys(), ...pending.keys(), ...failed]);
      for (const id of known) {
        const now = after.get(id);
        const stale =
          !now ||
          facesMoved(before.get(id), now) ||
          (faces.get(id) ?? []).some((f) => touched.has(f.id) || unsettled.has(f.id)) ||
          // A family asked for and not answered yet may hold a changed face.
          (pending.has(id) && touched.size > 0);
        if (!stale) continue;
        familyRound.set(id, (familyRound.get(id) ?? 0) + 1);
        pending.delete(id);
        failed.delete(id);
        if (!now) faces.delete(id);
        // Asked again; the faces on screen stay until the answer arrives.
        else void request(id).catch(() => undefined);
      }
      watchTouched.clear();
      // This change superseded the activation rounds before it (see `refreshActivation`):
      // their faces were asked for again above, with the list that holds their state.
      if (activated === activationRound) unsettled.clear();
    } catch (e) {
      if (mine === generation && round === watchRound) error.value = e as ApiError;
    }
  }

  /**
   * Faces were activated or deactivated: reloads the family summaries (their
   * activation state) and the loaded faces of the families concerned, in place,
   * so expanded families keep their rows and the list keeps its cursor.
   */
  async function refreshActivation(changed: readonly FaceId[]): Promise<void> {
    const mine = generation;
    const round = ++activationRound;
    const by = order.value;
    // A watcher change after this round started (E51.2) reads the list and the faces again,
    // with this round's state in them: an answer of this round must not overwrite them.
    const watch = watchRound;
    const live = () => mine === generation && round === activationRound && watch === watchRound;
    // A new order fetches the list again; an answer in the old order must not win.
    const current = () => live() && by === order.value;
    const client = useClient();
    // A newer round supersedes this one, so it must also refetch this one's faces.
    for (const id of changed) unsettled.add(id);
    try {
      const list = await listAll(client, by);
      if (current()) families.value = list;
      if (!live()) return;
      const stale = [...faces]
        .filter(([, fs]) => fs.some((f) => unsettled.has(f.id)))
        .map(([id]) => id);
      await Promise.all(
        stale.map(async (id) => {
          const family = await client.getFamily(id);
          if (live()) faces.set(id, family.faces);
        }),
      );
      if (live()) unsettled.clear();
    } catch {
      // The list keeps the state it had; the next refresh brings it up to date.
    }
  }

  function onEvent(event: Event): void {
    if (event.type === 'libraryChanged' && event.fromWatcher)
      void applyWatcherChange(event.removed, event.updated);
    else if (event.type === 'libraryChanged' || event.type === 'scanFinished') void refresh();
    else if (event.type === 'activationChanged')
      void refreshActivation([...event.activated, ...event.deactivated]);
  }

  /**
   * Subscribes to library changes (once) and loads the families. Still subscribed with a
   * good list (the library view and a family page hand over to each other): the events
   * kept it current, and a refresh would forget the faces and with them a selected style.
   */
  async function load(): Promise<void> {
    if (unsubscribe && loaded.value && !error.value) return;
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
