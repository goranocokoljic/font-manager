import { defineStore } from 'pinia';
import { computed, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  FaceSummary,
  FamilyId,
  PinInstanceRequest,
  VirtualStyle,
  VirtualStyleId,
} from '../api/types';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';

/** A virtual style with the summary of the variable face it is an instance of. */
export interface PinnedStyle {
  style: VirtualStyle;
  face: FaceSummary;
}

/**
 * The virtual styles (E10.2) of the collection the list shows: the list puts
 * each under its face's family, as a row of its own previewed at its
 * coordinates. Read again when the open collection or the library changes, and
 * after a pin. With no collection open there are none.
 */
export const useVirtualStylesStore = defineStore('virtualStyles', () => {
  const search = useSearchStore();
  const library = useLibraryStore();
  const styles = shallowRef<PinnedStyle[]>([]);
  /** Bumped by every read; an answer to an older read is dropped. */
  let round = 0;

  const collection = computed(() => search.filters.collection ?? null);

  async function refresh(): Promise<void> {
    const mine = ++round;
    const id = collection.value;
    if (id == null) {
      styles.value = [];
      return;
    }
    try {
      const client = useClient();
      const list = await client.listVirtualStyles({ collection: id });
      const ids = [...new Set(list.map((v) => v.face))];
      const details = await Promise.all(ids.map((f) => client.getFace(f)));
      const faces = new Map(details.map((d) => [d.face.id, d.face]));
      if (mine !== round) return;
      styles.value = list.flatMap((style) => {
        const face = faces.get(style.face);
        return face ? [{ style, face }] : [];
      });
    } catch {
      // A smart collection holds no virtual styles; a failed read shows none.
      if (mine === round) styles.value = [];
    }
  }

  watch([collection, () => library.families], () => void refresh(), { immediate: true });

  /** The pinned styles by family, in the API's order (A to Z). */
  const byFamily = computed(() => {
    const map = new Map<FamilyId, PinnedStyle[]>();
    for (const p of styles.value) {
      const list = map.get(p.face.family) ?? [];
      list.push(p);
      map.set(p.face.family, list);
    }
    return map;
  });

  /** Pins an instance through fm-api and reads the open collection's styles again. */
  async function pin(req: PinInstanceRequest): Promise<VirtualStyle> {
    const style = await useClient().pinInstance(req);
    await refresh();
    return style;
  }

  /** Deletes a virtual style; its face stays. */
  async function remove(id: VirtualStyleId): Promise<void> {
    await useClient().deleteVirtualStyle(id);
    await refresh();
  }

  return { styles, byFamily, refresh, pin, remove };
});
