import { defineStore } from 'pinia';
import { nextTick, ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, FaceId, FaceSummary, FamilyHit } from '../api/types';
import {
  disposePanelLayoutsStore,
  usePanelLayoutsStore,
  useTextLayoutsStore,
  type TextLayoutsStore,
} from './textLayouts';

/** Families a block font picker lists per search. */
export const STUDIO_RESULTS = 20;

/** The most panels the studio shows side by side (E24.3). */
export const MAX_PANELS = 4;

/** The key of the first panel: the library's layout, in the shared text layouts store. */
export const FIRST_PANEL = 0;

/**
 * The block-font scope of a layout in one panel. The first panel uses the layout id as it
 * is; the others put `p<key>/` before it. A layout id never holds `/`, so no two clash.
 */
export function blockScope(panel: number, layoutId: string): string {
  return panel === FIRST_PANEL ? layoutId : `p${panel}/${layoutId}`;
}

/** The text layouts store of panel `panel`. */
export function layoutsOf(panel: number): TextLayoutsStore {
  return panel === FIRST_PANEL ? useTextLayoutsStore() : usePanelLayoutsStore(panel);
}

/**
 * Pairing Studio's per-block fonts (E24.2): a block of a layout can be set in its own face
 * instead of the panel's. The choice lives here for the session only; it is never written
 * into the layout template file, which keeps its fm-api shape. A block is named by its
 * layout id and index, the only identity a block has. The pickers search the library
 * through fm-core's search (grouped by family); choosing a family takes its best face.
 *
 * The studio shows up to `MAX_PANELS` panels side by side (E24.3), by key in `panels`. The
 * first is the library's layout (`FIRST_PANEL`); each other one has its own text layouts
 * store and its own block fonts (`blockScope`). The panels live here, so they are as the
 * user left them when the studio opens again in the same session.
 *
 * "Print..." on a panel (E24.4) makes it the print target (`printPanel`) and opens the OS
 * print dialog through the client. The print stylesheet (styles/print.css) prints only the
 * target; the target stays until another panel prints, as it changes nothing on screen.
 */
export const useStudioStore = defineStore('studio', () => {
  /** Block fonts by `layoutId#index`. */
  const blockFaces = ref<Record<string, FaceSummary>>({});
  /** The picker that searched last (`layoutId#index`), so only it shows the results. */
  const searchFor = ref<string | null>(null);
  const query = ref('');
  const results = ref<FamilyHit[]>([]);
  const error = ref<ApiError | null>(null);

  // Answers to an older search, or to an older pick for the same block, are dropped.
  let searchRequest = 0;
  const picks = new Map<string, number>();

  const key = (layoutId: string, index: number) => `${layoutId}#${index}`;

  /** The panels, left to right, by key; the first is always `FIRST_PANEL`. */
  const panels = ref<number[]>([FIRST_PANEL]);
  let nextPanel = FIRST_PANEL + 1;

  /**
   * Adds a panel at the end as a copy of panel `from`: its open layout, its draft with the
   * unsaved text, and its block fonts. Answers the new key; null when `MAX_PANELS` are open
   * or `from` is not a panel.
   */
  function addPanel(from: number): number | null {
    if (panels.value.length >= MAX_PANELS || !panels.value.includes(from)) return null;
    const panel = nextPanel++;
    const source = layoutsOf(from);
    layoutsOf(panel).adopt(source);
    const layoutId = source.saved?.id;
    if (layoutId) {
      const mine = `${blockScope(from, layoutId)}#`;
      const theirs = `${blockScope(panel, layoutId)}#`;
      const copies = Object.entries(blockFaces.value)
        .filter(([k]) => k.startsWith(mine))
        .map(([k, face]) => [theirs + k.slice(mine.length), face]);
      blockFaces.value = { ...blockFaces.value, ...Object.fromEntries(copies) };
    }
    panels.value = [...panels.value, panel];
    return panel;
  }

  /** Removes panel `panel` with its layout draft and block fonts; the first panel stays. */
  function closePanel(panel: number): void {
    if (panel === FIRST_PANEL || !panels.value.includes(panel)) return;
    panels.value = panels.value.filter((p) => p !== panel);
    const prefix = `p${panel}/`;
    blockFaces.value = Object.fromEntries(
      Object.entries(blockFaces.value).filter(([k]) => !k.startsWith(prefix)),
    );
    if (searchFor.value?.startsWith(prefix)) searchFor.value = null;
    if (printPanel.value === panel) printPanel.value = null;
    if (printError.value?.panel === panel) printError.value = null;
    disposePanelLayoutsStore(panel);
  }

  /** The panel the print stylesheet prints: the one whose "Print..." was pressed last. */
  const printPanel = ref<number | null>(null);
  /** Why the last print failed, with its panel; null when it opened. */
  const printError = ref<{ panel: number; error: ApiError } | null>(null);

  /** Prints panel `panel` alone: marks it the print target, then opens the print dialog. */
  async function print(panel: number): Promise<void> {
    if (!panels.value.includes(panel)) return;
    printPanel.value = panel;
    printError.value = null;
    // The page must carry the mark before the webview lays it out for print.
    await nextTick();
    try {
      await useClient().printPage();
    } catch (e) {
      printError.value = { panel, error: e as ApiError };
    }
  }

  /** The face block `index` of layout `layoutId` is set in; null uses the panel's. */
  function faceOf(layoutId: string | null | undefined, index: number): FaceSummary | null {
    return layoutId ? (blockFaces.value[key(layoutId, index)] ?? null) : null;
  }

  /** Searches the library by name for the picker of one block. */
  async function search(layoutId: string, index: number, text: string): Promise<void> {
    searchFor.value = key(layoutId, index);
    query.value = text;
    const mine = ++searchRequest;
    try {
      const r = await useClient().search({
        query: text.trim(),
        page: { offset: 0, limit: STUDIO_RESULTS },
      });
      if (mine !== searchRequest) return;
      results.value = r.families;
      error.value = null;
    } catch (e) {
      if (mine === searchRequest) error.value = e as ApiError;
    }
  }

  /** Sets block `index` in face `face`. */
  async function setBlockFace(layoutId: string, index: number, face: FaceId): Promise<void> {
    const k = key(layoutId, index);
    const mine = (picks.get(k) ?? 0) + 1;
    picks.set(k, mine);
    try {
      const detail = await useClient().getFace(face);
      if (picks.get(k) !== mine) return;
      blockFaces.value = { ...blockFaces.value, [k]: detail.face };
      error.value = null;
    } catch (e) {
      if (picks.get(k) === mine) error.value = e as ApiError;
    }
  }

  /** Sets block `index` in the best face of a search result. */
  async function pickFamily(layoutId: string, index: number, hit: FamilyHit): Promise<void> {
    const best = hit.faces[0];
    if (best !== undefined) await setBlockFace(layoutId, index, best);
  }

  /** "Use panel font": block `index` follows the panel's face again. */
  function resetBlock(layoutId: string, index: number): void {
    const k = key(layoutId, index);
    picks.set(k, (picks.get(k) ?? 0) + 1);
    const rest = { ...blockFaces.value };
    delete rest[k];
    blockFaces.value = rest;
  }

  return {
    panels,
    addPanel,
    closePanel,
    printPanel,
    printError,
    print,
    blockFaces,
    searchFor,
    query,
    results,
    error,
    faceOf,
    search,
    setBlockFace,
    pickFamily,
    resetBlock,
  };
});
