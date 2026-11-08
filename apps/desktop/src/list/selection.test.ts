import { describe, expect, it } from 'vitest';
import {
  EMPTY_SELECTION,
  moveTo,
  prune,
  rangeBetween,
  selectAll,
  clearSelected,
  selectBox,
  selectKey,
  type ListSelection,
} from './selection';

const keys = ['a', 'b', 'c', 'd', 'e'];
const selected = (s: ListSelection) => [...s.selected].sort();

describe('list selection', () => {
  it('a plain move or click selects only that row and sets the anchor', () => {
    let s = selectKey(keys, EMPTY_SELECTION, 'b', 'replace');
    s = moveTo(keys, s, 3, 'replace');
    expect(selected(s)).toEqual(['d']);
    expect([s.cursor, s.anchor]).toEqual(['d', 'd']);
  });

  it('Shift extends a range from the anchor, both ways', () => {
    let s = selectKey(keys, EMPTY_SELECTION, 'b', 'replace');
    s = selectKey(keys, s, 'd', 'extend');
    expect(selected(s)).toEqual(['b', 'c', 'd']);
    s = selectKey(keys, s, 'a', 'extend');
    expect(selected(s)).toEqual(['a', 'b']);
    expect(s.anchor).toBe('b');
  });

  it('Ctrl/Cmd toggles one row and keeps the rest', () => {
    let s = selectKey(keys, EMPTY_SELECTION, 'a', 'replace');
    s = selectKey(keys, s, 'c', 'toggle');
    s = selectKey(keys, s, 'e', 'toggle');
    expect(selected(s)).toEqual(['a', 'c', 'e']);
    s = selectKey(keys, s, 'c', 'toggle');
    expect(selected(s)).toEqual(['a', 'e']);
  });

  it('keep moves the cursor without changing the selection', () => {
    let s = selectKey(keys, EMPTY_SELECTION, 'a', 'replace');
    s = moveTo(keys, s, 2, 'keep');
    expect(selected(s)).toEqual(['a']);
    expect(s.cursor).toBe('c');
  });

  it('clamps moves to the list and ignores an empty one', () => {
    expect(moveTo(keys, EMPTY_SELECTION, 99, 'replace').cursor).toBe('e');
    expect(moveTo(keys, EMPTY_SELECTION, -3, 'replace').cursor).toBe('a');
    expect(moveTo([], EMPTY_SELECTION, 1, 'replace')).toBe(EMPTY_SELECTION);
  });

  it('selects all rows', () => {
    const s = selectAll(keys, selectKey(keys, EMPTY_SELECTION, 'c', 'replace'));
    expect(selected(s)).toEqual(keys);
    expect(s.cursor).toBe('c');
  });

  it('extends from the cursor when the anchor is gone', () => {
    const s = selectKey(keys, { selected: new Set(), cursor: null, anchor: 'zz' }, 'c', 'extend');
    expect(selected(s)).toEqual(['c']);
    expect(rangeBetween(keys, 'zz', 'b')).toEqual(['b']);
  });

  it('drops rows that went away and keeps the same object when nothing did', () => {
    const s = selectAll(keys, selectKey(keys, EMPTY_SELECTION, 'd', 'replace'));
    expect(prune(new Set(keys), s)).toBe(s);
    const pruned = prune(new Set(['a', 'b', 'c']), s);
    expect(selected(pruned)).toEqual(['a', 'b', 'c']);
    expect([pruned.cursor, pruned.anchor]).toEqual([null, null]);
  });
});

describe('clearing and the selection box (E44.1)', () => {
  it('Esc clears the selection and keeps the cursor', () => {
    const sel = selectKey(keys, selectKey(keys, EMPTY_SELECTION, 'b', 'replace'), 'd', 'extend');
    const cleared = clearSelected(sel);
    expect(selected(cleared)).toEqual([]);
    expect(cleared.cursor).toBe('d');
    expect(clearSelected(EMPTY_SELECTION)).toBe(EMPTY_SELECTION);
  });

  it('a box selects what it touches, replacing or adding to the base', () => {
    const sel = selectKey(keys, EMPTY_SELECTION, 'a', 'replace');
    const replaced = selectBox(sel, new Set(), ['c', 'd']);
    expect(selected(replaced)).toEqual(['c', 'd']);
    expect(replaced.cursor).toBe('c');
    expect(replaced.anchor).toBe('c');
    const added = selectBox(sel, sel.selected, ['c', 'd']);
    expect(selected(added)).toEqual(['a', 'c', 'd']);
  });

  it('a box that touches nothing keeps the cursor', () => {
    const sel = selectKey(keys, EMPTY_SELECTION, 'a', 'replace');
    const none = selectBox(sel, new Set(), []);
    expect(selected(none)).toEqual([]);
    expect(none.cursor).toBe('a');
  });
});
