import { defineStore } from 'pinia';
import { computed, shallowReactive, shallowRef, watch } from 'vue';
import type { FaceSummary, FamilyId, FamilySummary } from '../api/types';
import {
  faceKey,
  familyKey,
  flattenRows,
  type FamilyRow,
  type ListRow,
  type RowKey,
} from '../list/rows';
import {
  clearSelected,
  EMPTY_SELECTION,
  moveTo,
  prune,
  selectAll as selectAllKeys,
  selectBox,
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

  /**
   * The faces the selected rows stand for, in list order, at most `limit`: a style row's own
   * face, a family row's first shown face (its styles are read when not loaded yet). The
   * selection bar's Compare and Studio act on them (E44.1).
   */
  async function selectedFaces(limit = Infinity): Promise<FaceSummary[]> {
    const out: FaceSummary[] = [];
    for (const row of selectedRows.value) {
      if (out.length >= limit) break;
      if (row.kind === 'face') {
        out.push(row.face);
        continue;
      }
      const id = row.family.id;
      if (!previewFace(id)) await library.facesOf(id).catch(() => undefined);
      const face = previewFace(id);
      if (face) out.push(face);
    }
    return out;
  }

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

  /**
   * The exact place of the view a family page opened over (E42.1). `anchor` keeps only the
   * family at the top, which is what a switch to another view needs; Back must come back to
   * the same pixel. It counts while the view would still lay out the same way.
   */
  let holding = false;
  let held: {
    top: number;
    linePx: number;
    perLine: number;
    rows: number;
    anchorIndex: number;
  } | null = null;

  /** While on, a view that leaves records where it was. */
  function holdScroll(on: boolean): void {
    holding = on;
  }
  /** A view is leaving at `top`. */
  function viewLeft(top: number, linePx: number, perLine: number): void {
    if (!holding) return;
    held = { top, linePx, perLine, rows: rows.value.length, anchorIndex: anchorIndex.value };
  }
  /** The place to come back to, for a view with these lines; null when there is none. */
  function heldScrollTop(linePx: number, perLine: number): number | null {
    if (
      !held ||
      held.linePx !== linePx ||
      held.perLine !== perLine ||
      held.rows !== rows.value.length ||
      held.anchorIndex !== anchorIndex.value
    ) {
      return null;
    }
    return held.top;
  }
  /** The user scrolled: the place is theirs again. */
  function dropHeldScroll(): void {
    held = null;
  }

  /**
   * Where each layout of a view was left (E43.1: the wall and its reading rows), so a switch
   * back lands on the same pixel. It counts while the layout lays out the same way and the
   * family at the top is the same; a scroll in between moves the anchor, and the anchor wins.
   */
  const layoutLeftAt = new Map<
    string,
    { top: number; linePx: number; perLine: number; rows: number; anchorIndex: number }
  >();
  /** Layout `key` is leaving at `top`. */
  function layoutLeft(key: string, top: number, linePx: number, perLine: number): void {
    layoutLeftAt.set(key, {
      top,
      linePx,
      perLine,
      rows: rows.value.length,
      anchorIndex: anchorIndex.value,
    });
  }
  /** The place layout `key` was left at, for these lines; null when it no longer counts. */
  function layoutScrollTop(key: string, linePx: number, perLine: number): number | null {
    const at = layoutLeftAt.get(key);
    if (
      !at ||
      at.linePx !== linePx ||
      at.perLine !== perLine ||
      at.rows !== rows.value.length ||
      at.anchorIndex !== anchorIndex.value
    ) {
      return null;
    }
    return at.top;
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

  const rowOf = (id: FamilyId) =>
    rows.value.find((r): r is FamilyRow => r.kind === 'family' && r.family.id === id);

  function toggleExpanded(id: FamilyId): void {
    if (rowOf(id)?.single) return;
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

  /** Esc, or the selection bar's ×: nothing selected. The cursor stays. */
  function clearSelection(): void {
    selection.value = clearSelected(selection.value);
  }

  /**
   * The wall's selection box (E44.1) touches the rows at `indexes`: they are the selection,
   * added to `base` (the selection when a Shift or Ctrl/Cmd drag started; else empty).
   */
  function boxSelect(indexes: readonly number[], base: ReadonlySet<RowKey> = new Set()): void {
    const touched = indexes.map((i) => keys.value[i]).filter((k): k is RowKey => k !== undefined);
    const next = selectBox(selection.value, base, touched);
    const now = selection.value;
    const same =
      next.cursor === now.cursor &&
      next.selected.size === now.selected.size &&
      [...next.selected].every((k) => now.selected.has(k));
    if (!same) selection.value = next;
  }

  /**
   * Right arrow: expands a collapsed family, or goes to its first style.
   * Left arrow: collapses an expanded family, or goes from a style to its family.
   */
  function expandOrEnter(): void {
    const row = cursorRow.value;
    if (row?.kind !== 'family') return;
    if (row.single) return;
    if (!row.expanded) expand(row.family.id);
    else if (rows.value[cursorIndex.value + 1]?.kind === 'face') moveBy(1);
  }

  /** Bumped by `reveal`; the list scrolls its cursor row into view on each change. */
  const revealed = shallowRef(0);

  /**
   * Puts the cursor (and the selection) on a face: expands its family and selects the
   * face's row, or the family row when the list does not show that style. False when
   * the list does not show the family at all (a search or filter hides it).
   */
  async function reveal(face: FaceSummary): Promise<boolean> {
    const id = face.family;
    if (!indexOf.value.has(familyKey(id))) return false;
    // A one-style family stays folded: its row already shows that style's sample.
    const single = families.value.find((f) => f.id === id)?.styleCount === 1;
    if (single) {
      click(familyKey(id));
      revealed.value++;
      return true;
    }
    expanded.add(id);
    await library.facesOf(id).catch(() => undefined);
    const key = indexOf.value.has(faceKey(face.id)) ? faceKey(face.id) : familyKey(id);
    if (!indexOf.value.has(key)) return false;
    click(key);
    revealed.value++;
    return true;
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
    selectedFaces,
    isSelected,
    anchor,
    anchorIndex,
    setAnchor,
    layoutLeft,
    layoutScrollTop,
    holdScroll,
    viewLeft,
    heldScrollTop,
    dropHeldScroll,
    expand,
    collapse,
    toggleExpanded,
    moveCursor,
    moveBy,
    click,
    toggleCursor,
    selectAll,
    clearSelection,
    boxSelect,
    expandOrEnter,
    collapseOrLeave,
    revealed,
    reveal,
  };
});
