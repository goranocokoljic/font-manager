import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  CompareMode as ApiCompareMode,
  ComparisonEntry,
  ComparisonId,
  FaceId,
  FaceSummary,
  MissingFace,
} from '../api/types';
import { fontNameSample } from '../preview/sampleText';
import { useToastStore } from './toast';

/** Most faces Compare holds at once. */
export const MAX_PINS = 4;

export type CompareMode = ApiCompareMode;

export const COMPARE_MODES: { value: CompareMode; label: string }[] = [
  { value: 'sideBySide', label: 'Side by side' },
  { value: 'stacked', label: 'Stacked' },
  { value: 'overlay', label: 'Overlay' },
];

/** What `pin` did. */
export type PinResult = 'pinned' | 'already' | 'full';

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
  const mode = ref<CompareMode>('sideBySide');
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

  /** Pins `face`; a fifth face is refused. Each outcome is told in a toast. */
  function pin(face: FaceSummary): PinResult {
    const toast = useToastStore();
    const name = fontNameSample(face);
    if (isPinned(face.id)) {
      toast.show(`${name} is already pinned to Compare.`);
      return 'already';
    }
    if (pins.value.length >= MAX_PINS) {
      toast.show(`Compare holds ${MAX_PINS} fonts. Remove one to pin ${name}.`);
      return 'full';
    }
    pins.value = [...pins.value, face];
    toast.show(`${name} pinned to Compare (${pins.value.length} of ${MAX_PINS}).`);
    return 'pinned';
  }

  /** Removes a pin; the sheet closes when none is left. */
  function unpin(face: FaceId): void {
    pins.value = pins.value.filter((p) => p.id !== face);
    overlayChoice.value = overlayChoice.value.map((id) => (id === face ? null : id)) as [
      FaceId | null,
      FaceId | null,
    ];
    if (!pins.value.length) {
      open.value = false;
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
      useToastStore().show((e as ApiError).message);
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
    useToastStore().show(`Comparison “${got.name}” saved.`);
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
      toast.show(`None of the fonts of “${got.name}” are in the library now: ${names}.`);
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
        ? `Opened “${got.name}”. Not in the library now: ${names}.`
        : `Opened “${got.name}”.`,
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
    pin,
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
