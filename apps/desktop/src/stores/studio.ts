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
  PairingRole,
  PairingSuggestion,
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
 * Whether a layout's name is the pairing name `pairing` (E45.4): the name itself, or the name
 * with the number Save adds when another layout has it ("Lora + Inter 2"), whatever the case.
 */
export function namedAfter(name: string, pairing: string): boolean {
  const n = name.toLowerCase();
  const p = pairing.toLowerCase();
  return n === p || (n.startsWith(`${p} `) && /^\d+$/.test(n.slice(p.length + 1)));
}

/** Candidates Shuffle (E45.3) asks for per role; it moves through them press by press. */
export const SHUFFLE_POOL = 12;

/** Candidates the rail's "Try for <role>" list shows (E45.3). */
export const TRY_COUNT = 3;

/** The most shuffles and Try picks Ctrl/Cmd-Z can take back, per layout. */
export const UNDO_DEPTH = 20;

/**
 * The pairing role (ADR 0021) a block role asks `suggestPairings` for, by the size of its type:
 * large type is a headline, running text is body, small type around it is a caption.
 */
export function pairingRoleOf(role: BlockRole): PairingRole {
  switch (role) {
    case 'headline':
    case 'title':
    case 'deck':
    case 'subtitle':
      return 'headline';
    case 'body':
      return 'body';
    case 'caption':
    case 'eyebrow':
    case 'action':
    case 'footnote':
    case 'details':
      return 'caption';
  }
}

/**
 * The face the suggestions for `role` must go with (E45.3): the face of the first locked role
 * in the rail's order other than `role` itself, else the panel's face. `faceOf` answers a role's
 * face (its own or the panel's); `locked` whether a role is locked.
 */
export function partnerOf(
  roles: readonly LayoutRole[],
  role: BlockRole,
  faceOf: (r: LayoutRole) => FaceSummary | null,
  locked: (r: BlockRole) => boolean,
  panelFace: FaceSummary | null,
): FaceSummary | null {
  for (const r of roles) {
    if (r.role === role || !locked(r.role)) continue;
    const face = faceOf(r);
    if (face) return face;
  }
  return panelFace;
}

/**
 * The next candidate of `list` for a role (E45.3): the first whose family is not `current`, not
 * in `shown` (what earlier shuffles gave the role) and not in `taken` (given to another role in
 * the same shuffle). When every candidate was shown, the round starts over: `restart` is true
 * and `shown` no longer counts. Null when nothing is left.
 */
export function nextCandidate(
  list: readonly PairingSuggestion[],
  current: number | null,
  shown: ReadonlySet<number>,
  taken: ReadonlySet<number>,
): { suggestion: PairingSuggestion; restart: boolean } | null {
  const free = list.filter((s) => s.face.family !== current && !taken.has(s.face.family));
  const fresh = free.find((s) => !shown.has(s.face.family));
  if (fresh) return { suggestion: fresh, restart: false };
  return free[0] ? { suggestion: free[0], restart: true } : null;
}

/** What a shuffle did: how many roles it changed, or why it changed none. */
export type ShuffleResult =
  { changed: number } | { none: 'allLocked' | 'noFace' | 'noCandidates' | 'failed' | 'stale' };

/** The "Try for <role>" list of one role (E45.3). */
export interface TryList {
  /** The role's card key (`scope@role`). */
  key: string;
  role: BlockRole;
  suggestions: PairingSuggestion[];
  loading: boolean;
  error: ApiError | null;
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
    fades.value = Object.fromEntries(
      Object.entries(fades.value).filter(([k]) => !k.startsWith(prefix)),
    );
    undoStacks.value = Object.fromEntries(
      Object.entries(undoStacks.value).filter(([k]) => !k.startsWith(prefix)),
    );
    for (const k of [...shown.keys()]) if (k.startsWith(prefix)) shown.delete(k);
    if (tries.value?.key.startsWith(prefix)) tries.value = null;
    triesBy.value = Object.fromEntries(
      Object.entries(triesBy.value).filter(([k]) => !k.startsWith(prefix)),
    );
    for (const k of [...tryRequests.keys()]) if (k.startsWith(prefix)) tryRequests.delete(k);
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
   * Closes the first panel while another is open (E45.4): the second panel's layout, draft and
   * roles (faces, locks, crossfades, undo) move into the first panel, which is the library's
   * layout, and the second panel closes. Answers the layout id the first panel shows now; null
   * when only the first panel is open.
   */
  function promote(): string | null {
    const other = panels.value.find((p) => p !== FIRST_PANEL);
    if (other === undefined) return null;
    const source = layoutsOf(other);
    const id = source.saved?.id ?? null;
    layoutsOf(FIRST_PANEL).adopt(source);
    if (id !== null) moveScope(blockScope(other, id), blockScope(FIRST_PANEL, id));
    closePanel(other);
    return id;
  }

  /**
   * Gives layout scope `to` the roles of scope `from`: the block faces, locks, crossfades, undo
   * steps and what Shuffle showed. What `to` had is dropped; `from` keeps its own.
   */
  function copyScope(from: string, to: string): void {
    if (from === to) return;
    const moved = <T>(record: Record<string, T>, sep: string) => {
      const kept = Object.entries(record).filter(([k]) => !k.startsWith(`${to}${sep}`));
      const copies = Object.entries(record)
        .filter(([k]) => k.startsWith(`${from}${sep}`))
        .map(([k, v]) => [`${to}${k.slice(from.length)}`, v] as const);
      return Object.fromEntries([...kept, ...copies]);
    };
    blockFaces.value = moved(blockFaces.value, '#');
    fades.value = moved(fades.value, '#');
    locks.value = moved(locks.value, '@');
    const stack = undoStacks.value[from];
    const stacks = { ...undoStacks.value };
    if (stack) stacks[to] = stack.map((step) => step.map((e) => ({ ...e, blocks: [...e.blocks] })));
    else delete stacks[to];
    undoStacks.value = stacks;
    for (const k of [...shown.keys()]) if (k.startsWith(`${to}@`)) shown.delete(k);
    for (const [k, v] of [...shown]) {
      if (k.startsWith(`${from}@`)) shown.set(`${to}${k.slice(from.length)}`, new Set(v));
    }
  }

  /** Moves the roles of scope `from` to scope `to` (`copyScope`), and drops them from `from`. */
  function moveScope(from: string, to: string): void {
    copyScope(from, to);
    const own = (k: string) => k.startsWith(`${from}#`) || k.startsWith(`${from}@`);
    const drop = <T>(record: Record<string, T>) =>
      Object.fromEntries(Object.entries(record).filter(([k]) => !own(k)));
    blockFaces.value = drop(blockFaces.value);
    fades.value = drop(fades.value);
    locks.value = drop(locks.value);
    const stacks = { ...undoStacks.value };
    delete stacks[from];
    undoStacks.value = stacks;
    for (const k of [...shown.keys()]) if (own(k)) shown.delete(k);
  }

  /**
   * The text is shared between compared pairings (E45.4): a block's new text goes to every open
   * panel that shows the same layout file as panel `panel`; the roles stay each panel's own.
   */
  function setText(panel: number, index: number, text: string): void {
    const id = layoutsOf(panel).saved?.id;
    for (const p of panels.value) {
      const layouts = layoutsOf(p);
      if (p === panel || (id !== undefined && layouts.saved?.id === id))
        layouts.setBlockText(index, text);
    }
  }

  /**
   * The faces of panel `panel`'s pairing (E45.2): the Headline and Body roles' first blocks, each
   * in its own face, else `panelFace`. A layout without roles names its largest block as the
   * headline. Null without a panel face.
   */
  function pairingOf(
    panel: number,
    panelFace: FaceSummary | null,
  ): { headline: FaceSummary; body: FaceSummary } | null {
    if (!panelFace) return null;
    const layouts = layoutsOf(panel);
    const draft = layouts.draft;
    const scope = layouts.saved ? blockScope(panel, layouts.saved.id) : null;
    const faceOfRole = (role: 'headline' | 'body') => {
      const first = draft ? blocksOf(draft, role)[0] : undefined;
      return (first !== undefined ? faceOf(scope, first) : null) ?? panelFace;
    };
    return { headline: faceOfRole('headline'), body: faceOfRole('body') };
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
   * "Export" as PNG (E45.1, E45.4): the core renders panel `panel`'s saved layout file (E45.5) in
   * `face`, each block in its own face where it has one, at `scale` on paper, and names the file
   * `<Headline>-<Body>-<context>.png`. Unsaved edits are not in the file, so the toolbar asks for
   * a save first.
   */
  async function exportPng(panel: number, face: FaceId, scale: 1 | 2 = 2): Promise<string | null> {
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
        scale,
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

  /**
   * Crossfades by `layoutId#index` (E45.3): bumped when Shuffle, a Try pick or Undo sets a
   * block's face, so the page crossfades that block; a picker's change just shows.
   */
  const fades = ref<Record<string, number>>({});

  /** The crossfade count of block `index`; 0 when it never faded. */
  function fadeOf(layoutId: string | null | undefined, index: number): number {
    return layoutId ? (fades.value[key(layoutId, index)] ?? 0) : 0;
  }

  /**
   * Sets the blocks of each entry in its face (null: the panel's) at once, with a crossfade, and
   * drops any pick still on its way for them.
   */
  function applyFaces(
    layoutId: string,
    entries: readonly { blocks: readonly number[]; face: FaceSummary | null }[],
  ): void {
    const faces = { ...blockFaces.value };
    const fading = { ...fades.value };
    for (const { blocks, face } of entries) {
      for (const i of blocks) {
        const k = key(layoutId, i);
        picks.set(k, (picks.get(k) ?? 0) + 1);
        if (face) faces[k] = face;
        else delete faces[k];
        fading[k] = (fading[k] ?? 0) + 1;
      }
    }
    blockFaces.value = faces;
    fades.value = fading;
  }

  /** Undo stacks by layout scope: the role faces before each shuffle or Try pick, newest last. */
  const undoStacks = ref<Record<string, { blocks: number[]; face: FaceSummary | null }[][]>>({});

  /** Keeps the faces `roles` have now, so Ctrl/Cmd-Z can bring them back. */
  function remember(scope: string, roles: readonly LayoutRole[]): void {
    const step = roles.map((r) => ({ blocks: [...r.blocks], face: roleFace(scope, r) }));
    const stack = [...(undoStacks.value[scope] ?? []), step].slice(-UNDO_DEPTH);
    undoStacks.value = { ...undoStacks.value, [scope]: stack };
  }

  /** Whether layout scope `scope` has a shuffle or Try pick to undo. */
  function canUndo(scope: string | null | undefined): boolean {
    return Boolean(scope && undoStacks.value[scope]?.length);
  }

  /** Takes back the last shuffle or Try pick of `scope`, with a crossfade; false when none. */
  function undo(scope: string | null | undefined): boolean {
    const stack = scope ? undoStacks.value[scope] : undefined;
    const step = stack?.[stack.length - 1];
    if (!scope || !stack || !step) return false;
    undoStacks.value = { ...undoStacks.value, [scope]: stack.slice(0, -1) };
    applyFaces(scope, step);
    return true;
  }

  /** Families each role's shuffles showed, by `scope@role`, so the next press moves on. */
  const shown = new Map<string, Set<number>>();
  let shuffleRequest = 0;

  /**
   * "Shuffle unlocked" (E45.3): each unlocked role of `layout` asks `suggestPairings` for faces of
   * its pairing role (`pairingRoleOf`) that go with the locked roles (`partnerOf`), from the
   * library only, and takes its next candidate (`nextCandidate`). Every changed role crossfades;
   * the faces before go on the undo stack. A newer shuffle drops an older one's answers.
   */
  async function shuffle(
    scope: string,
    layout: Pick<Layout, 'roles'>,
    panelFace: FaceSummary | null,
  ): Promise<ShuffleResult> {
    const roles = layout.roles ?? [];
    const open = roles.filter((r) => !isLocked(scope, r.role) && r.blocks.length);
    if (!open.length) return { none: 'allLocked' };
    const faceOf = (r: LayoutRole) => roleFace(scope, r) ?? panelFace;
    const mine = ++shuffleRequest;
    const asks = open.map((r) => {
      const current = faceOf(r);
      const partner = partnerOf(roles, r.role, faceOf, (x) => isLocked(scope, x), panelFace);
      if (!current || !partner) return Promise.resolve(null);
      return useClient().suggestPairings({
        face: current.id,
        limit: SHUFFLE_POOL,
        role: pairingRoleOf(r.role),
        partnerOf: partner.id,
      });
    });
    let answers: (PairingSuggestion[] | null)[];
    try {
      answers = await Promise.all(asks);
    } catch (e) {
      if (mine !== shuffleRequest) return { none: 'stale' };
      error.value = e as ApiError;
      return { none: 'failed' };
    }
    if (mine !== shuffleRequest) return { none: 'stale' };
    error.value = null;
    if (answers.every((a) => a === null)) return { none: 'noFace' };
    const taken = new Set<number>();
    const changes: { role: LayoutRole; face: FaceSummary }[] = [];
    open.forEach((r, i) => {
      const list = answers[i];
      if (!list) return;
      const k = roleKey(scope, r.role);
      // Locked while the answers were on their way: left as it is.
      if (isLocked(scope, r.role)) return;
      const current = faceOf(r)?.family ?? null;
      // The face the role has now counts as shown, so the next presses do not go back to it.
      const seen = shown.get(k) ?? new Set<number>();
      if (current !== null) seen.add(current);
      const next = nextCandidate(list, current, seen, taken);
      if (!next) return;
      if (next.restart) {
        seen.clear();
        if (current !== null) seen.add(current);
      }
      const family = next.suggestion.face.family;
      seen.add(family);
      shown.set(k, seen);
      taken.add(family);
      changes.push({ role: r, face: next.suggestion.face });
    });
    if (!changes.length) return { none: 'noCandidates' };
    remember(
      scope,
      changes.map((c) => c.role),
    );
    applyFaces(
      scope,
      changes.map((c) => ({ blocks: c.role.blocks, face: c.face })),
    );
    return { changed: changes.length };
  }

  /** The last "Try for <role>" list any rail asked for; null before the first ask. */
  const tries = ref<TryList | null>(null);
  /**
   * Each layout scope's "Try for <role>" list (E45.4): compared pairings each have a rail, and
   * one panel's list does not replace the other's.
   */
  const triesBy = ref<Record<string, TryList>>({});
  const tryRequests = new Map<string, number>();

  /** The "Try for <role>" list of layout scope `scope`; null before its first ask. */
  function triesOf(scope: string | null | undefined): TryList | null {
    return scope ? (triesBy.value[scope] ?? null) : null;
  }

  function putTries(scope: string, list: TryList): void {
    triesBy.value = { ...triesBy.value, [scope]: list };
    tries.value = list;
  }

  /**
   * Asks for the `TRY_COUNT` faces of the library that pair best in `role` of `layout` with the
   * locked roles (`partnerOf`), with their reasons; the role's own family is left out.
   */
  async function loadTries(
    scope: string,
    layout: Pick<Layout, 'roles'>,
    role: LayoutRole,
    panelFace: FaceSummary | null,
  ): Promise<void> {
    const mine = (tryRequests.get(scope) ?? 0) + 1;
    tryRequests.set(scope, mine);
    const k = roleKey(scope, role.role);
    const faceOf = (r: LayoutRole) => roleFace(scope, r) ?? panelFace;
    const current = faceOf(role);
    const locked = (x: BlockRole) => isLocked(scope, x);
    const partner = partnerOf(layout.roles ?? [], role.role, faceOf, locked, panelFace);
    const list = (patch: Partial<TryList>): TryList => ({
      key: k,
      role: role.role,
      suggestions: [],
      loading: false,
      error: null,
      ...patch,
    });
    if (!current || !partner) {
      putTries(scope, list({}));
      return;
    }
    const before = triesOf(scope);
    const kept = before?.key === k ? before.suggestions : [];
    putTries(scope, list({ suggestions: kept, loading: true }));
    try {
      const got = await useClient().suggestPairings({
        face: current.id,
        limit: TRY_COUNT,
        role: pairingRoleOf(role.role),
        partnerOf: partner.id,
      });
      if (mine === tryRequests.get(scope))
        putTries(scope, list({ suggestions: got.slice(0, TRY_COUNT) }));
    } catch (e) {
      if (mine === tryRequests.get(scope)) putTries(scope, list({ error: e as ApiError }));
    }
  }

  /** A Try candidate was clicked: it sets `role` alone, with a crossfade; Ctrl/Cmd-Z undoes it. */
  function applySuggestion(scope: string, role: LayoutRole, suggestion: PairingSuggestion): void {
    if (!role.blocks.length) return;
    remember(scope, [role]);
    applyFaces(scope, [{ blocks: role.blocks, face: suggestion.face }]);
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
    promote,
    copyScope,
    setText,
    pairingOf,
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
    fades,
    fadeOf,
    shuffle,
    tries,
    triesOf,
    loadTries,
    applySuggestion,
    canUndo,
    undo,
  };
});
