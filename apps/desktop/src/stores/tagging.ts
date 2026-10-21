import { defineStore } from 'pinia';
import { computed, ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, SelectionTag, Tag, TagId } from '../api/types';
import { fuzzyMatch } from '../keyboard/fuzzy';
import { pickOf } from '../list/drag';
import type { FontPick } from './collections';
import { useFontListStore } from './fontList';
import { sameTagName, useTagsStore } from './tags';
import { useToastStore } from './toast';

/** A tag on every font of the selection, on some of them, or on none. */
export type TagState = 'all' | 'some' | 'none';

/** One tag as the multi-tagging panel lists it. */
export interface TagRow {
  tag: Tag;
  /** What the panel shows: the API's answer with the staged toggle on top. */
  state: TagState;
  /** Fonts of the selection carrying it now, as the API counted them. */
  carried: number;
  /** True when Space changed it and Apply has not run yet. */
  staged: boolean;
  /** Matched character positions in the name, for highlighting. */
  indices: number[];
}

const isEmpty = (p: FontPick) => !p.faces.length && !p.families.length;
const fonts = (n: number) => (n === 1 ? '1 font' : `${n} fonts`);

/**
 * Tagging many fonts at once (E13.4): the multi-tagging panel for the list's selection,
 * and the Tags group's drag state. The API tells which tags the fonts carry and on how
 * many of them (`selectionTags`); this store keeps that answer and the toggles the user
 * staged, and sends them all in one `tagFaces` and one `untagFaces` call.
 */
export const useTaggingStore = defineStore('tagging', () => {
  const fontList = useFontListStore();
  const tags = useTagsStore();

  // ---- the panel ----
  const open = ref(false);
  const query = ref('');
  /** The fonts the panel acts on, as list rows name them (faces and whole families). */
  const pick = shallowRef<FontPick>({ faces: [], families: [] });
  /** Every face of `pick`, each once (the API resolves the families). */
  const faces = shallowRef<FaceId[]>([]);
  const carried = shallowRef<Map<TagId, SelectionTag>>(new Map());
  /** Staged toggles: true puts the tag on every font, false takes it off every font. */
  const staged = shallowRef<Map<TagId, boolean>>(new Map());
  const error = ref<ApiError | null>(null);
  const busy = ref(false);

  /** The selected rows of the list, or the cursor row when nothing is selected. */
  const selectionPick = computed<FontPick>(() => {
    const selected = fontList.selection.selected;
    const cursor = fontList.cursor;
    return pickOf(selected.size ? selected : cursor ? [cursor] : []);
  });

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

  /** Asks the API which tags the panel's fonts carry. */
  async function load(): Promise<boolean> {
    const got = await attempt(() =>
      useClient().selectionTags({ faces: pick.value.faces, families: pick.value.families }),
    );
    if (!got) return false;
    faces.value = got.faces;
    carried.value = new Map(got.tags.map((t) => [t.tag, t]));
    return true;
  }

  /** Opens the panel for `target` (the list's selection by default). */
  async function openFor(target: FontPick = selectionPick.value): Promise<void> {
    if (isEmpty(target)) return;
    pick.value = target;
    query.value = '';
    staged.value = new Map();
    error.value = null;
    await tags.refresh();
    if (await load()) open.value = true;
  }

  /** Closes the panel; staged toggles are dropped. */
  function close(): void {
    open.value = false;
    staged.value = new Map();
    query.value = '';
  }

  function baseState(id: TagId): TagState {
    const t = carried.value.get(id);
    return !t ? 'none' : t.all ? 'all' : 'some';
  }

  function stateOf(id: TagId): TagState {
    const s = staged.value.get(id);
    return s === undefined ? baseState(id) : s ? 'all' : 'none';
  }

  /**
   * Space on a tag: none → all → none; all → none → all; some → all → none → some
   * (the toggles go back to what the fonts carry).
   */
  function toggle(id: TagId): void {
    const next = new Map(staged.value);
    const base = baseState(id);
    const s = next.get(id);
    if (s === undefined) next.set(id, base !== 'all');
    else if (s && base === 'some') next.set(id, false);
    else next.delete(id);
    staged.value = next;
  }

  /** Every tag, filtered and ranked by the fuzzy query (A to Z when there is none). */
  const rows = computed<TagRow[]>(() => {
    const scored: { row: TagRow; score: number; order: number }[] = [];
    tags.tags.forEach((tag, order) => {
      const m = fuzzyMatch(query.value, tag.name);
      if (!m) return;
      const row: TagRow = {
        tag,
        state: stateOf(tag.id),
        carried: carried.value.get(tag.id)?.faceCount ?? 0,
        staged: staged.value.has(tag.id),
        indices: m.indices,
      };
      scored.push({ row, score: m.score, order });
    });
    return scored.sort((a, b) => b.score - a.score || a.order - b.order).map((s) => s.row);
  });

  /** The typed name when no tag has it (in the API's sense of the same name). */
  const newName = computed(() => {
    const name = query.value.trim();
    return name && !tags.tags.some((t) => sameTagName(t.name, name)) ? name : null;
  });

  const pending = computed(() => staged.value.size);

  /** Enter on a name no tag has: makes the tag on every font of the panel at once. */
  async function create(): Promise<boolean> {
    const name = newName.value;
    if (!name || busy.value) return false;
    busy.value = true;
    try {
      const made = await attempt(() =>
        useClient().tagFaces({
          faces: pick.value.faces,
          families: pick.value.families,
          tags: [name],
        }),
      );
      if (!made) return false;
      query.value = '';
      await tags.refresh();
      await load();
      return true;
    } finally {
      busy.value = false;
    }
  }

  /** Sends every staged toggle: one `tagFaces` and one `untagFaces` call, then closes. */
  async function apply(): Promise<boolean> {
    if (busy.value) return false;
    const attach = [...staged.value].filter(([, on]) => on).map(([id]) => id);
    const detach = [...staged.value].filter(([, on]) => !on).map(([id]) => id);
    if (!attach.length && !detach.length) {
      close();
      return true;
    }
    busy.value = true;
    try {
      const names = attach.map((id) => tags.nameOf(id));
      const ok = await attempt(async () => {
        if (names.length)
          await useClient().tagFaces({
            faces: pick.value.faces,
            families: pick.value.families,
            tags: names,
          });
        if (detach.length) await useClient().untagFaces({ faces: faces.value, tags: detach });
        return true;
      });
      if (!ok) return false;
      const n = faces.value.length;
      const parts = [
        attach.length ? `added ${attach.length === 1 ? '1 tag' : `${attach.length} tags`}` : '',
        detach.length ? `removed ${detach.length === 1 ? '1 tag' : `${detach.length} tags`}` : '',
      ].filter(Boolean);
      const said = parts.join(' and ');
      useToastStore().show(`${said[0]!.toUpperCase()}${said.slice(1)} on ${fonts(n)}.`);
      close();
      await tags.refresh();
      return true;
    } finally {
      busy.value = false;
    }
  }

  // ---- the Tags group of the sidebar ----
  /** The Tags group is folded (a view preference of this session). */
  const folded = ref(false);
  /** A font drag is under way. */
  const dragging = ref(false);
  /** The tags at least one dragged font carries, for highlighting. */
  const dragCarried = shallowRef<Set<TagId>>(new Set());
  let foldedBeforeDrag = false;
  let dragToken = 0;

  /** A font drag started: unfold the Tags group and find the tags the fonts carry. */
  async function startDrag(dragged: FontPick): Promise<void> {
    const token = ++dragToken;
    if (!dragging.value) foldedBeforeDrag = folded.value;
    dragging.value = true;
    folded.value = false;
    dragCarried.value = new Set();
    if (isEmpty(dragged)) return;
    const got = await attempt(() =>
      useClient().selectionTags({ faces: dragged.faces, families: dragged.families }),
    );
    if (got && token === dragToken && dragging.value)
      dragCarried.value = new Set(got.tags.map((t) => t.tag));
  }

  /** The drag ended (dropped anywhere or cancelled): fold back if it was folded. */
  function endDrag(): void {
    if (!dragging.value) return;
    dragToken++;
    dragging.value = false;
    dragCarried.value = new Set();
    folded.value = foldedBeforeDrag;
  }

  /** A font drop on tag `name`: tags every dropped font, whole families included. */
  async function dropOn(name: string, dropped: FontPick): Promise<boolean> {
    if (isEmpty(dropped)) return false;
    const made = await attempt(() =>
      useClient().tagFaces({ faces: dropped.faces, families: dropped.families, tags: [name] }),
    );
    if (!made) {
      if (error.value) useToastStore().show(error.value.message);
      return false;
    }
    const tag = made[0];
    useToastStore().show(`Tagged “${tag?.name ?? name}” (${fonts(tag?.faceCount ?? 0)}).`);
    await tags.refresh();
    return true;
  }

  return {
    open,
    query,
    pick,
    faces,
    error,
    busy,
    selectionPick,
    rows,
    newName,
    pending,
    openFor,
    close,
    stateOf,
    toggle,
    create,
    apply,
    folded,
    dragging,
    dragCarried,
    startDrag,
    endDrag,
    dropOn,
  };
});
