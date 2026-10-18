import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Collection, CollectionId, FaceId, FamilyId } from '../api/types';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';

/** A collection as the sidebar lists it: with its depth and its place among its siblings. */
export interface CollectionRow {
  collection: Collection;
  /** 0 at the top level. */
  depth: number;
  /** Its index among its siblings, 0 first. */
  position: number;
  /** How many siblings it has, itself included. */
  siblings: number;
}

/** Faces and whole families, as a drag or the palette hands them to a collection. */
export interface FontPick {
  faces: FaceId[];
  families: FamilyId[];
}

/**
 * The sidebar's collections, in the API's order (parents before children), with
 * their face counts. Every change asks the API and lists again; the counts come
 * from the API, never from the UI. The open collection is the list's
 * `collection` filter.
 */
export const useCollectionsStore = defineStore('collections', () => {
  const library = useLibraryStore();
  const search = useSearchStore();
  const collections = shallowRef<Collection[]>([]);
  const error = ref<ApiError | null>(null);

  async function attempt<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await work();
      error.value = null;
      return result;
    } catch (e) {
      error.value = e as ApiError;
      return undefined;
    }
  }

  async function refresh(): Promise<void> {
    await attempt(async () => {
      collections.value = await useClient().listCollections();
    });
  }

  const rows = computed<CollectionRow[]>(() => {
    const depth = new Map<CollectionId, number>();
    const siblings = new Map<CollectionId | null, number>();
    for (const c of collections.value) siblings.set(c.parent, (siblings.get(c.parent) ?? 0) + 1);
    const seen = new Map<CollectionId | null, number>();
    return collections.value.map((c) => {
      const d = c.parent == null ? 0 : (depth.get(c.parent) ?? 0) + 1;
      depth.set(c.id, d);
      const position = seen.get(c.parent) ?? 0;
      seen.set(c.parent, position + 1);
      return { collection: c, depth: d, position, siblings: siblings.get(c.parent) ?? 1 };
    });
  });

  /** The collection the list shows, or null for all fonts. */
  const openId = computed(() => search.filters.collection ?? null);
  const open = computed(() => collections.value.find((c) => c.id === openId.value) ?? null);

  /** Shows one collection in the list (or all fonts for `null`), keeping the other filters. */
  function show(id: CollectionId | null): void {
    search.setFilters({ ...search.filters, collection: id });
  }

  /** Creates a collection, last among its siblings; true when it was made. */
  async function create(name: string, parent: CollectionId | null = null): Promise<boolean> {
    const made = await attempt(() => useClient().createCollection({ name, parent }));
    if (made) await refresh();
    return !!made;
  }

  async function rename(collection: CollectionId, name: string): Promise<boolean> {
    const done = await attempt(() => useClient().renameCollection({ collection, name }));
    if (done) await refresh();
    return !!done;
  }

  /** Puts a collection under `parent` (null: the top level) at `position` among its siblings. */
  async function move(
    collection: CollectionId,
    parent: CollectionId | null,
    position: number,
  ): Promise<void> {
    const done = await attempt(() =>
      useClient().moveCollection({ collection, parent, position: Math.max(0, position) }),
    );
    if (done) await refresh();
  }

  /** Deletes a collection and its children; the fonts stay in the library. */
  async function remove(collection: CollectionId): Promise<void> {
    const done = await attempt(async () => {
      await useClient().deleteCollection(collection);
      return true;
    });
    if (!done) return;
    await refresh();
    // The open collection went (itself or with its parent): back to all fonts.
    if (openId.value != null && !collections.value.some((c) => c.id === openId.value)) show(null);
  }

  /** Adds faces and whole families to a collection; a face can be in many. */
  async function add(collection: CollectionId, pick: FontPick): Promise<void> {
    if (!pick.faces.length && !pick.families.length) return;
    const done = await attempt(() =>
      useClient().addToCollection({ collection, faces: pick.faces, families: pick.families }),
    );
    if (!done) return;
    await refresh();
    // The open collection gained fonts: the list must show them.
    if (openId.value === collection) await search.run();
  }

  // A scan or a source change moves the counts.
  watch(
    () => library.families,
    () => void refresh(),
  );

  return {
    collections,
    rows,
    error,
    openId,
    open,
    refresh,
    show,
    create,
    rename,
    move,
    remove,
    add,
  };
});
