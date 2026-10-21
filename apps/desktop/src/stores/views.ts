import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import type { FamilyOrder, LayoutPreset } from '../api/types';
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

/**
 * How a view shows its fonts: preview rows, the detail list (table), cards, a waterfall, or
 * the selected family in a body-text layout.
 */
export type ViewMode = 'list' | 'table' | 'grid' | 'waterfall' | 'layout';

export interface ViewState {
  mode: ViewMode;
  table: TableConfig;
  /** The layout preset the layout mode shows. */
  preset: LayoutPreset;
  /** The layout template file the layout mode shows (a preset's is its preset id). */
  layout: string;
}

const MODES: readonly ViewMode[] = ['list', 'table', 'grid', 'waterfall', 'layout'];

/** The layout presets, in the order of the picker. */
export const LAYOUT_PRESETS: readonly LayoutPreset[] = [
  'book',
  'columns',
  'magazine',
  'quote',
  'characters',
  'waterfall',
];

/** What the picker and the command palette call each preset. */
export const PRESET_LABELS: Record<LayoutPreset, string> = {
  book: 'Book',
  columns: 'Columns',
  magazine: 'Magazine',
  quote: 'Quote',
  characters: 'Characters',
  waterfall: 'Waterfall',
};

/** Whether `id` can name a layout template file (as `fm_core::layout::store` checks). */
const isLayoutId = (id: unknown): id is string =>
  typeof id === 'string' &&
  id.length > 0 &&
  id.length <= 64 &&
  !id.startsWith('.') &&
  !/[/\\:*?"<>|]/.test(id);

export function normalizeView(raw: unknown): ViewState {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Partial<
    Record<keyof ViewState, unknown>
  >;
  const preset = LAYOUT_PRESETS.includes(r.preset as LayoutPreset)
    ? (r.preset as LayoutPreset)
    : 'book';
  return {
    mode: MODES.includes(r.mode as ViewMode) ? (r.mode as ViewMode) : 'list',
    table: normalizeTable(r.table),
    preset,
    layout: isLayoutId(r.layout) ? r.layout : preset,
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
 * Per view: the view mode (list, table, grid, waterfall or layout), the layout shown and the
 * table's setup — column order, hidden columns, sort column and preview strip size. Saved per view as a view
 * preference, like the preview settings.
 */
export const useViewsStore = defineStore('views', () => {
  const storage = browserStorage();
  const views = ref<Record<string, ViewState>>(readSaved(storage));

  function stateOf(view: string): ViewState {
    return (
      views.value[view] ?? { mode: 'list', table: defaultTable(), preset: 'book', layout: 'book' }
    );
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
  const presetFor = (view: string): LayoutPreset => stateOf(view).preset;
  const layoutFor = (view: string): string => stateOf(view).layout;

  /** The API order of `view`: the table's sort column in table mode, else the default. */
  function orderFor(view: string): FamilyOrder | null {
    const s = stateOf(view);
    return s.mode === 'table' ? orderOf(s.table) : null;
  }

  function setMode(view: string, mode: ViewMode): void {
    put(view, { ...stateOf(view), mode });
  }

  /** Picks the layout preset; the selection and the shown family stay. */
  function setPreset(view: string, preset: LayoutPreset): void {
    put(view, { ...stateOf(view), preset, layout: preset });
  }

  /** Picks a layout template file by id (a preset's, or a copy); an invalid id is ignored. */
  function setLayout(view: string, id: string): void {
    if (!isLayoutId(id)) return;
    const preset = LAYOUT_PRESETS.find((p) => p === id) ?? stateOf(view).preset;
    put(view, { ...stateOf(view), preset, layout: id });
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
    presetFor,
    layoutFor,
    orderFor,
    setMode,
    setPreset,
    setLayout,
    sortBy,
    moveColumn,
    toggleColumn,
    setStripSize,
    resetColumns,
  };
});
