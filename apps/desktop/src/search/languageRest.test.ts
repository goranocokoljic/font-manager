// E58.2: "Then the rest" under Settings › Languages is the API's answer after the user's
// languages, cut at the ten the filter drawer shows. The answer here is in an order no rule
// would give (not A to Z), so a split that sorts or regroups shows something else.
import { describe, expect, it } from 'vitest';
import type { LanguageGroup, LanguagePreset } from '../api/types';
import { LANGUAGES_SHOWN, languageRest } from './languages';

const preset = (code: string, group: LanguageGroup): LanguagePreset => ({
  code,
  name: `Name ${code}`,
  script: 'Latn',
  group,
});

/** `mine` codes, then `mostUsed`, then `others`, each in the order given. */
const answer = (mine: string[], mostUsed: string[], others: string[]) => [
  ...mine.map((c) => preset(c, 'mine')),
  ...mostUsed.map((c) => preset(c, 'mostUsed')),
  ...others.map((c) => preset(c, 'others')),
];

const codes = (list: readonly { code: string }[]) => list.map((p) => p.code);

describe('languageRest', () => {
  it('completes the first ten after the user’s languages, with their places', () => {
    const list = answer(['m1', 'm2', 'm3'], ['u1', 'u2'], ['z', 'y', 'x', 'w', 'v', 'q', 'r']);
    const rest = languageRest(list);
    expect(rest.first.map((r) => [r.preset.code, r.place])).toEqual([
      ['u1', 4],
      ['u2', 5],
      ['z', 6],
      ['y', 7],
      ['x', 8],
      ['w', 9],
      ['v', 10],
    ]);
    expect(rest.first).toHaveLength(LANGUAGES_SHOWN - 3);
    expect(rest.more).toEqual([]);
    expect(codes(rest.others)).toEqual(['q', 'r']);
  });

  it('puts the most used past the tenth behind "Show more" first, then the others as answered', () => {
    const mine = ['m1', 'm2', 'm3', 'm4', 'm5', 'm6', 'm7', 'm8'];
    const list = answer(mine, ['u1', 'u2', 'u3', 'u4'], ['z', 'a', 'k']);
    const rest = languageRest(list);
    expect(codes(rest.first.map((r) => r.preset))).toEqual(['u1', 'u2']);
    expect(codes(rest.more)).toEqual(['u3', 'u4']);
    expect(codes(rest.others)).toEqual(['z', 'a', 'k']);
  });

  it('never lists the user’s languages, even past the tenth', () => {
    const mine = Array.from({ length: 12 }, (_, i) => `m${i}`);
    const rest = languageRest(answer(mine, ['u1'], ['o1']));
    expect(rest.first).toEqual([]);
    expect(codes(rest.more)).toEqual(['u1']);
    expect(codes(rest.others)).toEqual(['o1']);
  });

  it('gives nothing for an empty answer', () => {
    expect(languageRest([])).toEqual({ first: [], more: [], others: [] });
  });
});
