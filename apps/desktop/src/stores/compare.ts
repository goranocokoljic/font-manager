import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { fontNameSample } from '../preview/sampleText';
import { useToastStore } from './toast';

/** Most faces Compare holds at once. */
export const MAX_PINS = 4;

export type CompareMode = 'sideBySide' | 'stacked' | 'overlay';

export const COMPARE_MODES: { value: CompareMode; label: string }[] = [
  { value: 'sideBySide', label: 'Side by side' },
  { value: 'stacked', label: 'Stacked' },
  { value: 'overlay', label: 'Overlay' },
];

/** What `pin` did. */
export type PinResult = 'pinned' | 'already' | 'full';

/**
 * Compare: up to four faces pinned from the list (C or the command palette), shown
 * together in a sheet over the list. The pins live for the session only (in memory,
 * not in storage), so closing and reopening the sheet keeps them.
 */
export const useCompareStore = defineStore('compare', () => {
  const pins = ref<FaceSummary[]>([]);
  const open = ref(false);
  const mode = ref<CompareMode>('sideBySide');
  /** The two faces Overlay draws, as the user chose them; see `overlayPair`. */
  const overlayChoice = ref<[FaceId | null, FaceId | null]>([null, null]);

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
    if (!pins.value.length) open.value = false;
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

  function openSheet(): void {
    if (pins.value.length) open.value = true;
  }

  function close(): void {
    open.value = false;
  }

  return {
    pins,
    open,
    mode,
    overlayPair,
    isPinned,
    pin,
    unpin,
    setOverlay,
    setMode,
    openSheet,
    close,
  };
});
