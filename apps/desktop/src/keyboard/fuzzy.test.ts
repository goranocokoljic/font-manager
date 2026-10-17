import { describe, expect, it } from 'vitest';
import { fuzzyMatch, highlight } from './fuzzy';

describe('fuzzy match', () => {
  it('matches characters in order, ignoring case and spaces', () => {
    expect(fuzzyMatch('insp', 'Show or hide the Inspector')).not.toBeNull();
    expect(fuzzyMatch('SHOW insp', 'Show or hide the inspector')).not.toBeNull();
    expect(fuzzyMatch('gtl', 'Go to the font list')).not.toBeNull();
  });

  it('rejects text missing a character or with them out of order', () => {
    expect(fuzzyMatch('xyz', 'Go to the sidebar')).toBeNull();
    expect(fuzzyMatch('rabedis', 'Go to the sidebar')).toBeNull();
  });

  it('matches everything for an empty query', () => {
    expect(fuzzyMatch('', 'anything')).toEqual({ score: 0, indices: [] });
    expect(fuzzyMatch('   ', 'anything')).toEqual({ score: 0, indices: [] });
  });

  it('prefers word starts for initials', () => {
    expect(fuzzyMatch('gtl', 'Go to list')?.indices).toEqual([0, 3, 6]);
  });

  it('still matches when the word-start jump would use up a needed character', () => {
    // "s" jumps to "saw", leaving no "e" after it; the plain pass finds "use".
    expect(fuzzyMatch('se', 'use saw')?.indices).toEqual([1, 2]);
  });

  it('ranks a word-start, contiguous match above a scattered one', () => {
    const tight = fuzzyMatch('side', 'Go to the sidebar')!;
    const loose = fuzzyMatch('side', 'Show or hide the inspector')!;
    expect(tight.score).toBeGreaterThan(loose.score);
  });

  it('splits a title into matched and unmatched runs', () => {
    expect(highlight('Go to', [0, 3, 4])).toEqual([
      { text: 'G', match: true },
      { text: 'o ', match: false },
      { text: 'to', match: true },
    ]);
    expect(highlight('abc', [])).toEqual([{ text: 'abc', match: false }]);
  });
});
