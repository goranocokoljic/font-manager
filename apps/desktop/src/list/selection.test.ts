import { describe, expect, it } from 'vitest';
import {
  EMPTY_SELECTION,
  moveTo,
  prune,
  rangeBetween,
  selectAll,
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
