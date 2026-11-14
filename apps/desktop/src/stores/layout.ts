import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

/** Width bounds of the resizable sidebar, in CSS px. */
export const SIDEBAR = { min: 180, max: 400, initial: 232 } as const;
/** Width bounds of the list in the Layout mode's split (E50.3), in CSS px. */
export const SPLIT_LIST = { min: 400, max: 760, initial: 560 } as const;
/** Below this window width the split shows the pane only; a Fonts button slides the list over. */
export const SPLIT_COLLAPSE_PX = 1100;

/** The localStorage key of the saved layout (a view preference). */
export const LAYOUT_STORAGE_KEY = 'fm.layout.v1';

interface SavedLayout {
  sidebarWidth: number;
  sidebarOpen: boolean;
  splitListWidth: number;
}

/** Rounds `value` and keeps it inside `bounds`; anything not a finite number gives `initial`. */
export function clampWidth(value: unknown, bounds: { min: number; max: number; initial: number }) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return bounds.initial;
  return Math.min(bounds.max, Math.max(bounds.min, Math.round(value)));
}

function readSaved(storage: Storage | undefined): SavedLayout {
  let raw: Partial<SavedLayout> = {};
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(LAYOUT_STORAGE_KEY) ?? 'null');
    if (parsed && typeof parsed === 'object') raw = parsed as Partial<SavedLayout>;
  } catch {
    // A damaged entry falls back to the defaults.
  }
  return {
    sidebarWidth: clampWidth(raw.sidebarWidth, SIDEBAR),
    sidebarOpen: typeof raw.sidebarOpen === 'boolean' ? raw.sidebarOpen : true,
    splitListWidth: clampWidth(raw.splitListWidth, SPLIT_LIST),
  };
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/**
 * The sidebar's width and visibility in the main window, and the width of the list in the
 * Layout mode's split, saved as a view preference.
 */
export const useLayoutStore = defineStore('layout', () => {
  const storage = browserStorage();
  const saved = readSaved(storage);

  const sidebarWidth = ref(saved.sidebarWidth);
  const sidebarOpen = ref(saved.sidebarOpen);
  const splitListWidth = ref(saved.splitListWidth);

  function setSidebarWidth(px: number): void {
    sidebarWidth.value = clampWidth(px, SIDEBAR);
  }

  function setSplitListWidth(px: number): void {
    splitListWidth.value = clampWidth(px, SPLIT_LIST);
  }

  function toggleSidebar(): void {
    sidebarOpen.value = !sidebarOpen.value;
  }

  watch([sidebarWidth, sidebarOpen, splitListWidth], () => {
    const value: SavedLayout = {
      sidebarWidth: sidebarWidth.value,
      sidebarOpen: sidebarOpen.value,
      splitListWidth: splitListWidth.value,
    };
    try {
      storage?.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the layout still works for this session.
    }
  });

  return {
    sidebarWidth,
    sidebarOpen,
    splitListWidth,
    setSidebarWidth,
    setSplitListWidth,
    toggleSidebar,
  };
});
