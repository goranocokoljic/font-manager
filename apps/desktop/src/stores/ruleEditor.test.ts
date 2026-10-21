import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { Filters, RuleGroup } from '../api/types';
import { useCollectionsStore } from './collections';
import {
  COUNT_DELAY_MS,
  DIMENSIONS,
  buildRule,
  choicesFor,
  draftOf,
  newRow,
  rowFilters,
  useRuleEditorStore,
  type Dimension,
  type DraftRow,
} from './ruleEditor';
import { useSearchStore } from './search';

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ faceTags: { 1: ['Brand'] } });
  setClient(client);
});
afterEach(() => {
  setClient(undefined);
  vi.useRealTimers();
});

const row = (dimension: Dimension, value = '', more: Partial<DraftRow> = {}): DraftRow => ({
  ...newRow(dimension),
  value,
  ...more,
});

describe('rule rows', () => {
  it('have a row for every dimension of the spec, and each builds its own Filters field', () => {
    expect(DIMENSIONS.map((d) => d.id)).toEqual([
      'classification',
      'weight',
      'width',
      'slant',
      'feature',
      'language',
      'text',
      'license',
      'foundry',
      'format',
      'added',
      'tag',
      'rating',
      'source',
    ]);
    const built: [DraftRow, Filters][] = [
      [row('classification', 'sans'), { classification: ['sans'] }],
      [row('weight', '', { min: '300', max: '700' }), { weight: { min: 300, max: 700 } }],
      [row('width', '', { min: '3', max: '5' }), { width: { min: 3, max: 5 } }],
      [row('slant', 'italic'), { slant: 'italic' }],
      [row('feature', 'smcp'), { features: ['smcp'] }],
      [row('language', 'el'), { supports: { kind: 'languages', codes: ['el'] } }],
      [row('text', 'Ђ'), { supports: { kind: 'text', text: 'Ђ' } }],
      [row('license', 'web'), { license: 'web' }],
      [row('foundry', 'rsms'), { foundry: 'rsms' }],
      [row('format', 'otf'), { formats: ['otf'] }],
      [row('added', '30'), { addedWithinDays: 30 }],
      [row('tag', '7'), { tags: [7] }],
      [row('rating', '4'), { minRating: 4 }],
      [row('source', '1'), { sources: [1] }],
    ];
    expect(built.map(([r]) => r.dimension)).toEqual(DIMENSIONS.map((d) => d.id));
    for (const [r, filters] of built) expect(rowFilters(r)).toEqual(filters);
    // No value yet: no filter.
    for (const d of DIMENSIONS) expect(rowFilters(row(d.id))).toBeNull();
  });

  it('build the E14.2 rule type, with one level of nested groups', () => {
    const rule = buildRule({
      op: 'and',
      items: [
        row('classification', 'sans'),
        row('tag'), // no value: left out
        {
          type: 'group',
          key: 99,
          op: 'or',
          rows: [row('language', 'sr-Cyrl'), row('language', 'el')],
        },
      ],
    });
    expect(rule).toEqual({
      op: 'and',
      rules: [
        { kind: 'filters', filters: { classification: ['sans'] } },
        {
          kind: 'group',
          group: {
            op: 'or',
            rules: [
              { kind: 'filters', filters: { supports: { kind: 'languages', codes: ['sr-Cyrl'] } } },
              { kind: 'filters', filters: { supports: { kind: 'languages', codes: ['el'] } } },
            ],
          },
        },
      ],
    } satisfies RuleGroup);
  });

  it('read a saved rule back, keeping what they have no row for', () => {
    const saved: RuleGroup = {
      op: 'or',
      rules: [
        { kind: 'filters', filters: { variable: true } },
        { kind: 'filters', filters: { slant: 'italic', weight: { min: 600, max: 900 } } },
        {
          kind: 'group',
          group: {
            op: 'and',
            rules: [
              { kind: 'filters', filters: { features: ['smcp', 'onum'] } },
              { kind: 'group', group: { op: 'or', rules: [] } },
            ],
          },
        },
      ],
    };
    const draft = draftOf(saved);
    // Two fields in an "any of" group become a nested "all of" group.
    expect(draft.items.map((i) => i.type)).toEqual(['row', 'group', 'group']);
    expect(buildRule(draft)).toEqual({
      op: 'or',
      rules: [
        { kind: 'filters', filters: { variable: true } },
        {
          kind: 'group',
          group: {
            op: 'and',
            rules: [
              { kind: 'filters', filters: { slant: 'italic' } },
              { kind: 'filters', filters: { weight: { min: 600, max: 900 } } },
            ],
          },
        },
        {
          kind: 'group',
          group: {
            op: 'and',
            rules: [
              { kind: 'filters', filters: { features: ['smcp'] } },
              { kind: 'filters', filters: { features: ['onum'] } },
              { kind: 'group', group: { op: 'or', rules: [] } },
            ],
          },
        },
      ],
    });
  });

  it('take their value menus from the API answers, not from lists of their own', () => {
    const choices = {
      counts: {
        classification: [{ value: 'serif' as const, count: 2 }],
        foundry: [{ value: 'Acme', count: 1 }],
        minRating: [{ value: 3, count: 1 }],
      },
      languages: [{ code: 'el', name: 'Greek', script: 'Grek' }],
      tags: [{ id: 5, name: 'Brand', faceCount: 1 }],
      sources: [{ id: 2, path: '/fonts', kind: 'watched' as const, paused: false, faceCount: 1 }],
    };
    expect(choicesFor('classification', choices)).toEqual([{ value: 'serif', label: 'Serif' }]);
    expect(choicesFor('foundry', choices)).toEqual([{ value: 'Acme', label: 'Acme' }]);
    expect(choicesFor('rating', choices)).toEqual([{ value: '3', label: '3 stars or more' }]);
    expect(choicesFor('language', choices)).toEqual([{ value: 'el', label: 'Greek' }]);
    expect(choicesFor('tag', choices)).toEqual([{ value: '5', label: 'Brand' }]);
    expect(choicesFor('source', choices)).toEqual([{ value: '2', label: '/fonts' }]);
    expect(choicesFor('slant', choices)).toEqual([]);
    expect(choicesFor('text', choices)).toBeNull();
    expect(choicesFor('added', choices)).toBeNull();
  });
});

describe('rule editor store', () => {
  it('fills the menus from list_languages, list_tags, list_sources and a search', async () => {
    const editor = useRuleEditorStore();
    await editor.openNew();
    expect(editor.open).toBe(true);
    expect(editor.choices.languages).toEqual(await client.listLanguages());
    expect(editor.choices.tags.map((t) => t.name)).toEqual(['Brand']);
    expect(editor.choices.sources).toEqual(await client.listSources());
    expect(editor.choices.counts).toEqual(
      (await client.search({ query: '', filters: {}, page: { offset: 0, limit: 0 } })).counts,
    );
  });

  it('asks a search with the draft rule for the live count', async () => {
    const search = vi.spyOn(client, 'search');
    vi.useFakeTimers();
    const editor = useRuleEditorStore();
    await editor.openNew();
    editor.update(editor.draft.items[0] as DraftRow, { value: 'mono' });
    await vi.advanceTimersByTimeAsync(COUNT_DELAY_MS + 10);
    const last = search.mock.calls.at(-1)![0];
    expect(last.rule).toEqual(editor.rule);
    expect(last.rule?.rules).toEqual([{ kind: 'filters', filters: { classification: ['mono'] } }]);
    expect(editor.count).toBe(1);
  });

  it('saves a new smart collection and shows it in the list', async () => {
    const editor = useRuleEditorStore();
    const collections = useCollectionsStore();
    await editor.openNew();
    editor.name = 'Mono';
    editor.update(editor.draft.items[0] as DraftRow, { value: 'mono' });
    expect(await editor.save()).toBe(true);
    expect(editor.open).toBe(false);
    const made = collections.collections.find((c) => c.name === 'Mono')!;
    expect(made.ruleGroup).toEqual(editor.rule);
    expect(useSearchStore().filters.collection).toBe(made.id);
  });

  it('shows the refusal of fm-core inline and saves nothing', async () => {
    const editor = useRuleEditorStore();
    const collections = useCollectionsStore();
    await editor.openNew();
    editor.name = 'Bad';
    editor.setDimension(editor.draft.items[0] as DraftRow, 'weight');
    editor.update(editor.draft.items[0] as DraftRow, { min: '700', max: '300' });
    expect(await editor.save()).toBe(false);
    expect(editor.open).toBe(true);
    expect(editor.error?.code).toBe('invalidRequest');
    expect(editor.error?.message).toMatch(/weight/);
    // An empty group: every row removed.
    editor.remove(editor.draft.items[0]!);
    expect(await editor.save()).toBe(false);
    expect(editor.error?.message).toMatch(/at least one rule/);
    await collections.refresh();
    expect(collections.collections).toEqual([]);
  });

  it('edits the rules of a smart collection through updateCollectionRules', async () => {
    const collections = useCollectionsStore();
    const made = await client.createCollection({ name: 'Smart', rules: { slant: 'italic' } });
    await collections.refresh();
    const update = vi.spyOn(client, 'updateCollectionRules');
    const editor = useRuleEditorStore();
    await editor.openEdit(made.id);
    expect(editor.editing?.id).toBe(made.id);
    const first = editor.draft.items[0] as DraftRow;
    expect([first.dimension, first.value]).toEqual(['slant', 'italic']);
    editor.setDimension(first, 'classification');
    editor.update(first, { value: 'sans' });
    expect(await editor.save()).toBe(true);
    expect(update).toHaveBeenCalledWith({
      collection: made.id,
      ruleGroup: { op: 'and', rules: [{ kind: 'filters', filters: { classification: ['sans'] } }] },
    });
    expect(collections.collections[0]!.rules).toEqual({ classification: ['sans'] });
  });

  it('adds and removes rows and nested groups', async () => {
    const editor = useRuleEditorStore();
    await editor.openNew();
    const group = editor.addGroup();
    const inner = editor.addRow(group);
    expect(editor.draft.items).toHaveLength(2);
    expect((editor.draft.items[1] as typeof group).rows).toHaveLength(2);
    editor.remove(inner);
    expect((editor.draft.items[1] as typeof group).rows).toHaveLength(1);
    editor.remove(group);
    expect(editor.draft.items).toHaveLength(1);
  });
});
