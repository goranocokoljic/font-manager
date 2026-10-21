import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  Classification,
  Collection,
  CollectionId,
  FilterCounts,
  Filters,
  FontFormat,
  LanguagePreset,
  LicenseType,
  Rule,
  RuleGroup,
  RuleOp,
  Slant,
  Source,
  Tag,
} from '../api/types';
import { CLASSES, FORMATS, LICENSES, SLANTS } from '../search/chips';
import { useCollectionsStore } from './collections';
import { useSearchStore } from './search';

/** What a rule row tests: one filter of `Filters`. */
export type Dimension =
  | 'classification'
  | 'weight'
  | 'width'
  | 'slant'
  | 'feature'
  | 'language'
  | 'text'
  | 'license'
  | 'foundry'
  | 'format'
  | 'added'
  | 'tag'
  | 'rating'
  | 'source';

/** The dimensions in the order the row's first menu lists them, with their labels. */
export const DIMENSIONS: { id: Dimension; label: string }[] = [
  { id: 'classification', label: 'Class' },
  { id: 'weight', label: 'Weight' },
  { id: 'width', label: 'Width' },
  { id: 'slant', label: 'Slant' },
  { id: 'feature', label: 'OpenType feature' },
  { id: 'language', label: 'Supports language' },
  { id: 'text', label: 'Supports text' },
  { id: 'license', label: 'License type' },
  { id: 'foundry', label: 'Foundry' },
  { id: 'format', label: 'Format' },
  { id: 'added', label: 'Date added' },
  { id: 'tag', label: 'Tag' },
  { id: 'rating', label: 'Rating' },
  { id: 'source', label: 'Source folder' },
];

/**
 * One rule row. `value` holds a single choice (as a string, like a `<select>`), the
 * pasted text or the number of days; `min` and `max` hold a weight or width range. A
 * `kept` row is a part of a stored rule the editor has no row for (say `variable`, or
 * a group nested deeper than one level): it is saved as it was, and can only be removed.
 */
export interface DraftRow {
  type: 'row';
  key: number;
  dimension: Dimension | 'kept';
  value: string;
  min: string;
  max: string;
  kept?: Rule;
}

/** A nested group: one level only, so it holds rows. */
export interface DraftGroup {
  type: 'group';
  key: number;
  op: RuleOp;
  rows: DraftRow[];
}

export type DraftItem = DraftRow | DraftGroup;

/** The top group of the rule being edited. */
export interface DraftRule {
  op: RuleOp;
  items: DraftItem[];
}

/** One choice of a value menu. */
export interface Choice {
  value: string;
  label: string;
}

/** The value menus, every one from the API. */
export interface Choices {
  counts: FilterCounts;
  languages: LanguagePreset[];
  tags: Tag[];
  sources: Source[];
}

const NO_CHOICES: Choices = { counts: {}, languages: [], tags: [], sources: [] };

let nextKey = 0;
const key = () => ++nextKey;

/** A new row for `dimension`, with no value chosen yet. */
export function newRow(dimension: Dimension = 'classification'): DraftRow {
  return { type: 'row', key: key(), dimension, value: '', min: '', max: '' };
}

/** The menu of values a row for `dimension` picks from; `null` for a typed value. */
export function choicesFor(dimension: Dimension, c: Choices): Choice[] | null {
  const counts = c.counts;
  switch (dimension) {
    case 'classification':
      return (counts.classification ?? []).map(({ value }) => ({
        value,
        label: CLASSES[value] ?? value,
      }));
    case 'weight':
      return (counts.weight ?? []).map(({ value }) => ({
        value: String(value),
        label: String(value),
      }));
    case 'width':
      return (counts.width ?? []).map(({ value }) => ({
        value: String(value),
        label: String(value),
      }));
    case 'slant':
      return (counts.slant ?? []).map(({ value }) => ({ value, label: SLANTS[value] ?? value }));
    case 'feature':
      return (counts.features ?? []).map(({ value }) => ({ value, label: value }));
    case 'language':
      return c.languages.map((l) => ({ value: l.code, label: l.name }));
    case 'license':
      return (counts.license ?? []).map(({ value }) => ({
        value,
        label: LICENSES[value] ?? value,
      }));
    case 'foundry':
      return (counts.foundry ?? []).map(({ value }) => ({ value, label: value }));
    case 'format':
      return (counts.formats ?? []).map(({ value }) => ({ value, label: FORMATS[value] ?? value }));
    case 'tag':
      return c.tags.map((t) => ({ value: String(t.id), label: t.name }));
    case 'rating':
      return (counts.minRating ?? []).map(({ value }) => ({
        value: String(value),
        label: `${value} ${value === 1 ? 'star' : 'stars'} or more`,
      }));
    case 'source':
      return c.sources.map((s) => ({ value: String(s.id), label: s.path }));
    case 'text':
    case 'added':
      return null;
  }
}

/**
 * The filters a row stands for; `null` while it has no value. The editor only puts the
 * chosen value in its `Filters` field: fm-core decides which fonts pass.
 */
export function rowFilters(row: DraftRow): Filters | null {
  if (row.dimension === 'kept') return null;
  const v = row.value;
  const n = Number(v);
  switch (row.dimension) {
    case 'weight':
    case 'width': {
      if (row.min === '' || row.max === '') return null;
      const range = { min: Number(row.min), max: Number(row.max) };
      return row.dimension === 'weight' ? { weight: range } : { width: range };
    }
    case 'text':
      return v ? { supports: { kind: 'text', text: v } } : null;
  }
  if (v === '') return null;
  switch (row.dimension) {
    case 'classification':
      return { classification: [v as Classification] };
    case 'slant':
      return { slant: v as Slant };
    case 'feature':
      return { features: [v] };
    case 'language':
      return { supports: { kind: 'languages', codes: [v] } };
    case 'license':
      return { license: v as LicenseType };
    case 'foundry':
      return { foundry: v };
    case 'format':
      return { formats: [v as FontFormat] };
    case 'added':
      return Number.isInteger(n) && n >= 0 ? { addedWithinDays: n } : null;
    case 'tag':
      return { tags: [n] };
    case 'rating':
      return { minRating: n };
    case 'source':
      return { sources: [n] };
  }
}

const leaf = (filters: Filters): Rule => ({ kind: 'filters', filters });

/**
 * The E14.2 rule the draft stands for: each row one rule, a nested group one group.
 * Rows with no value yet are left out.
 */
export function buildRule(draft: DraftRule): RuleGroup {
  const rows = (list: DraftRow[]): Rule[] =>
    list.flatMap((r) => {
      if (r.dimension === 'kept') return r.kept ? [r.kept] : [];
      const f = rowFilters(r);
      return f ? [leaf(f)] : [];
    });
  return {
    op: draft.op,
    rules: draft.items.flatMap((item): Rule[] =>
      item.type === 'row'
        ? rows([item])
        : [{ kind: 'group', group: { op: item.op, rules: rows(item.rows) } }],
    ),
  };
}

const keptRow = (rule: Rule): DraftRow => ({ ...newRow(), dimension: 'kept', kept: rule });

/** The rows of one stored filter set: one per field the editor has a row for. */
function rowsOf(f: Filters): DraftRow[] {
  const out: DraftRow[] = [];
  const kept: Filters = {};
  const add = (dimension: Dimension, value: string, more: Partial<DraftRow> = {}) =>
    out.push({ ...newRow(dimension), value, ...more });
  for (const [field, raw] of Object.entries(f) as [keyof Filters, unknown][]) {
    if (raw == null || (Array.isArray(raw) && !raw.length)) continue;
    const list = Array.isArray(raw) ? (raw as (string | number)[]) : null;
    const one = list?.length === 1 ? String(list[0]) : null;
    switch (field) {
      case 'classification':
      case 'formats':
      case 'sources':
        // "Any of": one value is a row, several stay as they are.
        if (one != null) {
          add(field === 'classification' ? field : field === 'formats' ? 'format' : 'source', one);
          continue;
        }
        break;
      case 'features':
      case 'tags':
        // "Every one of": one row per value.
        for (const v of list ?? []) add(field === 'features' ? 'feature' : 'tag', String(v));
        continue;
      case 'weight':
      case 'width': {
        const r = raw as { min: number; max: number };
        add(field, '', { min: String(r.min), max: String(r.max) });
        continue;
      }
      case 'slant':
      case 'license':
      case 'foundry':
        add(field, String(raw));
        continue;
      case 'minRating':
        add('rating', String(raw));
        continue;
      case 'addedWithinDays':
        add('added', String(raw));
        continue;
      case 'supports': {
        const s = raw as NonNullable<Filters['supports']>;
        if (s.kind === 'text') add('text', s.text);
        else if (s.codes.length === 1) add('language', s.codes[0]!);
        else break;
        continue;
      }
    }
    (kept as Record<string, unknown>)[field] = raw;
  }
  if (Object.keys(kept).length) out.push(keptRow(leaf(kept)));
  return out;
}

/**
 * The draft of a stored rule. A filter set becomes one row per field (they are ANDed,
 * like the rows of an "all of" group); in an "any of" group such a set becomes a
 * nested "all of" group, or, already nested, one kept row.
 */
export function draftOf(group: RuleGroup): DraftRule {
  const items: DraftItem[] = [];
  for (const rule of group.rules ?? []) {
    if (rule.kind === 'group') {
      const rows: DraftRow[] = [];
      for (const r of rule.group.rules ?? []) {
        // Deeper than the editor goes, or several fields in an "any of" group: kept.
        const split = r.kind === 'filters' ? rowsOf(r.filters) : [];
        if (r.kind === 'filters' && (split.length <= 1 || rule.group.op === 'and'))
          rows.push(...split);
        else rows.push(keptRow(r));
      }
      items.push({ type: 'group', key: key(), op: rule.group.op, rows });
      continue;
    }
    const split = rowsOf(rule.filters);
    if (split.length <= 1 || group.op === 'and') items.push(...split);
    else items.push({ type: 'group', key: key(), op: 'and', rows: split });
  }
  return { op: group.op, items };
}

/** How long the editor waits after a change before it asks for the live count. */
export const COUNT_DELAY_MS = 150;

/**
 * The smart-collection rule editor (E14.1): "New smart collection" and "Edit rules".
 * It keeps the draft rows, fills their value menus from the API, asks the API for the
 * live count of the draft rule (a search with `rule`) and saves through
 * `createCollection` or `updateCollectionRules`. Which fonts pass is fm-core's answer;
 * the editor only builds the `RuleGroup`.
 */
export const useRuleEditorStore = defineStore('ruleEditor', () => {
  const collections = useCollectionsStore();
  const search = useSearchStore();

  const open = ref(false);
  /** The smart collection being edited; null for a new one. */
  const editing = shallowRef<Collection | null>(null);
  const name = ref('');
  const draft = ref<DraftRule>({ op: 'and', items: [] });
  const choices = shallowRef<Choices>(NO_CHOICES);
  /** Family groups the draft rule finds now; null until the API answered. */
  const count = ref<number | null>(null);
  const error = ref<ApiError | null>(null);
  const busy = ref(false);

  const rule = computed(() => buildRule(draft.value));
  /** Rows with no value yet: they are left out of the rule. */
  const incomplete = computed(() => {
    const rows = draft.value.items.flatMap((i) => (i.type === 'row' ? [i] : i.rows));
    return rows.filter((r) => r.dimension !== 'kept' && rowFilters(r) == null).length;
  });

  async function loadChoices(): Promise<void> {
    const client = useClient();
    choices.value = NO_CHOICES;
    try {
      const [languages, tags, sources, all] = await Promise.all([
        client.listLanguages(),
        client.listTags(),
        client.listSources(),
        client.search({ query: '', filters: {}, page: { offset: 0, limit: 0 } }),
      ]);
      choices.value = { counts: all.counts ?? {}, languages, tags, sources };
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  /** Opens the editor once the menus are filled, so each row's menu shows its value. */
  async function start(collection: Collection | null, rules: () => DraftRule): Promise<void> {
    error.value = null;
    await loadChoices();
    editing.value = collection;
    name.value = '';
    draft.value = rules();
    count.value = null;
    open.value = true;
  }

  /** "New smart collection": one empty row in an "all of" group. */
  function openNew(): Promise<void> {
    return start(null, () => ({ op: 'and', items: [newRow()] }));
  }

  /** "Edit rules" on a smart collection. */
  function openEdit(id: CollectionId): Promise<void> {
    const c = collections.collections.find((x) => x.id === id);
    const group =
      c?.ruleGroup ?? (c?.rules ? { op: 'and' as const, rules: [leaf(c.rules)] } : null);
    if (!c || !group) return Promise.resolve();
    return start(c, () => draftOf(group));
  }

  function close(): void {
    open.value = false;
    editing.value = null;
  }

  /** Adds a row to the top group, or to the nested group `group`; returns it. */
  function addRow(group?: DraftGroup): DraftRow {
    const row = newRow();
    if (group) group.rows.push(row);
    else draft.value.items.push(row);
    return row;
  }

  /** Adds a nested group with one row. */
  function addGroup(): DraftGroup {
    const group: DraftGroup = { type: 'group', key: key(), op: 'or', rows: [newRow()] };
    draft.value.items.push(group);
    return draft.value.items.at(-1) as DraftGroup;
  }

  /** Removes a row or a nested group, wherever it is. */
  function remove(target: DraftItem): void {
    const items = draft.value.items;
    const at = items.findIndex((i) => i.key === target.key);
    if (at >= 0) {
      items.splice(at, 1);
      return;
    }
    for (const g of items)
      if (g.type === 'group') {
        const r = g.rows.findIndex((x) => x.key === target.key);
        if (r >= 0) g.rows.splice(r, 1);
      }
  }

  /** Changes the value, or the range, of a row. */
  function update(row: DraftRow, patch: Partial<Pick<DraftRow, 'value' | 'min' | 'max'>>): void {
    Object.assign(row, patch);
  }

  /** A new dimension clears the row's value; a range starts at the ends of its menu. */
  function setDimension(row: DraftRow, dimension: Dimension): void {
    row.dimension = dimension;
    row.value = '';
    const menu = choicesFor(dimension, choices.value) ?? [];
    row.min = dimension === 'weight' || dimension === 'width' ? (menu[0]?.value ?? '') : '';
    row.max = dimension === 'weight' || dimension === 'width' ? (menu.at(-1)?.value ?? '') : '';
  }

  // The live count: the draft rule as a search, a moment after the last change.
  let timer: ReturnType<typeof setTimeout> | undefined;
  let asked = 0;
  async function refreshCount(): Promise<void> {
    const ask = ++asked;
    try {
      const res = await useClient().search({
        query: '',
        filters: {},
        rule: rule.value,
        page: { offset: 0, limit: 0 },
      });
      if (ask === asked) count.value = res.total;
    } catch (e) {
      if (ask === asked) {
        count.value = null;
        error.value = e as ApiError;
      }
    }
  }
  watch(
    () => (open.value ? JSON.stringify(rule.value) : null),
    (now) => {
      clearTimeout(timer);
      if (now == null) return;
      timer = setTimeout(() => void refreshCount(), COUNT_DELAY_MS);
    },
  );

  /**
   * Saves the rule: a new smart collection, shown in the list, or the edited one's new
   * rule. A rule fm-core refuses keeps the editor open with its message.
   */
  async function save(): Promise<boolean> {
    busy.value = true;
    try {
      const client = useClient();
      const c = editing.value
        ? await client.updateCollectionRules({
            collection: editing.value.id,
            ruleGroup: rule.value,
          })
        : await client.createCollection({ name: name.value, parent: null, ruleGroup: rule.value });
      error.value = null;
      close();
      await collections.refresh();
      if (collections.openId === c.id) await search.run();
      else collections.show(c.id);
      return true;
    } catch (e) {
      error.value = e as ApiError;
      return false;
    } finally {
      busy.value = false;
    }
  }

  return {
    open,
    editing,
    name,
    draft,
    choices,
    count,
    error,
    busy,
    rule,
    incomplete,
    openNew,
    openEdit,
    close,
    addRow,
    addGroup,
    remove,
    update,
    setDimension,
    refreshCount,
    save,
  };
});
