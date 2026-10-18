import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { FamilyOrder } from '../api/types';
import {
  cycleSort,
  defaultTable,
  moveColumn as moveIn,
  normalizeTable,
  orderOf,
  toggleColumn as toggleIn,
  type ColumnId,
  type TableConfig,
} from '../list/columns';
import { clampSetting, SIZE } from './preview';

/** The localStorage key of the saved view modes and table setups (a view preference). */
export const VIEWS_STORAGE_KEY = 'fm.views.v1';

/** How a view shows its fonts: preview rows, or the detail list (table). */
export type ViewMode = 'list' | 'table';

export interface ViewState {
  mode: ViewMode;
  table: TableConfig;
}

const MODES: readonly ViewMode[] = ['list', 'table'];

export function normalizeView(raw: unknown): ViewState {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Partial<Record<keyof ViewState, unknown>>;
  return {
    mode: MODES.includes(r.mode as ViewMode) ? (r.mode as ViewMode) : 'list',
    table: normalizeTable(r.table),
  };
}

function readSaved(storage: Storage | undefined): Record<string, ViewState> {
  const views: Record<string, ViewState> = {};
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(VIEWS_STORAGE_KEY) ?? 'null');
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      for (const [view, value] of Object.entries(parsed)) views[view] = normalizeView(value);
    }
  } catch {
    // A damaged entry falls back to the defaults.
  }
  return views;
}

function browserStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

/**
 * Per view: the view mode (list or table) and the table's setup — column order,
 * hidden columns, sort column and preview strip size. Saved per view as a view
 * preference, like the preview settings.
 */
export const useViewsStore = defineStore('views', () => {
  const storage = browserStorage();
  const views = ref<Record<string, ViewState>>(readSaved(storage));

  function stateOf(view: string): ViewState {
    return views.value[view] ?? { mode: 'list', table: defaultTable() };
  }

  function put(view: string, state: ViewState): void {
    views.value = { ...views.value, [view]: state };
  }

  function updateTable(view: string, change: (t: TableConfig) => TableConfig): void {
    const s = stateOf(view);
    put(view, { ...s, table: normalizeTable(change(s.table)) });
  }

  const modeFor = (view: string): ViewMode => stateOf(view).mode;
  const tableFor = (view: string): TableConfig => stateOf(view).table;

  /** The API order of `view`: the table's sort column in table mode, else the default. */
  function orderFor(view: string): FamilyOrder | null {
    const s = stateOf(view);
    return s.mode === 'table' ? orderOf(s.table) : null;
  }

  function setMode(view: string, mode: ViewMode): void {
    put(view, { ...stateOf(view), mode });
  }

  const sortBy = (view: string, id: ColumnId) => updateTable(view, (t) => cycleSort(t, id));
  const moveColumn = (view: string, id: ColumnId, to: number) =>
    updateTable(view, (t) => moveIn(t, id, to));
  const toggleColumn = (view: string, id: ColumnId) => updateTable(view, (t) => toggleIn(t, id));

  /** Sets the strip size (clamped to the preview size range); `null` follows the toolbar. */
  function setStripSize(view: string, px: number | null): void {
    updateTable(view, (t) => ({
      ...t,
      stripSizePx: px === null ? null : clampSetting(px, SIZE, SIZE.min),
    }));
  }

  /** Puts the columns back (order, visibility, sort); the strip size stays. */
  function resetColumns(view: string): void {
    updateTable(view, (t) => ({ ...defaultTable(), stripSizePx: t.stripSizePx }));
  }

  watch(views, (value) => {
    try {
      storage?.setItem(VIEWS_STORAGE_KEY, JSON.stringify(value));
    } catch {
      // Storage full or blocked: the setup still works for this session.
    }
  });

  return {
    views,
    modeFor,
    tableFor,
    orderFor,
    setMode,
    sortBy,
    moveColumn,
    toggleColumn,
    setStripSize,
    resetColumns,
  };
});
