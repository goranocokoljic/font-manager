import { describe, expect, it } from 'vitest';
import type { FilterCounts } from '../api/types';
import {
  chipGroups,
  chosenLanguages,
  hasFilters,
  isChosen,
  languageName,
  toggleChip,
  toggleLanguage,
} from './chips';

const counts: FilterCounts = {
  slant: [
    { value: 'upright', count: 12 },
    { value: 'italic', count: 3 },
  ],
  formats: [
    { value: 'ttf', count: 9 },
    { value: 'otf', count: 4 },
  ],
  features: [{ value: 'smcp', count: 2 }],
  sources: [{ value: 7, count: 11 }],
  variable: [{ value: true, count: 5 }],
};

describe('filter chips', () => {
  it('shows one chip per counted value, grouped, with its count', () => {
    const groups = chipGroups({}, counts, (id) => `folder-${id}`);
    expect(groups.map((g) => g.label)).toEqual([
      'Slant',
      'Variable',
      'Format',
      'Features',
      'Source',
    ]);
    expect(groups[0]!.chips.map((c) => [c.label, c.count, c.selected])).toEqual([
      ['Upright', 12, false],
      ['Italic', 3, false],
    ]);
    expect(groups.find((g) => g.facet === 'sources')!.chips[0]!.label).toBe('folder-7');
  });

  it('marks chosen chips and keeps a chosen value with no result, at 0', () => {
    const groups = chipGroups({ slant: 'italic', formats: ['woff2'] }, counts);
    const slant = groups.find((g) => g.facet === 'slant')!;
    expect(slant.chips.find((c) => c.value === 'italic')!.selected).toBe(true);
    const woff2 = groups.find((g) => g.facet === 'formats')!.chips.find((c) => c.value === 'woff2');
    expect(woff2).toMatchObject({ label: 'WOFF2', count: 0, selected: true });
  });

  it('shows favorite, rating and tag chips, tags by name', () => {
    const groups = chipGroups(
      { tags: [4] },
      {
        favorite: [{ value: true, count: 2 }],
        minRating: [
          { value: 3, count: 4 },
          { value: 5, count: 1 },
        ],
        tags: [
          { value: 4, count: 6 },
          { value: 9, count: 2 },
        ],
      },
      undefined,
      (id) => (id === 4 ? 'display' : 'brand'),
    );
    expect(groups.map((g) => g.label)).toEqual(['Favorite', 'Rating', 'Tag']);
    expect(groups[0]!.chips.map((c) => c.label)).toEqual(['Favorites']);
    expect(groups[1]!.chips.map((c) => c.label)).toEqual(['3+ stars', '5 stars']);
    expect(groups[2]!.chips.map((c) => [c.label, c.count, c.selected])).toEqual([
      ['display', 6, true],
      ['brand', 2, false],
    ]);
  });

  it('tags add up (every one), rating and favorite take one value', () => {
    let f = toggleChip({}, 'tags', 4);
    f = toggleChip(f, 'tags', 9);
    expect(f).toEqual({ tags: [4, 9] });
    expect(toggleChip(f, 'tags', 4)).toEqual({ tags: [9] });
    f = toggleChip({}, 'minRating', 3);
    expect(toggleChip(f, 'minRating', 5)).toEqual({ minRating: 5 });
    expect(toggleChip(f, 'minRating', 3)).toEqual({});
    expect(toggleChip({}, 'favorite', true)).toEqual({ favorite: true });
  });

  it('leaves out groups the API counted nothing for', () => {
    expect(chipGroups({}, {})).toEqual([]);
  });

  it('a single-value filter takes the new value; choosing it again lets it go', () => {
    let f = toggleChip({}, 'slant', 'italic');
    expect(f).toEqual({ slant: 'italic' });
    f = toggleChip(f, 'slant', 'upright');
    expect(f).toEqual({ slant: 'upright' });
    expect(toggleChip(f, 'slant', 'upright')).toEqual({});
  });

  it('a false boolean is a chosen value, not an empty one', () => {
    const f = toggleChip({}, 'variable', false);
    expect(f).toEqual({ variable: false });
    expect(isChosen(f, 'variable', false)).toBe(true);
    expect(hasFilters(f)).toBe(true);
  });

  it('any-of and every-of filters collect values and drop the empty list', () => {
    let f = toggleChip({}, 'formats', 'ttf');
    f = toggleChip(f, 'formats', 'otf');
    expect(f.formats).toEqual(['ttf', 'otf']);
    f = toggleChip(toggleChip(f, 'formats', 'ttf'), 'formats', 'otf');
    expect(f).toEqual({});
    expect(toggleChip({ features: ['kern'] }, 'features', 'smcp')).toEqual({
      features: ['kern', 'smcp'],
    });
  });

  it('does not change the filters it is given', () => {
    const f = { formats: ['ttf' as const] };
    toggleChip(f, 'formats', 'otf');
    expect(f).toEqual({ formats: ['ttf'] });
  });

  it('languages add up in one supports filter (all of them) and go away with the last', () => {
    let f = toggleLanguage({}, 'pl');
    f = toggleLanguage(f, 'sr-Cyrl');
    expect(f.supports).toEqual({ kind: 'languages', codes: ['pl', 'sr-Cyrl'] });
    expect(chosenLanguages(f)).toEqual(['pl', 'sr-Cyrl']);
    f = toggleLanguage(toggleLanguage(f, 'pl'), 'sr-Cyrl');
    expect(f).toEqual({});
    expect(hasFilters({ supports: { kind: 'text', text: 'ж' } })).toBe(true);
    expect(chosenLanguages({ supports: { kind: 'text', text: 'ж' } })).toEqual([]);
  });

  it('names a language by its preset, or by its code when unknown', () => {
    const presets = [{ code: 'pl', name: 'Polish', script: 'Latn' }];
    expect(languageName(presets, 'pl')).toBe('Polish');
    expect(languageName(presets, 'xx')).toBe('xx');
  });

  it('no filter is no filter', () => {
    expect(hasFilters({})).toBe(false);
    expect(hasFilters({ formats: [], slant: null })).toBe(false);
  });
});
