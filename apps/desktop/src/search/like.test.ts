import { describe, expect, it } from 'vitest';
import { likeThisLabel } from './like';

describe('likeThisLabel', () => {
  it('says "an" before a letter whose name starts with a vowel sound', () => {
    expect(likeThisLabel('R')).toBe('Fonts with an R like this');
    expect(likeThisLabel('a')).toBe('Fonts with an a like this');
    expect(likeThisLabel('S')).toBe('Fonts with an S like this');
  });

  it('says "a" before any other character', () => {
    expect(likeThisLabel('g')).toBe('Fonts with a g like this');
    expect(likeThisLabel('Ж')).toBe('Fonts with a Ж like this');
  });
});
