import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, type MockOptions } from '../api/mock';
import type { FaceSummary } from '../api/types';
import { faceKey, familyKey } from '../list/rows';
import { useFontListStore } from './fontList';
import { useLibraryStore } from './library';
import { useSearchStore } from './search';
import { scorePercent, SIMILAR_LIMIT, useSimilarStore } from './similar';

// Three sample families, then two extra ones with four styles each.
const A = EXTRA_ID_BASE;
let client: FmClient;

async function setup(options: MockOptions = {}) {
  client = createMockClient({ extraFaces: 2, stylesPerFamily: 4, ...options });
  setClient(client);
  await useLibraryStore().load();
  return { similar: useSimilarStore(), list: useFontListStore() };
}

const face = async (id: number): Promise<FaceSummary> => (await client.getFace(id)).face;

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('find similar', () => {
  it('asks findSimilar for the face and keeps its ranked answer', async () => {
    const { similar } = await setup();
    const spy = vi.spyOn(client, 'findSimilar');
    const inter = await face(1);
    await similar.openFor(inter);
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: SIMILAR_LIMIT });
    expect(similar.open).toBe(true);
    expect(similar.target?.id).toBe(1);
    expect(similar.loading).toBe(false);
    expect(similar.error).toBeNull();
    expect(similar.results.length).toBeGreaterThan(0);
    expect(similar.results.map((r) => r.face.id)).not.toContain(1);
    const scores = similar.results.map((r) => r.score ?? 0);
    expect(scores).toEqual([...scores].sort((a, b) => b - a));
    expect(similar.results[0]!.reasons.length).toBeGreaterThan(0);
  });

  it('keeps no results as an empty answer, not an error', async () => {
    const { similar } = await setup({ similar: 'none' });
    await similar.openFor(await face(1));
    expect(similar.open).toBe(true);
    expect(similar.results).toEqual([]);
    expect(similar.error).toBeNull();
  });

  it("keeps the core's error", async () => {
    const { similar } = await setup({ similar: 'fail' });
    await similar.openFor(await face(1));
    expect(similar.results).toEqual([]);
    expect(similar.error?.code).toBe('unsupported');
    expect(similar.loading).toBe(false);
  });

  it('drops an answer for a face it no longer shows', async () => {
    const { similar } = await setup();
    let release: () => void = () => undefined;
    const real = client.findSimilar.bind(client);
    vi.spyOn(client, 'findSimilar').mockImplementationOnce(
      (req) => new Promise((resolve) => (release = () => void real(req).then(resolve))),
    );
    const first = similar.openFor(await face(1));
    await similar.openFor(await face(6));
    const shown = similar.results.map((r) => r.face.id);
    release();
    await first;
    expect(similar.target?.id).toBe(6);
    expect(similar.results.map((r) => r.face.id)).toEqual(shown);
  });

  it('picking a result expands its family and selects its row', async () => {
    const { similar, list } = await setup();
    const third = await face(A + 2);
    expect(await similar.pick(third)).toBe(true);
    expect(similar.picked).toBe(A + 2);
    expect(list.expanded.has(A)).toBe(true);
    expect(list.cursor).toBe(faceKey(A + 2));
    expect([...list.selection.selected]).toEqual([faceKey(A + 2)]);
    expect(list.revealed).toBe(1);
  });

  it('picking a face whose family the list hides selects nothing in the list', async () => {
    const { similar, list } = await setup();
    // Only the monospaced families (JetBrains Mono, every second extra one) are listed.
    useSearchStore().setFilters({ classification: ['mono'] });
    await flushPromises();
    expect(list.rows.map((r) => r.key)).not.toContain(familyKey(1));
    list.moveCursor(0);
    const before = list.cursor;
    expect(await similar.pick(await face(1))).toBe(false);
    expect(list.cursor).toBe(before);
  });

  it('closing hides the panel and ignores a late answer', async () => {
    const { similar } = await setup();
    const pending = similar.openFor(await face(1));
    similar.close();
    await pending;
    expect(similar.open).toBe(false);
    expect(similar.results).toEqual([]);
  });
});

describe('scorePercent', () => {
  it('rounds a 0–1 score to a whole percent', () => {
    expect(scorePercent(0.9)).toBe(90);
    expect(scorePercent(0.876)).toBe(88);
    expect(scorePercent(1)).toBe(100);
    expect(scorePercent(0)).toBe(0);
  });
  it('has no percent for a missing score', () => {
    expect(scorePercent(null)).toBeNull();
    expect(scorePercent(Number.NaN)).toBeNull();
  });
});
