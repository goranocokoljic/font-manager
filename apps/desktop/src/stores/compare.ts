import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type {
  CompareMode as ApiCompareMode,
  ComparisonEntry,
  ComparisonId,
  FaceId,
  FamilyId,
  FaceSummary,
  MissingFace,
} from '../api/types';
import type { IconName } from '../components/ui/icons';
import { errorMessage, t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { useToastStore } from './toast';

/** Most faces Compare holds at once. */
export const MAX_PINS = 4;

export type CompareMode = ApiCompareMode;

/** The layout a new comparison opens in. */
export const DEFAULT_COMPARE_MODE: CompareMode = 'stacked';

/**
 * The layouts, in the order the switch shows them: Stacked, Grid, Overlay. Grid is
 * stored as the API's `sideBySide` (E48.2), so saved comparisons reopen unchanged and
 * old Side by side saves show as Grid.
 */
// Getters: the label is read in the language shown now.
export const COMPARE_MODES: { value: CompareMode; icon: IconName; label: string }[] = (
  [
    ['stacked', 'stacked'],
    ['sideBySide', 'grid'],
    ['overlay', 'overlay'],
  ] as const
).map(([value, icon]) => ({
  value,
  icon,
  get label() {
    return t(`compare.modes.${value}`);
  },
}));

/** What `pin` did. */
export type PinResult = 'pinned' | 'already' | 'full';

/** What `pinMany` did: faces added, already in the set, and left out because it was full. */
export interface PinManyResult {
  added: number;
  already: number;
  skipped: number;
}

/**
 * What a compare icon stands for (E48.4): one face (a style row, a face in the sheet, a
 * suggestion) or its whole family (a family's card or row: in the set when any of its faces is).
 */
export type CompareScope = 'face' | 'family';

/** The comparison the sheet shows when it was reopened or saved under a name. */
export interface CurrentComparison {
  id: ComparisonId;
  name: string;
}

/**
 * Compare: up to four faces pinned from the list (C or the command palette), shown
 * together in a sheet over the list. The pins live for the session only (in memory,
 * not in storage), so closing and reopening the sheet keeps them.
 *
 * Named comparisons (E13.2): the API keeps them in the index; `save` stores the pins,
 * mode and sample text under a name, `reopen` pins a saved one's faces again and keeps
 * its sample text (`text`) and the faces the library no longer has (`missing`).
 */
export const useCompareStore = defineStore('compare', () => {
  const pins = ref<FaceSummary[]>([]);
  const open = ref(false);
  const mode = ref<CompareMode>(DEFAULT_COMPARE_MODE);
  /** The two faces Overlay draws, as the user chose them; see `overlayPair`. */
  const overlayChoice = ref<[FaceId | null, FaceId | null]>([null, null]);
  /** Every saved comparison, by name. */
  const saved = ref<ComparisonEntry[]>([]);
  /** The saved comparison the pins came from or were last saved as. */
  const current = ref<CurrentComparison | null>(null);
  /**
   * The sample text of the reopened comparison, used instead of the list's; empty means
   * each face shows its own name. Null: the list's sample text.
   */
  const text = ref<string | null>(null);
  /** The faces of the reopened comparison the library no longer shows. */
  const missing = ref<MissingFace[]>([]);

  const isPinned = (face: FaceId) => pins.value.some((p) => p.id === face);
  /** A face of `family` is pinned. */
  const holdsFamily = (family: FamilyId) => pins.value.some((p) => p.family === family);

  /** The compare icon's state (E48.4): filled when `face`, or for a family its family, is in the set. */
  function inSet(face: FaceSummary, scope: CompareScope = 'face'): boolean {
    return scope === 'family' ? holdsFamily(face.family) : isPinned(face.id);
  }

  /** Pins `face`; a fifth face is refused. Each outcome is told in a toast. */
  function pin(face: FaceSummary): PinResult {
    const toast = useToastStore();
    const name = fontNameSample(face);
    if (isPinned(face.id)) {
      toast.show(t('compare.toasts.already', { name }));
      return 'already';
    }
    if (pins.value.length >= MAX_PINS) {
      toast.show(t('compare.toasts.full', { max: MAX_PINS, name }));
      return 'full';
    }
    pins.value = [...pins.value, face];
    toast.show(t('compare.toasts.pinned', { name, count: pins.value.length, max: MAX_PINS }));
    return 'pinned';
  }

  /**
   * Pins `faces` in order, up to the set's four, and says in one toast how many were
   * added and how many were left out (the selection bar's Compare, E48.4). `beyond` counts
   * more faces the caller did not read because the set could not take them: they are
   * skipped too.
   */
  function pinMany(faces: readonly FaceSummary[], beyond = 0): PinManyResult {
    const out: PinManyResult = { added: 0, already: 0, skipped: Math.max(0, beyond) };
    const next = [...pins.value];
    for (const face of faces) {
      if (next.some((p) => p.id === face.id)) out.already += 1;
      else if (next.length >= MAX_PINS) out.skipped += 1;
      else {
        next.push(face);
        out.added += 1;
      }
    }
    pins.value = next;
    const parts: string[] = [];
    if (out.added) {
      parts.push(
        t('compare.toasts.addedMany', { count: out.added, total: next.length, max: MAX_PINS }),
      );
    }
    if (out.skipped) parts.push(t('compare.toasts.skipped', { count: out.skipped, max: MAX_PINS }));
    if (!parts.length && out.already) parts.push(t('compare.toasts.allAlready'));
    if (parts.length) useToastStore().show(parts.join(' '));
    return out;
  }

  /**
   * The compare icon's click (E48.4): in the set, it removes the face (for a family, every
   * pinned face of it); else it pins the face. Either way a toast says what happened.
   */
  function toggle(face: FaceSummary, scope: CompareScope = 'face'): PinResult | 'removed' {
    if (!inSet(face, scope)) return pin(face);
    const name = scope === 'family' ? face.familyName : fontNameSample(face);
    const gone = pins.value.filter((p) =>
      scope === 'family' ? p.family === face.family : p.id === face.id,
    );
    for (const p of gone) unpin(p.id);
    useToastStore().show(t('compare.toasts.removed', { name }));
    return 'removed';
  }

  /** Removes a pin; the sheet closes when none is left, and the next comparison is new. */
  function unpin(face: FaceId): void {
    pins.value = pins.value.filter((p) => p.id !== face);
    overlayChoice.value = overlayChoice.value.map((id) => (id === face ? null : id)) as [
      FaceId | null,
      FaceId | null,
    ];
    if (!pins.value.length) {
      open.value = false;
      mode.value = DEFAULT_COMPARE_MODE;
      forget();
    }
  }

  /** Drops what the pins knew of a saved comparison. */
  function forget(): void {
    current.value = null;
    text.value = null;
    missing.value = [];
  }

  async function attempt<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      return await work();
    } catch (e) {
      useToastStore().show(errorMessage(e));
      return undefined;
    }
  }

  /** Reads the saved comparisons again. */
  async function loadSaved(): Promise<void> {
    const got = await attempt(() => useClient().listComparisons());
    if (got) saved.value = got;
  }

  /**
   * Saves the pins, mode and `sampleText` under `name`: over the current comparison
   * when the name is its name, else as a new one. True when it was saved.
   */
  async function save(name: string, sampleText: string): Promise<boolean> {
    const trimmed = name.trim();
    const over = current.value && current.value.name === trimmed ? current.value.id : null;
    const got = await attempt(() =>
      useClient().saveComparison({
        id: over,
        name: trimmed,
        faces: pins.value.map((p) => p.id),
        mode: mode.value,
        sampleText,
      }),
    );
    if (!got) return false;
    current.value = { id: got.id, name: got.name };
    text.value = sampleText;
    missing.value = [];
    useToastStore().show(t('compare.toasts.saved', { name: got.name }));
    await loadSaved();
    return true;
  }

  /**
   * Pins a saved comparison's faces in their order, with its mode and sample text, and
   * opens the sheet. Faces the library no longer shows are named in a toast and kept in
   * `missing`; with none left the sheet stays as it was.
   */
  async function reopen(id: ComparisonId): Promise<void> {
    const got = await attempt(() => useClient().getComparison({ id }));
    if (!got) return;
    const toast = useToastStore();
    const names = got.missing.map((m) => m.name).join(', ');
    if (!got.faces.length) {
      toast.show(t('compare.toasts.noneLeft', { name: got.name, names }));
      return;
    }
    pins.value = got.faces.slice(0, MAX_PINS);
    overlayChoice.value = [null, null];
    mode.value = got.mode;
    current.value = { id: got.id, name: got.name };
    text.value = got.sampleText;
    missing.value = got.missing;
    open.value = true;
    toast.show(
      got.missing.length
        ? t('compare.toasts.openedMissing', { name: got.name, names })
        : t('compare.toasts.opened', { name: got.name }),
    );
  }

  /**
   * The two faces Overlay draws: the user's choice where it is still pinned, else
   * the first pins not chosen for the other slot. Null for a slot with no face.
   */
  const overlayPair = computed<[FaceSummary | null, FaceSummary | null]>(() => {
    const byId = (id: FaceId | null) => pins.value.find((p) => p.id === id) ?? null;
    const out: [FaceSummary | null, FaceSummary | null] = [
      byId(overlayChoice.value[0]),
      byId(overlayChoice.value[1]),
    ];
    for (const slot of [0, 1] as const) {
      if (out[slot]) continue;
      out[slot] = pins.value.find((p) => p !== out[0] && p !== out[1]) ?? null;
    }
    return out;
  });

  /** Chooses the face of an Overlay slot; choosing the other slot's face swaps the two. */
  function setOverlay(slot: 0 | 1, face: FaceId): void {
    if (!isPinned(face)) return;
    const pair = overlayPair.value.map((f) => f?.id ?? null) as [FaceId | null, FaceId | null];
    const other = slot === 0 ? 1 : 0;
    if (pair[other] === face) pair[other] = pair[slot];
    pair[slot] = face;
    overlayChoice.value = pair;
  }

  function setMode(m: CompareMode): void {
    mode.value = m;
  }

  /** The sheet can open: something is pinned, or a saved comparison can be reopened. */
  const canOpen = computed(() => pins.value.length > 0 || saved.value.length > 0);

  function openSheet(): void {
    if (canOpen.value) open.value = true;
  }

  function close(): void {
    open.value = false;
  }

  return {
    pins,
    open,
    mode,
    overlayPair,
    saved,
    current,
    text,
    missing,
    canOpen,
    isPinned,
    holdsFamily,
    inSet,
    pin,
    pinMany,
    toggle,
    unpin,
    setOverlay,
    setMode,
    openSheet,
    close,
    loadSaved,
    save,
    reopen,
  };
});
