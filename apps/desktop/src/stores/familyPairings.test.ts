import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import type { PairingSuggestion } from '../api/types';
import { candidateRole, FAMILY_PAIRING_LIMIT, useFamilyPairingsStore } from './familyPairings';

// Twenty sample families after the mock's own: the odd ones copy Inter (face 1) and pair with it.
let client: FmClient;

function setup(extraFaces = 20) {
  client = createMockClient({ extraFaces });
  setClient(client);
  return useFamilyPairingsStore();
}

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('family pairings', () => {
  it('asks for body faces while the family sets the headline, at most eight', async () => {
    const pairings = setup();
    const spy = vi.spyOn(client, 'suggestPairings');
    await pairings.load(1);
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: FAMILY_PAIRING_LIMIT, role: 'body' });
    expect(FAMILY_PAIRING_LIMIT).toBe(8);
    expect(pairings.results).toHaveLength(8);
    expect(pairings.results[0]!.reasons[0]).toBe('lower contrast for body');
    expect(pairings.loading).toBe(false);
  });

  it('the switch sends the other role and swaps the parts in the cards', async () => {
    const pairings = setup();
    await pairings.load(1);
    const first = pairings.results[0]!;
    expect(pairings.cardOf(first)).toEqual({
      suggestion: first,
      headline: 1,
      body: first.face.id,
    });

    const spy = vi.spyOn(client, 'suggestPairings');
    await pairings.setFamilyRole('body');
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: FAMILY_PAIRING_LIMIT, role: 'headline' });
    const again = pairings.results[0]!;
    // The headline rule gives no body reason: the answer is the headline request's.
    expect(again.reasons).not.toContain('lower contrast for body');
    expect(pairings.cardOf(again)).toEqual({ suggestion: again, headline: again.face.id, body: 1 });
  });

  it('keeps the role while the face changes, and asks nothing for the same role', async () => {
    const pairings = setup();
    await pairings.setFamilyRole('body');
    const spy = vi.spyOn(client, 'suggestPairings');
    await pairings.load(EXTRA_ID_BASE);
    expect(spy).toHaveBeenLastCalledWith({
      face: EXTRA_ID_BASE,
      limit: FAMILY_PAIRING_LIMIT,
      role: 'headline',
    });
    spy.mockClear();
    await pairings.setFamilyRole('body');
    expect(spy).not.toHaveBeenCalled();
  });

  it('shows no more than eight cards when the answer has more', async () => {
    const pairings = setup();
    const many = (await client.suggestPairings({ face: 1, limit: 8, role: 'body' })).concat(
      await client.suggestPairings({ face: 1, limit: 8, role: 'headline' }),
    );
    expect(many.length).toBeGreaterThan(8);
    vi.spyOn(client, 'suggestPairings').mockResolvedValueOnce(many);
    await pairings.load(1);
    expect(pairings.results).toHaveLength(8);
  });

  it('is empty, not loading, when the library has no candidate', async () => {
    const pairings = setup(0);
    await pairings.load(1);
    expect(pairings.results).toEqual([]);
    expect(pairings.loading).toBe(false);
    expect(pairings.error).toBeNull();
  });

  it('drops an older answer, keeps an error and clears without a face', async () => {
    const pairings = setup();
    let release: (v: PairingSuggestion[]) => void = () => undefined;
    vi.spyOn(client, 'suggestPairings').mockImplementationOnce(
      () => new Promise((resolve) => (release = resolve)),
    );
    const first = pairings.load(1);
    await pairings.load(EXTRA_ID_BASE + 1);
    const fresh = pairings.results;
    release([{ face: (await client.getFace(6)).face, score: 1, reasons: ['stale'] }]);
    await first;
    expect(pairings.results).toBe(fresh);
    expect(pairings.face).toBe(EXTRA_ID_BASE + 1);

    vi.spyOn(client, 'suggestPairings').mockRejectedValueOnce({
      code: 'notFound',
      message: 'gone',
      detail: null,
    });
    await pairings.load(1);
    expect(pairings.error?.message).toBe('gone');
    expect(pairings.loading).toBe(false);

    await pairings.load(null);
    expect(pairings.results).toEqual([]);
    expect(pairings.error).toBeNull();
    expect(pairings.cardOf({ face: (await client.getFace(1)).face, score: 1, reasons: [] })).toBe(
      null,
    );
  });

  it('names the candidates the other part', () => {
    expect(candidateRole('headline')).toBe('body');
    expect(candidateRole('body')).toBe('headline');
  });
});
