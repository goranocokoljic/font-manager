import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';

/** Width bounds of the resizable sidebar, in CSS px. */
export const SIDEBAR = { min: 180, max: 400, initial: 232 } as const;
/** Width bounds of the list in the Layout mode's split (E50.3), in CSS px. */
export const SPLIT_LIST = { min: 400, max: 760, initial: 560 } as const;
/** Below this window width the split shows the pane only; a Fonts button slides the list over. */
export const SPLIT_COLLAPSE_PX = 1100;

/**
 * The least room the main pane keeps beside the panes (E58.6), in CSS px. With less, the
 * inspector folds first, then the sidebar, by themselves. At 1000 px and wider a sidebar of
 * any width ({@link SIDEBAR}.max) still fits, so nothing folds there.
 */
export const MAIN_ROOM = 480;

/** The panes beside the main pane: the width each takes when shown, `null` when folded by hand. */
export interface Panes {
  sidebar: number | null;
  inspector: number | null;
}

/**
 * Which panes are on screen in a window `width` px wide (E58.6). A pane the user folded stays
 * folded; of the panes the user keeps open the inspector folds first, then the sidebar, until
 * the main pane has `main` px.
 */
export function panesShown(
  width: number,
  panes: Panes,
  main: number = MAIN_ROOM,
): { sidebar: boolean; inspector: boolean } {
  const sidebar = panes.sidebar ?? 0;
  const inspector = panes.inspector ?? 0;
  const showInspector = panes.inspector !== null && sidebar + inspector + main <= width;
  const showSidebar =
    panes.sidebar !== null && sidebar + (showInspector ? inspector : 0) + main <= width;
  return { sidebar: showSidebar, inspector: showInspector };
}

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

function windowWidth(): number {
  const w = globalThis.innerWidth;
  return typeof w === 'number' && Number.isFinite(w) ? w : Number.POSITIVE_INFINITY;
}

/**
 * The sidebar's width and visibility in the main window, and the width of the list in the
 * Layout mode's split, saved as a view preference. `sidebarOpen` is the user's choice;
 * `sidebarShown` is what the window has room for (E58.6), never saved.
 */
export const useLayoutStore = defineStore('layout', () => {
  const storage = browserStorage();
  const saved = readSaved(storage);

  const sidebarWidth = ref(saved.sidebarWidth);
  const sidebarOpen = ref(saved.sidebarOpen);
  const splitListWidth = ref(saved.splitListWidth);
  /** The window's width now, in CSS px. */
  const width = ref(windowWidth());
  /**
   * The user asked for the sidebar while the window has no room for it: it is shown over the
   * room rule for this session, and the saved choice is left as it was. A change of room ends it.
   */
  const peek = ref(false);
  globalThis.addEventListener?.('resize', () => {
    const next = windowWidth();
    if (next === width.value) return;
    width.value = next;
    peek.value = false;
  });

  /** The sidebar has the room it needs beside the main pane. */
  const sidebarFits = computed(
    () => panesShown(width.value, { sidebar: sidebarWidth.value, inspector: null }).sidebar,
  );
  /** The sidebar is on screen: chosen by the user, and room for it (or asked for anyway). */
  const sidebarShown = computed(() => sidebarOpen.value && (sidebarFits.value || peek.value));
  /** The sidebar is folded by the room, not by the user. */
  const sidebarFolded = computed(() => sidebarOpen.value && !sidebarShown.value);

  function setSidebarWidth(px: number): void {
    sidebarWidth.value = clampWidth(px, SIDEBAR);
  }

  function setSplitListWidth(px: number): void {
    splitListWidth.value = clampWidth(px, SPLIT_LIST);
  }

  /**
   * Shows or hides the sidebar. Without room it is a look for this session (`peek`); the
   * saved choice changes only when the window has room for what the user picks.
   */
  function toggleSidebar(): void {
    if (sidebarOpen.value && !sidebarFits.value) {
      peek.value = !peek.value;
      return;
    }
    sidebarOpen.value = !sidebarOpen.value;
    peek.value = false;
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
    sidebarShown,
    sidebarFolded,
    splitListWidth,
    setSidebarWidth,
    setSplitListWidth,
    toggleSidebar,
  };
});
