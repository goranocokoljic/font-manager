import { defineStore } from 'pinia';
import { nextTick, ref } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  BlockRole,
  FaceId,
  FaceSummary,
  FamilyHit,
  Layout,
  LayoutRole,
  TextBlock,
} from '../api/types';
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
 * The block a pairing's headline face sets (E42.7): the largest one, the first of equals.
 * A block without a size counts as the smallest. Null for a layout without blocks.
 */
export function headlineBlock(blocks: readonly TextBlock[]): number | null {
  let best: number | null = null;
  blocks.forEach((b, i) => {
    if (best === null || (b.sizePt ?? 0) > (blocks[best]!.sizePt ?? 0)) best = i;
  });
  return best;
}

/** The role of block `index` in `roles` (E45.2); null for a block without one. */
export function roleOf(roles: readonly LayoutRole[], index: number): LayoutRole | null {
  return roles.find((r) => r.blocks.includes(index)) ?? null;
}

/**
 * The blocks a role sets, by the layout's own roles (E45.2), else the old rule for a layout
 * without roles: the headline is the largest block (`headlineBlock`). Empty when there is none.
 */
export function blocksOf(layout: Pick<Layout, 'blocks' | 'roles'>, role: BlockRole): number[] {
  const roles = layout.roles ?? [];
  if (roles.length) return roles.find((r) => r.role === role)?.blocks ?? [];
  if (role !== 'headline') return [];
  const index = headlineBlock(layout.blocks);
  return index === null ? [] : [index];
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
  /** The key of a role's card in the rail (E45.2): its picker's search and its lock. */
  const roleKey = (scope: string, role: BlockRole) => `${scope}@${role}`;

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
    locks.value = Object.fromEntries(
      Object.entries(locks.value).filter(([k]) => !k.startsWith(prefix)),
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

  /**
   * "Compare 2 pairings" (E45.1): a second panel, a copy of the first, beside it; pressed
   * again, back to the first panel alone.
   */
  function toggleCompare(): void {
    const others = panels.value.filter((p) => p !== FIRST_PANEL);
    if (others.length) others.forEach(closePanel);
    else addPanel(FIRST_PANEL);
  }

  /** Where the last PNG export went, or why it failed; null before the first one. */
  const exported = ref<{ path: string } | { error: ApiError } | null>(null);

  /**
   * "Export PNG" (E45.1): the core renders panel `panel`'s saved layout file (E45.5) in `face`,
   * each block in its own face where it has one, at 2× on paper. Unsaved edits are not in the
   * file, so the toolbar asks for a save first.
   */
  async function exportPng(panel: number, face: FaceId): Promise<string | null> {
    const layouts = layoutsOf(panel);
    const saved = layouts.saved;
    if (!saved || !panels.value.includes(panel)) return null;
    const scope = blockScope(panel, saved.id);
    const font = (id: FaceId) => ({ face: id, variations: [], features: [] });
    const blockFonts = saved.layout.blocks.map((_, i) => {
      const own = faceOf(scope, i);
      return own ? font(own.id) : null;
    });
    try {
      const path = await useClient().exportLayoutPng({
        layout: saved.id,
        scale: 2,
        surface: 'paper',
        font: font(face),
        blockFonts,
      });
      exported.value = { path };
      return path;
    } catch (e) {
      exported.value = { error: e as ApiError };
      return null;
    }
  }

  /** The face block `index` of layout `layoutId` is set in; null uses the panel's. */
  function faceOf(layoutId: string | null | undefined, index: number): FaceSummary | null {
    return layoutId ? (blockFaces.value[key(layoutId, index)] ?? null) : null;
  }

  /** Searches the library by name for the picker of one block. */
  async function search(layoutId: string, index: number, text: string): Promise<void> {
    await searchAs(key(layoutId, index), text);
  }

  /** Searches the library by name for the picker of one role's card (E45.2). */
  async function searchRole(scope: string, role: BlockRole, text: string): Promise<void> {
    await searchAs(roleKey(scope, role), text);
  }

  /** Searches the library for the picker `forKey`; only that picker shows the results. */
  async function searchAs(forKey: string, text: string): Promise<void> {
    searchFor.value = forKey;
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
    await setBlocksFace(layoutId, [index], face);
  }

  /**
   * Sets every block of `indices` in face `face`, asked once. A later pick or reset of a block
   * wins over this one for that block.
   */
  async function setBlocksFace(
    layoutId: string,
    indices: readonly number[],
    face: FaceId,
  ): Promise<void> {
    const mine = indices.map((i) => {
      const k = key(layoutId, i);
      const n = (picks.get(k) ?? 0) + 1;
      picks.set(k, n);
      return [k, n] as const;
    });
    const current = () => mine.filter(([k, n]) => picks.get(k) === n);
    try {
      const detail = await useClient().getFace(face);
      const keys = current();
      if (!keys.length) return;
      const set = Object.fromEntries(keys.map(([k]) => [k, detail.face]));
      blockFaces.value = { ...blockFaces.value, ...set };
      error.value = null;
    } catch (e) {
      if (current().length) error.value = e as ApiError;
    }
  }

  /** The face role `role` is set in: its first block's own face; null uses the panel's. */
  function roleFace(scope: string | null | undefined, role: LayoutRole): FaceSummary | null {
    const first = role.blocks[0];
    return first === undefined ? null : faceOf(scope, first);
  }

  /** Sets every block of `role` in the best face of a search result (the rail's picker). */
  async function pickRoleFamily(scope: string, role: LayoutRole, hit: FamilyHit): Promise<void> {
    const best = hit.faces[0];
    if (best !== undefined) await setBlocksFace(scope, role.blocks, best);
  }

  /** Locked roles by `scope@role` (E45.2): Shuffle (E45.3) leaves them as they are. */
  const locks = ref<Record<string, true>>({});

  /** Whether role `role` of layout scope `scope` is locked. */
  function isLocked(scope: string | null | undefined, role: BlockRole): boolean {
    return Boolean(scope && locks.value[roleKey(scope, role)]);
  }

  /** Locks role `role`, or unlocks it. */
  function toggleLock(scope: string, role: BlockRole): void {
    const k = roleKey(scope, role);
    const rest = { ...locks.value };
    if (rest[k]) delete rest[k];
    else rest[k] = true;
    locks.value = rest;
  }

  /** Sets block `index` in the best face of a search result. */
  async function pickFamily(layoutId: string, index: number, hit: FamilyHit): Promise<void> {
    const best = hit.faces[0];
    if (best !== undefined) await setBlockFace(layoutId, index, best);
  }

  /**
   * Loads a pairing into a layout (E42.7, E45.2): the Headline role's blocks are set in
   * `headline` and the Body role's in `body`; every other block follows the panel's face, the
   * body face. A layout without roles has its largest block as the headline (`blocksOf`).
   */
  async function loadPairing(
    layoutId: string,
    layout: Pick<Layout, 'blocks' | 'roles'>,
    headline: FaceId,
    body: FaceId | null,
  ): Promise<void> {
    const heads = blocksOf(layout, 'headline');
    const bodies = body === null ? [] : blocksOf(layout, 'body').filter((i) => !heads.includes(i));
    layout.blocks.forEach((_, i) => {
      if (!heads.includes(i) && !bodies.includes(i)) resetBlock(layoutId, i);
    });
    await Promise.all([
      heads.length ? setBlocksFace(layoutId, heads, headline) : null,
      bodies.length && body !== null ? setBlocksFace(layoutId, bodies, body) : null,
    ]);
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
    toggleCompare,
    exported,
    exportPng,
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
    searchRole,
    roleKey,
    setBlockFace,
    setBlocksFace,
    roleFace,
    pickRoleFamily,
    locks,
    isLocked,
    toggleLock,
    pickFamily,
    resetBlock,
    loadPairing,
  };
});
