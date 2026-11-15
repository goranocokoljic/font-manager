import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { GLYPH_GRID_STORAGE_KEY, normalizeGlyphGrid, useGlyphGridStore } from './glyphGrid';

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});

describe('the glyph grid setup', () => {
  it('defaults to 88 px cells with the grid on', () => {
    expect(useGlyphGridStore().settingsFor('family.characters')).toEqual({ cell: 88, lines: true });
  });

  it('remembers each view on its own, across a restart', () => {
    const store = useGlyphGridStore();
    store.update('family.characters', { cell: 48, lines: false });
    expect(store.settingsFor('other')).toEqual({ cell: 88, lines: true });
    setActivePinia(createPinia());
    const again = useGlyphGridStore();
    expect(again.settingsFor('family.characters')).toEqual({ cell: 48, lines: false });
    expect(again.settingsFor('other')).toEqual({ cell: 88, lines: true });
  });

  it('keeps a size in range', () => {
    const store = useGlyphGridStore();
    store.update('v', { cell: 400 });
    expect(store.settingsFor('v').cell).toBe(140);
    store.update('v', { cell: 10 });
    expect(store.settingsFor('v').cell).toBe(48);
  });

  it('repairs a damaged entry', () => {
    localStorage.setItem(GLYPH_GRID_STORAGE_KEY, '{"v":{"cell":"big","lines":3}');
    expect(useGlyphGridStore().settingsFor('v')).toEqual({ cell: 88, lines: true });
    expect(normalizeGlyphGrid({ cell: 'big', lines: 3 })).toEqual({ cell: 88, lines: true });
    expect(normalizeGlyphGrid(null)).toEqual({ cell: 88, lines: true });
  });
});
