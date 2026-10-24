import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { mockColorFace } from '../api/mock';
import { paletteName, setPaletteSheet } from '../preview/fontPalette';
import { PALETTES_STORAGE_KEY, usePalettesStore } from './palettes';
import { ALL_FONTS_VIEW } from './preview';

function freshStore() {
  setActivePinia(createPinia());
  return usePalettesStore();
}

const face = mockColorFace.face;
let rules: string[];

beforeEach(() => {
  localStorage.clear();
  rules = [];
  setPaletteSheet({ insertRule: (r) => rules.push(r) });
});
afterEach(() => setPaletteSheet(undefined));

describe('palettes store', () => {
  it('starts at the default palette, which needs no font-palette', () => {
    const store = freshStore();
    expect(store.chosenFor(ALL_FONTS_VIEW, face.familyName)).toBe(0);
    expect(store.fontPaletteOf(ALL_FONTS_VIEW, face)).toBeUndefined();
    expect(rules).toEqual([]);
  });

  it('draws every face of the family in the chosen palette through one rule per face', () => {
    const store = freshStore();
    store.choose(ALL_FONTS_VIEW, face.familyName, 1);
    expect(store.fontPaletteOf(ALL_FONTS_VIEW, face)).toBe(paletteName(face.id, 1));
    const sibling = { ...face, id: 9, style: 'Bold' };
    expect(store.fontPaletteOf(ALL_FONTS_VIEW, sibling)).toBe(paletteName(9, 1));
    // Asked again: no second rule.
    store.fontPaletteOf(ALL_FONTS_VIEW, face);
    expect(rules).toEqual([
      `@font-palette-values --fm-palette-8-1 { font-family: "fm-face-8"; base-palette: 1; }`,
      `@font-palette-values --fm-palette-9-1 { font-family: "fm-face-9"; base-palette: 1; }`,
    ]);
    // Another family keeps its default.
    expect(store.fontPaletteOf(ALL_FONTS_VIEW, { ...face, familyName: 'Other' })).toBeUndefined();
  });

  it('keeps the choice per view and saves it, and choosing 0 forgets it', async () => {
    const store = freshStore();
    store.choose(ALL_FONTS_VIEW, face.familyName, 1);
    store.choose('collection-7', face.familyName, 2);
    await nextTick();
    const again = freshStore();
    expect(again.chosenFor(ALL_FONTS_VIEW, face.familyName)).toBe(1);
    expect(again.chosenFor('collection-7', face.familyName)).toBe(2);
    again.choose(ALL_FONTS_VIEW, face.familyName, 0);
    await nextTick();
    expect(JSON.parse(localStorage.getItem(PALETTES_STORAGE_KEY)!)).toEqual({
      [ALL_FONTS_VIEW]: {},
      'collection-7': { 'Color Play': 2 },
    });
  });

  it('ignores damaged saved entries', () => {
    localStorage.setItem(
      PALETTES_STORAGE_KEY,
      JSON.stringify({ all: { A: -1, B: 1.5, C: 'x', D: 3 }, bad: [1], worse: null }),
    );
    const store = freshStore();
    expect(store.chosen).toEqual({ all: { D: 3 } });
    localStorage.setItem(PALETTES_STORAGE_KEY, '{not json');
    expect(freshStore().chosen).toEqual({});
  });

  it('survives a webview that rejects the rule', () => {
    setPaletteSheet({
      insertRule: () => {
        throw new SyntaxError('unknown rule');
      },
    });
    const store = freshStore();
    store.choose(ALL_FONTS_VIEW, face.familyName, 1);
    expect(store.fontPaletteOf(ALL_FONTS_VIEW, face)).toBe(paletteName(face.id, 1));
  });
});
