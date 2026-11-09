import { defineStore } from 'pinia';
import { computed, ref } from 'vue';

/**
 * The Compare sheet's height (E48.1), as a share of the window: from 40 % to the whole
 * window. The last height is a view preference, kept in localStorage.
 */
export const SHEET_HEIGHT = { min: 0.4, max: 1, initial: 0.6 } as const;

/** The specimen size of Auto at the lowest and at the full height, in CSS px. */
export const SHEET_AUTO_SIZE = { min: 56, max: 80 } as const;

/** The localStorage key of the saved height (a view preference). */
export const COMPARE_SHEET_STORAGE_KEY = 'fm.compareSheet.v1';

interface SavedSheet {
  /** The height shown last. */
  height: number;
  /** The last height below full: where the maximize button goes back to. */
  partial: number;
}

/** Keeps `value` inside 40–100 %; anything not a finite number gives `fallback`. */
export function clampSheetHeight(value: unknown, fallback: number = SHEET_HEIGHT.initial): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback;
  return Math.min(SHEET_HEIGHT.max, Math.max(SHEET_HEIGHT.min, value));
}

/** Auto's specimen size for a sheet `height`: 56 px at 40 %, 80 px at full, linear between. */
export function autoSpecimenSize(height: number): number {
  const h = clampSheetHeight(height);
  const t = (h - SHEET_HEIGHT.min) / (SHEET_HEIGHT.max - SHEET_HEIGHT.min);
  return Math.round(SHEET_AUTO_SIZE.min + t * (SHEET_AUTO_SIZE.max - SHEET_AUTO_SIZE.min));
}

/** The sheet's height for a pointer at `clientY` in a window `viewport` px high. */
export function heightAtPointer(clientY: number, viewport: number): number {
  if (!(viewport > 0)) return SHEET_HEIGHT.initial;
  return clampSheetHeight((viewport - clientY) / viewport);
}

function readSaved(storage: Storage | undefined): SavedSheet {
  let raw: Partial<SavedSheet> = {};
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(COMPARE_SHEET_STORAGE_KEY) ?? 'null');
    if (parsed && typeof parsed === 'object') raw = parsed as Partial<SavedSheet>;
  } catch {
    // A damaged entry falls back to the defaults.
  }
  const partial = clampSheetHeight(raw.partial);
  return {
    height: clampSheetHeight(raw.height),
    partial: partial >= SHEET_HEIGHT.max ? SHEET_HEIGHT.initial : partial,
  };
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/** The Compare sheet's height; read when the sheet opens, written when a resize ends. */
export const useCompareSheetStore = defineStore('compareSheet', () => {
  const storage = browserStorage();
  const saved = readSaved(storage);

  const height = ref(saved.height);
  const partial = ref(saved.partial);
  const full = computed(() => height.value >= SHEET_HEIGHT.max);
  const autoSize = computed(() => autoSpecimenSize(height.value));

  /** Sets the height live (a drag in progress); `save` keeps it. */
  function setHeight(value: number): void {
    height.value = clampSheetHeight(value, height.value);
    if (!full.value) partial.value = height.value;
  }

  function save(): void {
    const value: SavedSheet = { height: height.value, partial: partial.value };
    try {
      storage?.setItem(COMPARE_SHEET_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the height still works for this session.
    }
  }

  /** Full height, or back to the last height below it. */
  function toggleFull(): void {
    height.value = full.value ? partial.value : SHEET_HEIGHT.max;
    save();
  }

  return { height, full, autoSize, setHeight, save, toggleFull };
});
