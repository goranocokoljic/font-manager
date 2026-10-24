import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE } from '../api/mock';
import type { FaceSummary, PairingSuggestion } from '../api/types';
import { useCompareStore } from './compare';
import { PAIRING_LIMIT, usePairingsStore } from './pairings';

// Three sample families, then Sample 1 (an Inter copy) and Sample 2 (a JetBrains Mono copy).
let client: FmClient;

function setup() {
  client = createMockClient({ extraFaces: 2 });
  setClient(client);
  return usePairingsStore();
}

const face = async (id: number): Promise<FaceSummary> => (await client.getFace(id)).face;

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('pairings', () => {
  it('asks suggestPairings for the face and keeps the rule-ranked answer', async () => {
    const pairings = setup();
    const spy = vi.spyOn(client, 'suggestPairings');
    const inter = await face(1);
    await pairings.load(inter);
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: PAIRING_LIMIT });
    expect(pairings.base?.id).toBe(1);
    expect(pairings.loading).toBe(false);
    expect(pairings.error).toBeNull();
    // The Inter copy pairs by designer and x-height; the mono copy does not.
    expect(pairings.results.map((s) => s.face.familyName)).toEqual(['Sample 1']);
    expect(pairings.results[0]!.reasons).toEqual(['same designer', 'similar x-height']);
    expect(pairings.results[0]!.score).toBeCloseTo(0.3);
  });

  it('drops an older answer and clears without a face', async () => {
    const pairings = setup();
    let release: (v: PairingSuggestion[]) => void = () => undefined;
    vi.spyOn(client, 'suggestPairings').mockImplementationOnce(
      () => new Promise((resolve) => (release = resolve)),
    );
    const first = pairings.load(await face(1));
    const second = pairings.load(await face(EXTRA_ID_BASE));
    await second;
    const fresh = pairings.results;
    release([{ face: await face(6), score: 1, reasons: ['stale'] }]);
    await first;
    expect(pairings.base?.id).toBe(EXTRA_ID_BASE);
    expect(pairings.results).toBe(fresh);
    expect(pairings.results.some((s) => s.reasons.includes('stale'))).toBe(false);

    await pairings.load(null);
    expect(pairings.base).toBeNull();
    expect(pairings.results).toEqual([]);
    expect(pairings.loading).toBe(false);
  });

  it('keeps the error of a failed call', async () => {
    const pairings = setup();
    await pairings.load({ ...(await face(1)), id: 99_999 });
    expect(pairings.error?.code).toBe('notFound');
    expect(pairings.results).toEqual([]);
    expect(pairings.loading).toBe(false);
  });

  it('adds a suggestion to Compare through its pin', async () => {
    const pairings = setup();
    const compare = useCompareStore();
    const sample = await face(EXTRA_ID_BASE);
    expect(pairings.add(sample)).toBe('pinned');
    expect(compare.isPinned(sample.id)).toBe(true);
    expect(pairings.add(sample)).toBe('already');
  });
});
