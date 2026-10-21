import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary } from '../api/types';
import { errorMessage, t } from '../i18n';
import type { ListRow } from '../list/rows';
import { useCollectionsStore } from './collections';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useToastStore } from './toast';

const fonts = (n: number) => t('quickCollection.fonts', { count: n });

/**
 * The Quick Collection (E13.3): fonts gathered with one key while browsing, then
 * reviewed in the center and kept as a manual collection or a tag. The API keeps it
 * (in the index, so it outlives a restart) and answers every change with the whole
 * list; this store only holds the last answer and whether the review is open.
 */
export const useQuickCollectionStore = defineStore('quickCollection', () => {
  const library = useLibraryStore();
  const faces = shallowRef<FaceSummary[]>([]);
  /** The review is shown in the center instead of the font list. */
  const open = ref(false);
  const error = ref<ApiError | null>(null);

  const ids = computed(() => new Set(faces.value.map((f) => f.id)));
  const families = computed(() => new Set(faces.value.map((f) => f.family)));
  const count = computed(() => faces.value.length);

  async function attempt<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await work();
      error.value = null;
      return result;
    } catch (e) {
      error.value = e as ApiError;
      useToastStore().show(errorMessage(error.value));
      return undefined;
    }
  }

  async function load(): Promise<void> {
    const got = await attempt(() => useClient().quickCollection());
    if (got) faces.value = got.faces;
  }

  /** True when the row's face (a style row) or one of its family's faces is in it. */
  function holds(row: ListRow): boolean {
    if (row.kind === 'face') return !row.virtual && ids.value.has(row.face.id);
    return families.value.has(row.family.id);
  }

  /** The faces of list rows: a style row's face, every face of a family row. */
  async function facesOfRows(rows: readonly ListRow[]): Promise<FaceId[]> {
    const out: FaceId[] = [];
    for (const row of rows) {
      if (row.kind === 'face') out.push(row.face.id);
      else out.push(...(await library.facesOf(row.family.id)).map((f) => f.id));
    }
    return [...new Set(out)];
  }

  /**
   * The one-key toggle: when every face of `rows` is in the Quick Collection they
   * leave it, else the ones not in it yet are added. Told in a toast.
   */
  async function toggleRows(rows: readonly ListRow[]): Promise<void> {
    const picked = await attempt(() => facesOfRows(rows));
    if (!picked?.length) return;
    const leaving = picked.every((id) => ids.value.has(id));
    const got = await attempt(() =>
      leaving
        ? useClient().removeFromQuickCollection({ faces: picked })
        : useClient().addToQuickCollection({ faces: picked.filter((id) => !ids.value.has(id)) }),
    );
    if (!got) return;
    faces.value = got.faces;
    useToastStore().show(
      t(leaving ? 'quickCollection.removed' : 'quickCollection.added', {
        fonts: fonts(picked.length),
        total: fonts(got.faces.length),
      }),
    );
  }

  /** Drops one face from the Quick Collection; the font stays in the library. */
  async function drop(face: FaceId): Promise<void> {
    const got = await attempt(() => useClient().removeFromQuickCollection({ faces: [face] }));
    if (got) faces.value = got.faces;
  }

  async function clear(): Promise<void> {
    const done = await attempt(async () => {
      await useClient().clearQuickCollection();
      return true;
    });
    if (done) faces.value = [];
  }

  /**
   * "Save as collection": a new manual collection named `name` holding exactly the
   * fonts left in the Quick Collection. True when it was made.
   */
  async function saveAsCollection(name: string): Promise<boolean> {
    const kept = faces.value.map((f) => f.id);
    if (!kept.length) return false;
    const made = await attempt(async () => {
      const client = useClient();
      const collection = await client.createCollection({ name: name.trim() });
      return client.addToCollection({ collection: collection.id, faces: kept, families: [] });
    });
    if (!made) return false;
    await useCollectionsStore().refresh();
    useToastStore().show(
      t('quickCollection.saved', { fonts: fonts(kept.length), name: made.name }),
    );
    return true;
  }

  /** "Tag all": puts tag `name` on every font left in the Quick Collection. */
  async function tagAll(name: string): Promise<boolean> {
    const kept = faces.value.map((f) => f.id);
    if (!kept.length) return false;
    const tagged = await attempt(() => useClient().tagFaces({ faces: kept, tags: [name.trim()] }));
    if (!tagged) return false;
    useToastStore().show(
      t('quickCollection.tagged', {
        fonts: fonts(kept.length),
        name: tagged[0]?.name ?? name.trim(),
      }),
    );
    return true;
  }

  function show(): void {
    open.value = true;
  }

  function close(): void {
    open.value = false;
  }

  // Opening a collection or All fonts (they set the list's filters) ends the review.
  watch(
    () => useSearchStore().filters,
    () => close(),
  );

  // A scan or a paused source hides or shows faces; list again.
  watch(
    () => library.families,
    () => void load(),
  );

  return {
    faces,
    open,
    error,
    ids,
    count,
    load,
    holds,
    toggleRows,
    drop,
    clear,
    saveAsCollection,
    tagAll,
    show,
    close,
  };
});
