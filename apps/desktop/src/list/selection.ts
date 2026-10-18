// Multi-selection over an ordered list of row keys, as in a desktop file list:
// a plain click or arrow selects one row, Shift extends a range from the anchor,
// Ctrl/Cmd toggles one row or moves the cursor without changing the selection.
import type { RowKey } from './rows';

export interface ListSelection {
  selected: ReadonlySet<RowKey>;
  /** The row the cursor is on (`aria-activedescendant`). */
  cursor: RowKey | null;
  /** Where a Shift range starts. */
  anchor: RowKey | null;
}

export const EMPTY_SELECTION: ListSelection = Object.freeze({
  selected: new Set<RowKey>(),
  cursor: null,
  anchor: null,
});

/** How a move or click changes the selection. */
export type SelectMode = 'replace' | 'extend' | 'toggle' | 'keep';

/** The keys from `a` to `b`, both included, in list order. */
export function rangeBetween(keys: readonly RowKey[], a: RowKey, b: RowKey): RowKey[] {
  const i = keys.indexOf(a);
  const j = keys.indexOf(b);
  if (i < 0 || j < 0) return j < 0 ? [] : [b];
  return keys.slice(Math.min(i, j), Math.max(i, j) + 1);
}

/** Puts the cursor on `key` and changes the selection the way `mode` says. */
export function selectKey(
  keys: readonly RowKey[],
  sel: ListSelection,
  key: RowKey,
  mode: SelectMode,
): ListSelection {
  switch (mode) {
    case 'replace':
      return { selected: new Set([key]), cursor: key, anchor: key };
    case 'extend': {
      const anchor = sel.anchor !== null && keys.includes(sel.anchor) ? sel.anchor : key;
      return { selected: new Set(rangeBetween(keys, anchor, key)), cursor: key, anchor };
    }
    case 'toggle': {
      const selected = new Set(sel.selected);
      if (!selected.delete(key)) selected.add(key);
      return { selected, cursor: key, anchor: key };
    }
    case 'keep':
      return { ...sel, cursor: key };
  }
}

/** Moves the cursor to row `index` (clamped to the list). */
export function moveTo(
  keys: readonly RowKey[],
  sel: ListSelection,
  index: number,
  mode: SelectMode,
): ListSelection {
  if (!keys.length) return sel;
  const key = keys[Math.max(0, Math.min(keys.length - 1, index))]!;
  return selectKey(keys, sel, key, mode);
}

/** Selects every row; the cursor stays. */
export function selectAll(keys: readonly RowKey[], sel: ListSelection): ListSelection {
  return { ...sel, selected: new Set(keys) };
}

/** Drops keys that are no longer rows (a collapsed family's styles, a removed family). */
export function prune(keys: ReadonlySet<RowKey>, sel: ListSelection): ListSelection {
  const selected = new Set([...sel.selected].filter((k) => keys.has(k)));
  const keep = (k: RowKey | null) => (k !== null && keys.has(k) ? k : null);
  const cursor = keep(sel.cursor);
  const anchor = keep(sel.anchor) ?? cursor;
  const same =
    selected.size === sel.selected.size && cursor === sel.cursor && anchor === sel.anchor;
  return same ? sel : { selected, cursor, anchor };
}
