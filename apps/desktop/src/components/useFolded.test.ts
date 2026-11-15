import { beforeEach, describe, expect, it } from 'vitest';
import { useFolded } from './useFolded';

beforeEach(() => localStorage.clear());

describe('useFolded', () => {
  it('starts unfolded unless the group asks to start folded (E50.12)', () => {
    expect(useFolded('a').folded.value).toBe(false);
    expect(useFolded('b', true).folded.value).toBe(true);
  });

  it('remembers what the user chose over the starting state', () => {
    useFolded('foundries', true).toggle();
    expect(useFolded('foundries', true).folded.value).toBe(false);
    useFolded('tags').toggle();
    expect(useFolded('tags', false).folded.value).toBe(true);
  });
});
