import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';

/** Under this window width the inspector collapses (spec: hidden on narrow windows). */
export const NARROW_WIDTH = 1100;

/** Width bounds of the resizable columns, in CSS px. */
export const SIDEBAR = { min: 180, max: 400, initial: 240 } as const;
export const INSPECTOR = { min: 260, max: 480, initial: 320 } as const;

/** The localStorage key of the saved layout (a view preference). */
export const LAYOUT_STORAGE_KEY = 'fm.layout.v1';

interface SavedLayout {
  sidebarWidth: number;
  inspectorWidth: number;
  inspectorOpen: boolean;
}

/** Rounds `value` and keeps it inside `bounds`; anything not a finite number gives `initial`. */
export function clampWidth(value: unknown, bounds: { min: number; max: number; initial: number }) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return bounds.initial;
  return Math.min(bounds.max, Math.max(bounds.min, Math.round(value)));
}

/** True for Ctrl+I or Cmd+I with no other modifier: the inspector shortcut. */
export function isInspectorShortcut(e: KeyboardEvent): boolean {
  return (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'i';
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
    inspectorWidth: clampWidth(raw.inspectorWidth, INSPECTOR),
    inspectorOpen: typeof raw.inspectorOpen === 'boolean' ? raw.inspectorOpen : true,
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
 * Column widths and inspector visibility of the main window. `inspectorOpen` is the
 * wide-window choice and is saved; on a narrow window the inspector is hidden until the
 * shortcut reveals it for the session.
 */
export const useLayoutStore = defineStore('layout', () => {
  const storage = browserStorage();
  const saved = readSaved(storage);

  const sidebarWidth = ref(saved.sidebarWidth);
  const inspectorWidth = ref(saved.inspectorWidth);
  const inspectorOpen = ref(saved.inspectorOpen);
  const narrow = ref(false);
  const narrowReveal = ref(false);

  const inspectorVisible = computed(() =>
    narrow.value ? narrowReveal.value : inspectorOpen.value,
  );

  function setSidebarWidth(px: number): void {
    sidebarWidth.value = clampWidth(px, SIDEBAR);
  }

  function setInspectorWidth(px: number): void {
    inspectorWidth.value = clampWidth(px, INSPECTOR);
  }

  function setNarrow(value: boolean): void {
    if (narrow.value !== value) narrowReveal.value = false;
    narrow.value = value;
  }

  function toggleInspector(): void {
    if (narrow.value) narrowReveal.value = !narrowReveal.value;
    else inspectorOpen.value = !inspectorOpen.value;
  }

  watch([sidebarWidth, inspectorWidth, inspectorOpen], () => {
    const value: SavedLayout = {
      sidebarWidth: sidebarWidth.value,
      inspectorWidth: inspectorWidth.value,
      inspectorOpen: inspectorOpen.value,
    };
    try {
      storage?.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the layout still works for this session.
    }
  });

  return {
    sidebarWidth,
    inspectorWidth,
    inspectorOpen,
    narrow,
    inspectorVisible,
    setSidebarWidth,
    setInspectorWidth,
    setNarrow,
    toggleInspector,
  };
});
