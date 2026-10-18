import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { defaultTable } from '../list/columns';
import { ALL_FONTS_VIEW } from './preview';
import { useViewsStore, VIEWS_STORAGE_KEY } from './views';

function freshStore() {
  setActivePinia(createPinia());
  return useViewsStore();
}

beforeEach(() => localStorage.clear());

describe('views store', () => {
  it('starts in list mode with the default table, and no API order', () => {
    const store = freshStore();
    expect(store.modeFor(ALL_FONTS_VIEW)).toBe('list');
    expect(store.tableFor(ALL_FONTS_VIEW)).toEqual(defaultTable());
    expect(store.orderFor(ALL_FONTS_VIEW)).toBeNull();
  });

  it('asks the API for the sort column only in table mode', () => {
    const store = freshStore();
    store.sortBy(ALL_FONTS_VIEW, 'manufacturer');
    expect(store.orderFor(ALL_FONTS_VIEW)).toBeNull();
    store.setMode(ALL_FONTS_VIEW, 'table');
    expect(store.orderFor(ALL_FONTS_VIEW)).toEqual({ by: 'manufacturer', descending: false });
  });

  it('saves mode, columns, sort and strip size per view and reads them back', async () => {
    const store = freshStore();
    store.setMode(ALL_FONTS_VIEW, 'table');
    store.sortBy(ALL_FONTS_VIEW, 'version');
    store.sortBy(ALL_FONTS_VIEW, 'version');
    store.moveColumn(ALL_FONTS_VIEW, 'tags', 1);
    store.toggleColumn(ALL_FONTS_VIEW, 'location');
    store.setStripSize(ALL_FONTS_VIEW, 64);
    store.setMode('collection-7', 'list');
    await nextTick();
    expect(localStorage.getItem(VIEWS_STORAGE_KEY)).toBeTruthy();

    const again = freshStore();
    const t = again.tableFor(ALL_FONTS_VIEW);
    expect(again.modeFor(ALL_FONTS_VIEW)).toBe('table');
    expect(t.sort).toEqual({ column: 'version', descending: true });
    expect(t.order[1]).toBe('tags');
    expect(t.hidden).toEqual(['location']);
    expect(t.stripSizePx).toBe(64);
    // Another view keeps its own setup.
    expect(again.tableFor('collection-7')).toEqual(defaultTable());
  });

  it('clamps the strip size and follows the toolbar again on null', () => {
    const store = freshStore();
    store.setStripSize(ALL_FONTS_VIEW, 2);
    expect(store.tableFor(ALL_FONTS_VIEW).stripSizePx).toBe(8);
    store.setStripSize(ALL_FONTS_VIEW, null);
    expect(store.tableFor(ALL_FONTS_VIEW).stripSizePx).toBeNull();
  });

  it('resets the columns but keeps the strip size', () => {
    const store = freshStore();
    store.toggleColumn(ALL_FONTS_VIEW, 'tags');
    store.sortBy(ALL_FONTS_VIEW, 'tags');
    store.setStripSize(ALL_FONTS_VIEW, 40);
    store.resetColumns(ALL_FONTS_VIEW);
    expect(store.tableFor(ALL_FONTS_VIEW)).toEqual({ ...defaultTable(), stripSizePx: 40 });
  });

  it('falls back to the defaults when the saved entry is damaged', () => {
    localStorage.setItem(VIEWS_STORAGE_KEY, '{not json');
    expect(freshStore().modeFor(ALL_FONTS_VIEW)).toBe('list');
    localStorage.setItem(VIEWS_STORAGE_KEY, JSON.stringify({ all: { mode: 'grid', table: 5 } }));
    const store = freshStore();
    expect(store.modeFor(ALL_FONTS_VIEW)).toBe('list');
    expect(store.tableFor(ALL_FONTS_VIEW)).toEqual(defaultTable());
  });
});
