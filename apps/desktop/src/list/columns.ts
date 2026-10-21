// The columns of the detail list (table mode) and its saved configuration:
// column order, hidden columns, the sort column and the preview strip size.
// The sorting itself happens behind the API (`ListFamiliesRequest.order`);
// this module only says which column asks for which `FamilySortKey`, and how
// the API's values read in a cell.
import type {
  FamilyActivation,
  FamilyOrder,
  FamilySortKey,
  FamilySummary,
  FontFormat,
  LicenseType,
  OpenLicense,
  WeightRange,
} from '../api/types';
import { clampSetting, SIZE } from '../stores/preview';

export type ColumnId =
  | 'activation'
  | 'name'
  | 'styleCount'
  | 'weightLine'
  | 'weightRange'
  | 'format'
  | 'version'
  | 'manufacturer'
  | 'location'
  | 'license'
  | 'added'
  | 'tags'
  | 'notes';

export interface Column {
  id: ColumnId;
  /** Header text, and the accessible name of the column. */
  label: string;
  /** The API order a click on the header asks for; `null`: the column does not sort. */
  sortKey: FamilySortKey | null;
  /** Width in CSS px (the name column grows from it). */
  width: number;
  /** The name column always shows. */
  hideable: boolean;
}

/** The spec's columns, in their default order. */
export const COLUMNS: readonly Column[] = [
  { id: 'activation', label: 'Activation', sortKey: 'activation', width: 40, hideable: true },
  { id: 'name', label: 'Name', sortKey: 'name', width: 240, hideable: false },
  { id: 'styleCount', label: 'Styles', sortKey: 'styleCount', width: 72, hideable: true },
  { id: 'weightLine', label: 'Style', sortKey: 'weight', width: 140, hideable: true },
  { id: 'weightRange', label: 'Weight', sortKey: 'weight', width: 96, hideable: true },
  { id: 'format', label: 'Format', sortKey: 'format', width: 132, hideable: true },
  { id: 'version', label: 'Version', sortKey: 'version', width: 120, hideable: true },
  {
    id: 'manufacturer',
    label: 'Manufacturer',
    sortKey: 'manufacturer',
    width: 140,
    hideable: true,
  },
  { id: 'location', label: 'Location', sortKey: 'location', width: 200, hideable: true },
  { id: 'license', label: 'License', sortKey: 'license', width: 96, hideable: true },
  { id: 'added', label: 'Date added', sortKey: 'dateAdded', width: 112, hideable: true },
  { id: 'tags', label: 'Tags', sortKey: 'tags', width: 160, hideable: true },
  // The family's own notes; the API has no order by notes.
  { id: 'notes', label: 'Notes', sortKey: null, width: 200, hideable: true },
];

const BY_ID = new Map(COLUMNS.map((c) => [c.id, c]));

export const columnById = (id: ColumnId): Column => BY_ID.get(id)!;

const isColumnId = (v: unknown): v is ColumnId => BY_ID.has(v as ColumnId);

/** The column the table is sorted by, and the direction. */
export interface TableSort {
  column: ColumnId;
  descending: boolean;
}

export interface TableConfig {
  /** Every column, in display order. */
  order: ColumnId[];
  /** Columns not shown. */
  hidden: ColumnId[];
  /** `null`: the API's default order (A to Z). */
  sort: TableSort | null;
  /** Preview strip size in px; `null` follows the toolbar size. */
  stripSizePx: number | null;
}

export const defaultTable = (): TableConfig => ({
  order: COLUMNS.map((c) => c.id),
  hidden: [],
  sort: null,
  stripSizePx: null,
});

/**
 * A complete, valid configuration from anything (a saved entry). Unknown columns
 * are dropped, missing ones come back at their default place, the name column
 * cannot be hidden, a bad sort is no sort and the strip size is clamped.
 */
export function normalizeTable(raw: unknown): TableConfig {
  const r = (raw && typeof raw === 'object' ? raw : {}) as Partial<
    Record<keyof TableConfig, unknown>
  >;
  const order: ColumnId[] = [];
  for (const id of Array.isArray(r.order) ? r.order : [])
    if (isColumnId(id) && !order.includes(id)) order.push(id);
  COLUMNS.forEach((c, i) => {
    if (!order.includes(c.id)) order.splice(Math.min(i, order.length), 0, c.id);
  });
  const hidden = (Array.isArray(r.hidden) ? r.hidden : []).filter(
    (id, i, all): id is ColumnId =>
      isColumnId(id) && columnById(id).hideable && all.indexOf(id) === i,
  );
  const s = r.sort as Partial<TableSort> | null | undefined;
  const sort =
    s && typeof s === 'object' && isColumnId(s.column) && columnById(s.column).sortKey
      ? { column: s.column, descending: s.descending === true }
      : null;
  const stripSizePx =
    typeof r.stripSizePx === 'number' && Number.isFinite(r.stripSizePx)
      ? clampSetting(r.stripSizePx, SIZE, SIZE.min)
      : null;
  return { order, hidden, sort, stripSizePx };
}

/** The shown columns, in order. */
export function visibleColumns(config: TableConfig): Column[] {
  return config.order.filter((id) => !config.hidden.includes(id)).map(columnById);
}

/** Moves column `id` to position `to` of the full order (clamped). */
export function moveColumn(config: TableConfig, id: ColumnId, to: number): TableConfig {
  const order = config.order.filter((c) => c !== id);
  order.splice(Math.max(0, Math.min(to, order.length)), 0, id);
  return { ...config, order };
}

/** Shows a hidden column or hides a shown one; the name column always shows. */
export function toggleColumn(config: TableConfig, id: ColumnId): TableConfig {
  if (!columnById(id).hideable) return config;
  const hidden = config.hidden.includes(id)
    ? config.hidden.filter((c) => c !== id)
    : [...config.hidden, id];
  return { ...config, hidden };
}

/**
 * A header click: ascending, then descending, then back to the default order. A
 * column that does not sort leaves the order as it is.
 */
export function cycleSort(config: TableConfig, id: ColumnId): TableConfig {
  if (!columnById(id).sortKey) return config;
  const s = config.sort;
  const sort: TableSort | null =
    s?.column !== id
      ? { column: id, descending: false }
      : s.descending
        ? null
        : { column: id, descending: true };
  return { ...config, sort };
}

/** The API order of a configuration. */
export function orderOf(config: TableConfig): FamilyOrder | null {
  const by = config.sort && columnById(config.sort.column).sortKey;
  return config.sort && by ? { by, descending: config.sort.descending } : null;
}

/** `aria-sort` of a column header. */
export function ariaSort(config: TableConfig, id: ColumnId): 'ascending' | 'descending' | 'none' {
  const s = config.sort;
  if (s?.column !== id) return 'none';
  return s.descending ? 'descending' : 'ascending';
}

// ---- Cell text: how the API's values read in the table. ----

const FORMAT_BADGE: Record<FontFormat, string> = {
  otf: 'OT',
  otc: 'OT',
  ttf: 'TT',
  ttc: 'TT',
  woff: 'WF',
  woff2: 'WF',
  type1: 'T1',
  dfont: 'DF',
  bitmap: 'BMP',
};

/** The format badges: OT, TT, WF (…), then Variable and Color when they apply. */
export function formatBadges(
  formats: readonly FontFormat[],
  isVariable: boolean,
  isColor = false,
): string[] {
  const badges = [...new Set(formats.map((f) => FORMAT_BADGE[f]))];
  if (isVariable) badges.push('Variable');
  if (isColor) badges.push('Color');
  return badges;
}

const DECLARED: Record<LicenseType, string> = {
  desktop: 'Desktop',
  web: 'Web',
  app: 'App',
  trial: 'Trial',
  personalOnly: 'Personal',
  unknown: 'Unknown',
};

/** The badge of a family whose name table states an open license; shown in the good color. */
export const OPEN_LICENSE_BADGE = 'Open license';

const OPEN: Record<OpenLicense, string> = {
  ofl: 'SIL Open Font License',
  apache: 'Apache License 2.0',
  ufl: 'Ubuntu Font Licence',
  mit: 'MIT License',
};

/** The full name of an open license, for the badge's tooltip; '' for none. */
export function openLicenseName(l: OpenLicense | null | undefined): string {
  return l ? OPEN[l] : '';
}

/** The license badges: what the user declared, then "Open license" when the font states one. */
export function licenseBadges(f: Pick<FamilySummary, 'declaredLicense' | 'openLicense'>): string[] {
  const badges: string[] = [];
  if (f.declaredLicense) badges.push(DECLARED[f.declaredLicense]);
  if (f.openLicense) badges.push(OPEN_LICENSE_BADGE);
  return badges;
}

export function weightRangeText(r: WeightRange | null | undefined): string {
  if (!r) return '';
  return r.min === r.max ? String(r.min) : `${r.min}–${r.max}`;
}

/** Where weight `w` sits on the 100–1000 weight line, in percent. */
export function weightPosition(w: number): number {
  const clamped = Math.min(1000, Math.max(100, w));
  return ((clamped - 100) / 900) * 100;
}

export const ACTIVATION_LABEL: Record<FamilyActivation, string> = {
  inactive: 'Inactive',
  partial: 'Partly active',
  active: 'Active',
};

/** A timestamp as a short date in `locale` (the user's by default); '' when unreadable. */
export function dateText(iso: string, locale?: string): string {
  const d = new Date(iso);
  if (!iso || Number.isNaN(d.getTime())) return '';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeZone: 'UTC' }).format(d);
}
