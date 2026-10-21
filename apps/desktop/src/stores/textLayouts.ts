import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Layout, LayoutEntry, LayoutVersion, SavedLayout } from '../api/types';

/**
 * The body-text layouts as template files (fm-api `LayoutService`): the list of files, the
 * open one as saved, and the editor's draft of it. Saving, duplicating, reverting and
 * restoring go through the client; the core keeps the files and their versions.
 */
export const useTextLayoutsStore = defineStore('textLayouts', () => {
  const entries = ref<LayoutEntry[]>([]);
  /** The open layout as its file holds it. */
  const saved = ref<SavedLayout | null>(null);
  /** The open layout with the editor's unsaved changes. */
  const draft = ref<Layout | null>(null);
  /** The kept versions of the open layout, newest first; loaded on request. */
  const history = ref<LayoutVersion[]>([]);
  const error = ref<ApiError | null>(null);
  const busy = ref(false);
  let opening = 0;
  /** Unsaved drafts of layouts the user switched away from, by id; kept for this session. */
  const parked = new Map<string, Layout>();

  const dirty = computed(
    () =>
      saved.value !== null &&
      draft.value !== null &&
      JSON.stringify(saved.value.layout) !== JSON.stringify(draft.value),
  );
  /** The list entry of the open layout. */
  const current = computed(() => entries.value.find((e) => e.id === saved.value?.id) ?? null);

  function show(s: SavedLayout, keepDraft = false): void {
    const kept = keepDraft ? parked.get(s.id) : undefined;
    parked.delete(s.id);
    saved.value = s;
    draft.value = kept ?? structuredClone(s.layout);
  }

  /** Runs one call; its error is kept and the call answers `null`. */
  async function run<T>(call: () => Promise<T>): Promise<T | null> {
    busy.value = true;
    try {
      const value = await call();
      error.value = null;
      return value;
    } catch (e) {
      error.value = e as ApiError;
      return null;
    } finally {
      busy.value = false;
    }
  }

  /** Reads the layouts folder again, so a file edited by hand shows its new name. */
  async function loadList(): Promise<void> {
    const list = await run(() => useClient().listLayouts());
    if (list) entries.value = list;
  }

  /**
   * Opens a layout; its draft starts as the saved file, or as the unsaved draft left when
   * the user switched away from it. An older open that answers late is dropped.
   */
  async function open(id: string): Promise<boolean> {
    if (saved.value && draft.value && dirty.value) parked.set(saved.value.id, draft.value);
    const token = ++opening;
    const s = await run(() => useClient().getLayout({ id }));
    if (token !== opening) return false;
    history.value = [];
    if (!s) {
      saved.value = null;
      draft.value = null;
      return false;
    }
    show(s, true);
    return true;
  }

  /** Changes the text of one block of the draft. */
  function setBlockText(index: number, text: string): void {
    const block = draft.value?.blocks[index];
    if (block && block.text !== text) block.text = text;
  }

  async function after(s: SavedLayout | null, list = false): Promise<SavedLayout | null> {
    if (!s) return null;
    opening++;
    show(s);
    if (list) await loadList();
    if (history.value.length) await loadHistory();
    return s;
  }

  /** Saves the draft to the open file; the content it replaces becomes a version. */
  async function save(): Promise<SavedLayout | null> {
    const s = saved.value;
    const layout = draft.value;
    if (!s || !layout) return null;
    return after(await run(() => useClient().saveLayout({ id: s.id, layout })), true);
  }

  /**
   * Copies the open layout as saved into a new file and opens the copy; unsaved changes to
   * the original stay with it as a draft.
   */
  async function duplicate(name: string | null = null): Promise<SavedLayout | null> {
    const s = saved.value;
    if (!s) return null;
    if (draft.value && dirty.value) parked.set(s.id, draft.value);
    return after(await run(() => useClient().duplicateLayout({ id: s.id, name })), true);
  }

  /** Drops the unsaved changes: the draft becomes the saved file again. */
  async function revert(): Promise<SavedLayout | null> {
    const s = saved.value;
    if (!s) return null;
    return after(await run(() => useClient().revertLayout({ id: s.id, toOriginal: false })));
  }

  /** Writes the shipped original of the open layout's preset; the replaced content is kept. */
  async function resetToOriginal(): Promise<SavedLayout | null> {
    const s = saved.value;
    if (!s) return null;
    return after(await run(() => useClient().revertLayout({ id: s.id, toOriginal: true })), true);
  }

  async function loadHistory(): Promise<void> {
    const s = saved.value;
    if (!s) return;
    const list = await run(() => useClient().layoutHistory({ id: s.id }));
    if (list) history.value = list;
  }

  /** Makes a version current; what it replaces is kept as a version. */
  async function restore(number: number): Promise<SavedLayout | null> {
    const s = saved.value;
    if (!s) return null;
    const restored = await run(() => useClient().restoreLayout({ id: s.id, number }));
    if (!restored) return null;
    opening++;
    show(restored);
    await loadList();
    await loadHistory();
    return restored;
  }

  return {
    entries,
    saved,
    draft,
    history,
    error,
    busy,
    dirty,
    current,
    loadList,
    open,
    setBlockText,
    save,
    duplicate,
    revert,
    resetToOriginal,
    loadHistory,
    restore,
  };
});
