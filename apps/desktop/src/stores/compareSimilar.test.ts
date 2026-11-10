import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, mockFaces } from '../api/mock';
import type { ApiError, FaceSummary, SimilarFace } from '../api/types';
import { COMPARE_SIMILAR_LIMIT, groupByReason, useCompareSimilarStore } from './compareSimilar';

let client: FmClient;

function setup() {
  client = createMockClient({ extraFaces: 2 });
  setClient(client);
  return useCompareSimilarStore();
}

const face = async (id: number): Promise<FaceSummary> => (await client.getFace(id)).face;
/** A made-up face `id` (no API call). */
const made = (id: number): FaceSummary => ({ ...mockFaces[0]!.face, id });
const row = (f: FaceSummary, score: number, reasons: string[]): SimilarFace => ({
  face: f,
  score,
  reasons,
});

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('compare similar', () => {
  it('asks findSimilar for the base, up to 8, and keeps the ranked answer', async () => {
    const similar = setup();
    const spy = vi.spyOn(client, 'findSimilar');
    const inter = await face(1);
    await similar.load(inter);
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: COMPARE_SIMILAR_LIMIT });
    expect(COMPARE_SIMILAR_LIMIT).toBe(8);
    expect(similar.base?.id).toBe(1);
    expect(similar.loading).toBe(false);
    expect(similar.error).toBeNull();
    expect(similar.results.length).toBeGreaterThan(0);
    expect(similar.results.length).toBeLessThanOrEqual(COMPARE_SIMILAR_LIMIT);
    const scores = similar.results.map((r) => r.score ?? 0);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
  });

  it('never keeps more than 8 even when the API answers with more', async () => {
    const similar = setup();
    const many = Array.from({ length: 12 }, (_, i) => row(made(100 + i), 1 - i / 20, []));
    vi.spyOn(client, 'findSimilar').mockResolvedValueOnce(many);
    await similar.load(made(99));
    expect(similar.results).toHaveLength(COMPARE_SIMILAR_LIMIT);
  });

  it('drops an older base’s answer, keeps an error, and clears without a base', async () => {
    const similar = setup();
    let release: (v: SimilarFace[]) => void = () => undefined;
    vi.spyOn(client, 'findSimilar').mockImplementationOnce(
      () => new Promise((resolve) => (release = resolve)),
    );
    const first = similar.load(await face(1));
    const second = similar.load(await face(EXTRA_ID_BASE));
    await second;
    const fresh = similar.results;
    release([row(made(6), 1, ['stale'])]);
    await first;
    expect(similar.base?.id).toBe(EXTRA_ID_BASE);
    expect(similar.results).toBe(fresh);

    const err: ApiError = { code: 'unsupported', message: 'no', detail: '' };
    vi.spyOn(client, 'findSimilar').mockRejectedValueOnce(err);
    await similar.load(await face(1));
    expect(similar.error).toEqual(err);
    expect(similar.loading).toBe(false);

    await similar.load(null);
    expect(similar.base).toBeNull();
    expect(similar.results).toEqual([]);
    expect(similar.error).toBeNull();
    expect(similar.loading).toBe(false);
  });
});

describe('groupByReason', () => {
  it('names each shared reason once, groups in rank order, and drops the base and repeats', () => {
    const [a, b, c, d, base] = [1, 2, 3, 4, 5].map(made);
    const groups = groupByReason(
      [
        row(a!, 0.9, ['same designer', 'similar x-height']),
        row(b!, 0.8, ['same weight']),
        row(base!, 0.7, ['same weight']),
        row(c!, 0.6, ['same designer', 'similar x-height']),
        row(a!, 0.5, ['same weight']),
        row(d!, 0.4, []),
      ],
      base!,
    );
    expect(groups.map((g) => g.key)).toEqual([
      'same designer · similar x-height',
      'same weight',
      '',
    ]);
    expect(groups.map((g) => g.rows.map((r) => r.face.id))).toEqual([
      [a!.id, c!.id],
      [b!.id],
      [d!.id],
    ]);
    // Every face once: none repeated across groups, the base never listed.
    const ids = groups.flatMap((g) => g.rows.map((r) => r.face.id));
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids).not.toContain(base!.id);
  });

  it('is empty for an empty answer', () => {
    expect(groupByReason([])).toEqual([]);
  });
});
