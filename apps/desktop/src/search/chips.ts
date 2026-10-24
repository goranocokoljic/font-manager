// The filter chips under the list title: one chip per filter value the API
// counted, grouped by filter. Choosing a chip edits the `Filters` of the search;
// the counts come from the API (`FilterCounts`), the chips only show them.
import type {
  Classification,
  Embedding,
  FilterCounts,
  Filters,
  FontFormat,
  LanguagePreset,
  LicenseType,
  Slant,
  SourceId,
  TagId,
} from '../api/types';
import { t, type MessageKey } from '../i18n';

/**
 * The filters shown as chips. Weight and width are ranges in `Filters` (the
 * counts are per class), so they are not toggle chips.
 */
export type ChipFacet =
  | 'active'
  | 'classification'
  | 'slant'
  | 'variable'
  | 'color'
  | 'formats'
  | 'features'
  | 'license'
  | 'webOk'
  | 'openLicense'
  | 'embedding'
  | 'sources'
  | 'problems'
  | 'tags'
  | 'favorite'
  | 'minRating';

/** A chip's value: whatever its filter takes. */
export type ChipValue = string | number | boolean;

export interface Chip {
  facet: ChipFacet;
  value: ChipValue;
  label: string;
  /** Family groups the search gives with this chip chosen. */
  count: number;
  selected: boolean;
}

export interface ChipGroup {
  facet: ChipFacet;
  label: string;
  chips: Chip[];
}

/** How a filter takes its values: any of several, every one, or a single one. */
type Kind = 'any' | 'all' | 'one';

const KIND: Record<ChipFacet, Kind> = {
  active: 'one',
  classification: 'any',
  slant: 'one',
  variable: 'one',
  color: 'one',
  formats: 'any',
  features: 'all',
  license: 'one',
  webOk: 'one',
  openLicense: 'one',
  embedding: 'one',
  sources: 'any',
  problems: 'one',
  tags: 'all',
  favorite: 'one',
  minRating: 'one',
};

/**
 * Labels of `keys` from the catalog under `prefix`: getters, so a label read later
 * (a computed, the rule editor) is in the language chosen then.
 */
function labels<K extends string>(prefix: string, keys: readonly K[]): Record<K, string> {
  const out = {} as Record<K, string>;
  for (const key of keys)
    Object.defineProperty(out, key, {
      enumerable: true,
      get: () => t(`${prefix}.${key}` as MessageKey),
    });
  return out;
}

export const CLASSES: Record<Classification, string> = labels('chips.classes', [
  'serif',
  'sans',
  'slab',
  'script',
  'display',
  'mono',
  'symbol',
]);
export const SLANTS: Record<Slant, string> = labels('chips.slants', [
  'upright',
  'italic',
  'oblique',
]);
// Format names are the same in every language.
export const FORMATS: Record<FontFormat, string> = {
  ttf: 'TTF',
  otf: 'OTF',
  ttc: 'TTC',
  otc: 'OTC',
  woff: 'WOFF',
  woff2: 'WOFF2',
  type1: 'Type 1',
  dfont: 'dfont',
  bitmap: 'Bitmap',
};
export const LICENSES: Record<LicenseType, string> = labels('chips.licenses', [
  'desktop',
  'web',
  'app',
  'trial',
  'personalOnly',
  'unknown',
]);
const EMBEDDINGS: Record<Embedding, string> = labels('chips.embeddings', [
  'installable',
  'editable',
  'previewAndPrint',
  'restricted',
]);
/** A yes/no chip's name: the `chips.values` message for true or for false. */
const yesNo = (yes: string, no: string) => (v: ChipValue) =>
  t(`chips.values.${v ? yes : no}` as MessageKey);

/** The groups in the order the bar shows them; the label is `chips.groups.<facet>`. */
const GROUPS: { facet: ChipFacet; name: (v: ChipValue) => string }[] = [
  { facet: 'active', name: yesNo('active', 'inactive') },
  { facet: 'classification', name: (v) => CLASSES[v as Classification] ?? String(v) },
  { facet: 'slant', name: (v) => SLANTS[v as Slant] ?? String(v) },
  { facet: 'variable', name: yesNo('variable', 'static') },
  { facet: 'color', name: yesNo('color', 'notColor') },
  { facet: 'formats', name: (v) => FORMATS[v as FontFormat] ?? String(v) },
  { facet: 'features', name: String },
  { facet: 'license', name: (v) => LICENSES[v as LicenseType] ?? String(v) },
  { facet: 'webOk', name: yesNo('webOk', 'notWeb') },
  { facet: 'openLicense', name: yesNo('openLicense', 'notOpen') },
  { facet: 'embedding', name: (v) => EMBEDDINGS[v as Embedding] ?? String(v) },
  { facet: 'sources', name: String },
  { facet: 'problems', name: yesNo('hasProblems', 'healthy') },
  { facet: 'favorite', name: yesNo('favorites', 'notFavorites') },
  {
    facet: 'minRating',
    name: (v) =>
      v === 5
        ? t('chips.values.stars', { count: 5 })
        : t('chips.values.starsOrMore', { count: Number(v) }),
  },
  { facet: 'tags', name: String },
];

/** The values a filter has chosen: a list for any/every-of filters, else zero or one. */
function chosen(filters: Filters, facet: ChipFacet): ChipValue[] {
  const v = filters[facet] as ChipValue | ChipValue[] | null | undefined;
  if (Array.isArray(v)) return v;
  return v == null ? [] : [v];
}

/** True when a chip with this value is chosen. */
export function isChosen(filters: Filters, facet: ChipFacet, value: ChipValue): boolean {
  return chosen(filters, facet).includes(value);
}

/**
 * Chooses the chip, or lets it go when it was chosen. A single-value filter
 * takes the new value in place of the old one.
 */
export function toggleChip(filters: Filters, facet: ChipFacet, value: ChipValue): Filters {
  const now = chosen(filters, facet);
  const on = now.includes(value);
  const next: Filters = { ...filters };
  if (KIND[facet] === 'one') {
    if (on) delete next[facet];
    else Object.assign(next, { [facet]: value });
  } else {
    const list = on ? now.filter((v) => v !== value) : [...now, value];
    if (list.length) Object.assign(next, { [facet]: list });
    else delete next[facet];
  }
  return next;
}

/** True when any filter is chosen. */
export function hasFilters(filters: Filters): boolean {
  return Object.values(filters).some((v) => (Array.isArray(v) ? v.length > 0 : v != null));
}

/**
 * True when a filter other than the open collection is chosen: a chip, a language, a
 * weight or width range. The open collection is where the list is, not a filter
 * chosen on it, so it alone does not make a smart collection.
 */
export function hasChosenFilters(filters: Filters): boolean {
  return hasFilters({ ...filters, collection: null });
}

/**
 * The chip groups for the counts the API returned. A chosen value with no count
 * (no result) still shows, with 0, so it can be let go; groups with no chip are left out.
 */
export function chipGroups(
  filters: Filters,
  counts: FilterCounts,
  sourceName: (id: SourceId) => string = (id) => t('chips.source', { id: String(id) }),
  tagName: (id: TagId) => string = (id) => t('chips.tag', { id: String(id) }),
): ChipGroup[] {
  const groups: ChipGroup[] = [];
  for (const g of GROUPS) {
    const counted = (counts[g.facet] ?? []) as { value: ChipValue; count: number }[];
    const values = new Map(counted.map((c) => [c.value, c.count]));
    for (const v of chosen(filters, g.facet)) if (!values.has(v)) values.set(v, 0);
    const name =
      g.facet === 'sources'
        ? (v: ChipValue) => sourceName(v as SourceId)
        : g.facet === 'tags'
          ? (v: ChipValue) => tagName(v as TagId)
          : g.name;
    const chips = [...values].map(([value, count]) => ({
      facet: g.facet,
      value,
      label: name(value),
      count,
      selected: isChosen(filters, g.facet, value),
    }));
    if (chips.length) groups.push({ facet: g.facet, label: t(`chips.groups.${g.facet}`), chips });
  }
  return groups;
}

/** The language codes the `supports` filter asks for (all of them). */
export function chosenLanguages(filters: Filters): string[] {
  return filters.supports?.kind === 'languages' ? filters.supports.codes : [];
}

/** Adds a language to the `supports` filter, or takes it out when it is there. */
export function toggleLanguage(filters: Filters, code: string): Filters {
  const now = chosenLanguages(filters);
  const codes = now.includes(code) ? now.filter((c) => c !== code) : [...now, code];
  const next: Filters = { ...filters };
  if (codes.length) next.supports = { kind: 'languages', codes };
  else delete next.supports;
  return next;
}

/** The display name of a language preset code. */
export function languageName(languages: readonly LanguagePreset[], code: string): string {
  return languages.find((l) => l.code === code)?.name ?? code;
}
