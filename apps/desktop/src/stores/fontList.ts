import { defineStore } from 'pinia';
import { computed, shallowReactive, shallowRef, watch } from 'vue';
import type { FaceSummary, FamilyId } from '../api/types';
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

/**
 * The font list's view state: which families are expanded, the cursor and the
 * selected rows. Not saved; a relaunch starts with everything collapsed.
 */
export const useFontListStore = defineStore('fontList', () => {
  const library = useLibraryStore();
  const expanded = shallowReactive(new Set<FamilyId>());
  const selection = shallowRef<ListSelection>(EMPTY_SELECTION);

  const rows = computed<ListRow[]>(() =>
    flattenRows(library.families, expanded, (id) => library.faces.get(id)),
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

  /** The face the cursor shows: a style row's face, or a family's first face. */
  const cursorFace = computed<FaceSummary | undefined>(() => {
    const row = cursorRow.value;
    if (!row) return undefined;
    return row.kind === 'face' ? row.face : library.faces.get(row.family.id)?.[0];
  });

  const isSelected = (key: RowKey) => selection.value.selected.has(key);

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
    expanded,
    selection,
    rows,
    cursor,
    cursorIndex,
    cursorRow,
    cursorFace,
    selectedCount,
    isSelected,
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
