import { defineStore } from 'pinia';
import { reactive } from 'vue';
import { CELL, clampCell } from '../glyphs/layout';

/** The localStorage key of the glyph grid setups (a view preference). */
export const GLYPH_GRID_STORAGE_KEY = 'fm.glyphGrid.v1';

/** How a view's glyph grid looks (E50.7). */
export interface GlyphGridSettings {
  /** The smallest a cell may get, in CSS px (the slider). */
  cell: number;
  /** The hairline grid between cells. */
  lines: boolean;
  /** A click on a cell opens the glyph popover (E50.9); off, a click only selects it. */
  details: boolean;
}

export const DEFAULT_GLYPH_GRID: GlyphGridSettings = {
  cell: CELL.default,
  lines: true,
  details: true,
};

/** A saved setup, repaired: a bad or missing field takes its default. */
export function normalizeGlyphGrid(raw: unknown): GlyphGridSettings {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Partial<Record<string, unknown>>;
  return {
    cell: clampCell(r.cell),
    lines: typeof r.lines === 'boolean' ? r.lines : DEFAULT_GLYPH_GRID.lines,
    details: typeof r.details === 'boolean' ? r.details : DEFAULT_GLYPH_GRID.details,
  };
}

function storage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

function readSaved(): Record<string, GlyphGridSettings> {
  try {
    const parsed: unknown = JSON.parse(storage()?.getItem(GLYPH_GRID_STORAGE_KEY) ?? 'null');
    if (!parsed || typeof parsed !== 'object') return {};
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>).map(([k, v]) => [k, normalizeGlyphGrid(v)]),
    );
  } catch {
    return {};
  }
}

/**
 * The cell size and grid switch of each view that shows a glyph grid, remembered across
 * restarts. A view that never changed them gets the defaults (88 px cells, grid on,
 * details on click).
 */
export const useGlyphGridStore = defineStore('glyphGrid', () => {
  const saved = reactive<Record<string, GlyphGridSettings>>(readSaved());

  function settingsFor(view: string): GlyphGridSettings {
    return saved[view] ?? DEFAULT_GLYPH_GRID;
  }

  function update(view: string, patch: Partial<GlyphGridSettings>): void {
    saved[view] = normalizeGlyphGrid({ ...settingsFor(view), ...patch });
    try {
      storage()?.setItem(GLYPH_GRID_STORAGE_KEY, JSON.stringify(saved));
    } catch {
      // Storage unavailable: the setting still holds for this session.
    }
  }

  return { settingsFor, update };
});
