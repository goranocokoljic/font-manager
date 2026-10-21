import { defineStore } from 'pinia';
import { computed, shallowReactive, shallowRef, watch } from 'vue';
import type { FaceSummary, FamilyId, FamilySummary } from '../api/types';
import { familyKey, flattenRows, type ListRow, type RowKey } from '../list/rows';
import {
  EMPTY_SELECTION,
  moveTo,
  prune,
  selectAll as selectAllKeys,
  selectKey,
  type ListSelection,
  type SelectMode,
} from '../list/selection';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { useVirtualStylesStore } from './virtualStyles';

/**
 * The font list's view state: which families are expanded, the cursor and the
 * selected rows. Not saved; a relaunch starts with everything collapsed. The
 * list shows the search results while a search or filter is on (an expanded
 * family shows its matching styles only), else the whole library. In an open
 * collection, an expanded family also shows the collection's virtual styles of
 * its faces; a family held only through them is listed too. The views
 * (list, table, grid, waterfall) all show these rows, so switching views keeps
 * the selection; `anchor` keeps the scroll position, as the family at the top.
 */
export const useFontListStore = defineStore('fontList', () => {
  const library = useLibraryStore();
  const expanded = shallowReactive(new Set<FamilyId>());
  const selection = shallowRef<ListSelection>(EMPTY_SELECTION);

  const search = useSearchStore();
  const pinned = useVirtualStylesStore();

  const families = computed<FamilySummary[]>(() => {
    const hits = search.hits?.map((h) => h.family);
    if (!hits) return library.families;
    const listed = new Set(hits.map((f) => f.id));
    const extra = library.families.filter((f) => !listed.has(f.id) && pinned.byFamily.has(f.id));
    return extra.length ? [...hits, ...extra] : hits;
  });
  /** The faces of a family the list shows: with search results, the matching ones. */
  function shownFaces(id: FamilyId): FaceSummary[] | undefined {
    const all = library.faces.get(id);
    if (!search.hits) return all;
    const only = search.matchingFaces.get(id);
    if (!all) return all;
    // A family listed only for its virtual styles shows none of its own faces.
    return only ? all.filter((f) => only.has(f.id)) : pinned.byFamily.has(id) ? [] : all;
  }
  /** The face a family row previews: its first shown face, else its first virtual style's. */
  function previewFace(id: FamilyId): FaceSummary | undefined {
    return shownFaces(id)?.[0] ?? pinned.byFamily.get(id)?.[0]?.face;
  }

  const rows = computed<ListRow[]>(() =>
    flattenRows(families.value, expanded, shownFaces, (id) => pinned.byFamily.get(id) ?? []),
  );
  const keys = computed(() => rows.value.map((r) => r.key));
  const indexOf = computed(() => new Map(keys.value.map((k, i) => [k, i])));

  // Rows that went away (a collapsed family's styles, a removed family) leave the selection.
  watch(indexOf, (index) => {
    selection.value = prune(new Set(index.keys()), selection.value);
  });

  const cursor = computed(() => selection.value.cursor);
  const cursorIndex = computed(() =>
    cursor.value === null ? -1 : (indexOf.value.get(cursor.value) ?? -1),
  );
  const cursorRow = computed(() => rows.value[cursorIndex.value]);
  const selectedCount = computed(() => selection.value.selected.size);
  /** The selected rows, in list order. */
  const selectedRows = computed(() =>
    rows.value.filter((r) => selection.value.selected.has(r.key)),
  );

  /** The face the cursor shows: a style row's face, or a family's first face. */
  const cursorFace = computed<FaceSummary | undefined>(() => {
    const row = cursorRow.value;
    if (!row) return undefined;
    return row.kind === 'face' ? row.face : previewFace(row.family.id);
  });

  const isSelected = (key: RowKey) => selection.value.selected.has(key);

  /** The family at the top of the view; a view that mounts scrolls to it. */
  const anchor = shallowRef<FamilyId | null>(null);
  /** The row of the anchor family, or -1. */
  const anchorIndex = computed(() =>
    anchor.value === null ? -1 : (indexOf.value.get(familyKey(anchor.value)) ?? -1),
  );

  /**
   * A view scrolled; rows `start` to `end` (exclusive) are its first visible line
   * (one row, or one line of grid cards). The anchor stays while its family row is
   * in that line, so a view that restored it does not move it; else it becomes
   * the family of row `start`.
   */
  function setAnchor(start: number, end: number): void {
    const i = anchorIndex.value;
    if (i >= start && i < end) return;
    anchor.value = rows.value[start]?.family.id ?? null;
  }

  function expand(id: FamilyId): void {
    expanded.add(id);
    void library.facesOf(id).catch(() => undefined);
  }

  /** Collapses a family; a cursor on one of its styles goes to the family row. */
  function collapse(id: FamilyId): void {
    const row = cursorRow.value;
    if (row?.kind === 'face' && row.family.id === id)
      selection.value = selectKey(keys.value, selection.value, familyKey(id), 'keep');
    expanded.delete(id);
  }

  function toggleExpanded(id: FamilyId): void {
    if (expanded.has(id)) collapse(id);
    else expand(id);
  }

  /** Moves the cursor to row `index`. */
  function moveCursor(index: number, mode: SelectMode = 'replace'): void {
    selection.value = moveTo(keys.value, selection.value, index, mode);
  }

  /** Moves the cursor by `delta` rows; with no cursor yet, to the first row. */
  function moveBy(delta: number, mode: SelectMode = 'replace'): void {
    const from = cursorIndex.value;
    moveCursor(from < 0 ? 0 : from + delta, mode);
  }

  /** A click on a row: select it, toggle it (Ctrl/Cmd) or select a range to it (Shift). */
  function click(key: RowKey, mode: SelectMode = 'replace'): void {
    if (!indexOf.value.has(key)) return;
    selection.value = selectKey(keys.value, selection.value, key, mode);
  }

  /** Toggles the selection of the cursor row. */
  function toggleCursor(): void {
    if (cursor.value !== null) click(cursor.value, 'toggle');
  }

  function selectAll(): void {
    selection.value = selectAllKeys(keys.value, selection.value);
  }

  /**
   * Right arrow: expands a collapsed family, or goes to its first style.
   * Left arrow: collapses an expanded family, or goes from a style to its family.
   */
  function expandOrEnter(): void {
    const row = cursorRow.value;
    if (row?.kind !== 'family') return;
    if (!row.expanded) expand(row.family.id);
    else if (rows.value[cursorIndex.value + 1]?.kind === 'face') moveBy(1);
  }

  function collapseOrLeave(): void {
    const row = cursorRow.value;
    if (!row) return;
    if (row.kind === 'family') collapse(row.family.id);
    else click(familyKey(row.family.id));
  }

  return {
    families,
    shownFaces,
    previewFace,
    expanded,
    selection,
    rows,
    cursor,
    cursorIndex,
    cursorRow,
    cursorFace,
    selectedCount,
    selectedRows,
    isSelected,
    anchor,
    anchorIndex,
    setAnchor,
    expand,
    collapse,
    toggleExpanded,
    moveCursor,
    moveBy,
    click,
    toggleCursor,
    selectAll,
    expandOrEnter,
    collapseOrLeave,
  };
});
