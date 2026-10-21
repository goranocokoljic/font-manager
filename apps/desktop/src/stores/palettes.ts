import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { FaceSummary } from '../api/types';
import { fontPalette } from '../preview/fontPalette';

/** The localStorage key of the chosen color palettes (a view preference). */
export const PALETTES_STORAGE_KEY = 'fm.palettes.v1';

/** Per view: family name → chosen palette index (never 0, the font's default). */
type Chosen = Record<string, Record<string, number>>;

function readSaved(storage: Storage | undefined): Chosen {
  const out: Chosen = {};
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(PALETTES_STORAGE_KEY) ?? 'null');
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return out;
    for (const [view, families] of Object.entries(parsed)) {
      if (!families || typeof families !== 'object' || Array.isArray(families)) continue;
      const own: Record<string, number> = {};
      for (const [family, index] of Object.entries(families as Record<string, unknown>)) {
        if (Number.isInteger(index) && (index as number) > 0 && (index as number) <= 0xffff)
          own[family] = index as number;
      }
      out[view] = own;
    }
  } catch {
    // A damaged entry falls back to the default palettes.
  }
  return out;
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/**
 * The color palette (E16.3) chosen for a family in the inspector, per view, saved as
 * a view preference. Every row of the family reads `fontPaletteOf` for its CSS
 * `font-palette`, so the webview draws the family's COLR faces in that palette in
 * every view mode. Keyed by family name, like the family's notes, so a rescan that
 * gives the family a new id keeps the choice.
 */
export const usePalettesStore = defineStore('palettes', () => {
  const storage = browserStorage();
  const chosen = ref<Chosen>(readSaved(storage));

  /** The palette index chosen for `family` in `view`; 0 (the default) when none. */
  function chosenFor(view: string, family: string): number {
    return chosen.value[view]?.[family] ?? 0;
  }

  /** Chooses palette `index` for `family` in `view`; 0 goes back to the font's default. */
  function choose(view: string, family: string, index: number): void {
    const own = { ...chosen.value[view] };
    if (Number.isInteger(index) && index > 0) own[family] = index;
    else delete own[family];
    chosen.value = { ...chosen.value, [view]: own };
  }

  /** The CSS `font-palette` of `face` in `view`, or undefined for the default palette. */
  function fontPaletteOf(view: string, face: FaceSummary): string | undefined {
    return fontPalette(face.id, chosenFor(view, face.familyName));
  }

  watch(chosen, (value) => {
    try {
      storage?.setItem(PALETTES_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the choice still works for this session.
    }
  });

  return { chosen, chosenFor, choose, fontPaletteOf };
});
