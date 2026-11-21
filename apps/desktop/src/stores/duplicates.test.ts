import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import {
  createMockClient,
  mockDuplicateCopies,
  mockFolderPairCopies,
  MOCK_FOLDER_PAIR_FACES,
} from '../api/mock';
import { pairShare, QUEUE_PAGE, useDuplicatesStore } from './duplicates';
import type { DuplicateSet, FolderPair, ListDuplicateSetsRequest } from '../api/types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ copies: mockDuplicateCopies });
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('the duplicates store', () => {
  it('loads the overview and the queue of the levels to decide, without identical sets', async () => {
    const store = useDuplicatesStore();
    await store.load();
    expect(store.overview?.fonts).toBe(3);
    expect(store.overview?.identical.sets).toBe(1);
    await store.reload(0);
    expect(store.total).toBe(2);
    expect(store.current?.id).toBe('name:Inter-Regular');
  });

  it('starts each copy at what the API plans for it and marks the API keeper', async () => {
    const store = useDuplicatesStore();
    await store.reload(0);
    expect(store.keeper).toBe('/Users/demo/Fonts/Inter/Inter[wght].ttf');
    expect(store.reason).toBe('Stays: it is the copy that is kept.');
    expect(store.choices).toEqual({
      '/Users/demo/Fonts/Inter/Inter[wght].ttf': 'keep',
      '/Users/demo/Fonts/Old/Inter-Regular.ttf': 'trash',
      '/Users/demo/Fonts/Older/Inter-Regular.ttf': 'trash',
    });
  });

  it('compares the keeper by default, or the two copies the user picked (E56.13)', async () => {
    const store = useDuplicatesStore();
    await store.reload(0);
    await store.compare();
    expect(store.comparison?.a.path).toBe('/Users/demo/Fonts/Inter/Inter[wght].ttf');
    expect(store.comparison?.b.path).toBe('/Users/demo/Fonts/Old/Inter-Regular.ttf');
    await store.compare(
      '/Users/demo/Fonts/Inter/Inter[wght].ttf',
      '/Users/demo/Fonts/Older/Inter-Regular.ttf',
    );
    expect(store.comparison?.a.path).toBe('/Users/demo/Fonts/Older/Inter-Regular.ttf');
    expect(store.comparison?.b.path).toBe('/Users/demo/Fonts/Inter/Inter[wght].ttf');
    expect(store.comparison?.differences?.languagesGained).toEqual(['sr-Cyrl']);
    // The same copy twice compares it with another one, never with itself.
    await store.compare(
      '/Users/demo/Fonts/Older/Inter-Regular.ttf',
      '/Users/demo/Fonts/Older/Inter-Regular.ttf',
    );
    expect(store.comparison?.a.path).toBe('/Users/demo/Fonts/Older/Inter-Regular.ttf');
    expect(store.comparison?.b.path).not.toBe('/Users/demo/Fonts/Older/Inter-Regular.ttf');
    // A new set closes the comparison.
    await store.select(1);
    expect(store.comparison).toBeNull();
  });

  it('sends a decision to the API and shows the set that took its place', async () => {
    const store = useDuplicatesStore();
    await store.reload(0);
    store.keepOnly('/Users/demo/Fonts/Older/Inter-Regular.ttf');
    store.choose('/Users/demo/Fonts/Inter/Inter[wght].ttf', 'hide');
    expect(await store.decide('copies')).toBe(true);
    expect(store.current?.id).toBe('name:LegacyGrotesk');
    expect(store.total).toBe(1);
    const stored = await client.getCleanupDraft();
    expect(stored.decisions?.[0]).toMatchObject({
      set: 'name:Inter-Regular',
      decision: {
        kind: 'copies',
        copies: [
          { path: '/Users/demo/Fonts/Inter/Inter[wght].ttf', choice: 'hide' },
          { path: '/Users/demo/Fonts/Old/Inter-Regular.ttf', choice: 'trash' },
          { path: '/Users/demo/Fonts/Older/Inter-Regular.ttf', choice: 'keep' },
        ],
      },
    });
    expect(await store.decide('notDuplicates')).toBe(true);
    expect(store.current).toBeNull();
    expect(store.total).toBe(0);
    await store.takeBack('name:LegacyGrotesk');
    expect(store.current?.id).toBe('name:LegacyGrotesk');
  });

  it('asks the API for the filtered queue', async () => {
    const store = useDuplicatesStore();
    const list = vi.spyOn(client, 'listDuplicateSets');
    await store.reload(0);
    await store.setFilter({ level: 'sameVersion', format: 'type1' });
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: {
          levels: ['sameVersion'],
          format: 'type1',
          recommendation: null,
          undecided: true,
          ids: [],
        },
        sort: 'name',
      }),
    );
    expect(store.total).toBe(1);
    await store.setFilter({ level: null, format: null, recommendation: 'hasRecommendation' });
    expect(store.total).toBe(0);
    expect(store.current).toBeNull();
  });

  it('narrows the queue to a folder or a source through the request scope', async () => {
    const store = useDuplicatesStore();
    const list = vi.spyOn(client, 'listDuplicateSets');
    await store.reload(0);
    expect(store.total).toBe(2);

    // Only Inter has a copy in the Older folder.
    const folder = { kind: 'folder' as const, path: '/Users/demo/Fonts/Older' };
    await store.setFilter({ scope: folder });
    expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ scope: folder }));
    expect(store.total).toBe(1);
    expect(store.current?.id).toBe('name:Inter-Regular');

    // Every copy of the mock is in one source: that source lists both sets, another none.
    await store.setFilter({ scope: { kind: 'library' } });
    const own = store.at(0)!.copies[0]!.source;
    const source = { kind: 'source' as const, source: own };
    await store.setFilter({ scope: source });
    expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ scope: source }));
    expect(store.total).toBe(2);
    await store.setFilter({ scope: { kind: 'source', source: own + 1 } });
    expect(store.total).toBe(0);
    expect(store.current).toBeNull();
  });

  it('the Keep both chip asks for the keep-both sets, and All drops that filter again', async () => {
    const store = useDuplicatesStore();
    const list = vi.spyOn(client, 'listDuplicateSets');
    await store.reload(0);
    await store.showChip('keepBoth');
    expect(store.chip).toBe('keepBoth');
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: expect.objectContaining({
          levels: ['sameVersion', 'otherVersion'],
          recommendation: 'keepBoth',
        }),
      }),
    );
    await store.showChip('sameVersion');
    expect(store.chip).toBe('sameVersion');
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: expect.objectContaining({ levels: ['sameVersion'], recommendation: null }),
      }),
    );
    // A recommendation filter of its own stays when the level changes.
    await store.setFilter({ recommendation: 'hasRecommendation' });
    await store.showChip('all');
    expect(store.filter.recommendation).toBe('hasRecommendation');
    expect(store.chip).toBe('all');
  });

  it('pages through the API: a position past the first page loads that page', async () => {
    const store = useDuplicatesStore();
    const TOTAL = QUEUE_PAGE * 2 + 50;
    const one = (i: number): DuplicateSet => ({
      id: `name:S${i}`,
      level: 'sameVersion',
      postscriptName: `S${i}`,
      formatsDiffer: false,
      copies: [],
      recommendation: null,
    });
    const list = vi
      .spyOn(client, 'listDuplicateSets')
      .mockImplementation(async (req: ListDuplicateSetsRequest) => {
        const offset = req.page?.offset ?? 0;
        const limit = req.page?.limit ?? QUEUE_PAGE;
        const items = [];
        for (let i = offset; i < Math.min(TOTAL, offset + limit); i++) items.push(one(i));
        return { items, total: TOTAL };
      });
    await store.reload(0);
    expect(store.total).toBe(TOTAL);
    expect(list).toHaveBeenCalledTimes(1);
    expect(store.at(QUEUE_PAGE + 50)).toBeUndefined();

    await store.ensure(QUEUE_PAGE + 50);
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({ page: { offset: QUEUE_PAGE, limit: QUEUE_PAGE } }),
    );
    expect(store.at(QUEUE_PAGE + 50)?.id).toBe(`name:S${QUEUE_PAGE + 50}`);
    expect(store.at(QUEUE_PAGE - 1)?.id).toBe(`name:S${QUEUE_PAGE - 1}`);
    // A loaded page is not asked for again.
    await store.ensure(QUEUE_PAGE + 1);
    expect(list).toHaveBeenCalledTimes(2);

    // Selecting a set on the last page loads it and shows that set.
    await store.select(TOTAL - 1);
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({ page: { offset: QUEUE_PAGE * 2, limit: QUEUE_PAGE } }),
    );
    expect(store.cursor).toBe(TOTAL - 1);
    expect(store.current?.id).toBe(`name:S${TOTAL - 1}`);
  });
});

describe('the duplicates store: folder pairs (E56.12)', () => {
  const SAMPLES = '/Users/demo/Fonts/Samples';
  const OLD = '/Users/demo/Downloads/fonts-old';
  const SYSTEM = '/System/Library/Fonts/Supplemental';
  const withPairs = () => {
    setClient(
      createMockClient({ extraFaces: MOCK_FOLDER_PAIR_FACES, copies: mockFolderPairCopies }),
    );
    return useDuplicatesStore();
  };
  const find = (store: ReturnType<typeof useDuplicatesStore>, path: string) => {
    for (let i = 0; i < (store.pairTotal ?? 0); i++) {
      const p = store.pairAt(i);
      if (p && (p.a.path === path || p.b.path === path)) return p;
    }
    throw new Error(`no pair with ${path}`);
  };

  it('pages the pairs in the API order and counts them for the navigation', async () => {
    const store = withPairs();
    await store.reloadPairs();
    expect(store.pairTotal).toBe(3);
    const frees = [0, 1, 2].map((i) => store.pairAt(i)!.frees);
    expect([...frees].sort((a, b) => b - a)).toEqual(frees);
  });

  it("'Keep this side' puts the pair into the draft with what it covers, and counts it", async () => {
    const store = withPairs();
    await store.load();
    await store.reloadPairs();
    const pair = find(store, OLD);
    const keepSamples = pair.a.path === SAMPLES ? 'a' : 'b';
    expect(store.planned).toBe(0);
    expect(await store.keepSide(pair, keepSamples)).toBe(true);
    expect(store.draftPair(pair)).toMatchObject({ keep: SAMPLES, remove: OLD, identical: 5 });
    expect(store.planned).toBe(1);
    expect(await store.takeBackPair(OLD, SAMPLES)).toBe(true);
    expect(store.draftPair(pair)).toBeUndefined();
    expect(store.planned).toBe(0);
  });

  it('shows the refusal when the side to remove is a system folder', async () => {
    const store = withPairs();
    await store.reloadPairs();
    const pair = find(store, SYSTEM);
    const keepSamples = pair.a.path === SAMPLES ? 'a' : 'b';
    expect(await store.keepSide(pair, keepSamples)).toBe(false);
    expect(store.error).toMatchObject({ code: 'invalidRequest' });
    expect(store.draftPair(pair)).toBeUndefined();
  });

  it('opens a pair to its lists: both, not identical, and only one side', async () => {
    const store = withPairs();
    await store.reloadPairs();
    const pair = find(store, OLD);
    await store.showPair(pair);
    const oldIsA = pair.a.path === OLD;
    expect(store.pairFiles.both?.total).toBe(5);
    expect(store.pairFiles.differ?.total).toBe(1);
    expect(store.pairFiles.differ?.items[0]?.level).toBe('otherVersion');
    expect(store.pairFiles[oldIsA ? 'onlyB' : 'onlyA']?.total).toBe(12);
    expect(store.pairFiles[oldIsA ? 'onlyA' : 'onlyB']?.total).toBe(0);
    await store.showPair(null);
    expect(store.pairFiles).toEqual({});
  });

  it('the share is the larger of both sides, from the API counts', () => {
    const side = (fontFiles: number, identical: number, otherVersion: number) =>
      ({
        path: '/x',
        fontFiles,
        size: 0,
        shared: { identical, sameVersion: 0, otherVersion },
        sharedSize: 0,
        identicalSize: 0,
        onlyHere: fontFiles - identical - otherVersion,
        otherFiles: 0,
        contained: false,
        protected: null,
      }) satisfies FolderPair['a'];
    expect(pairShare({ a: side(10, 4, 1), b: side(100, 5, 0), frees: 0 })).toBe(0.5);
    expect(pairShare({ a: side(0, 0, 0), b: side(4, 1, 0), frees: 0 })).toBe(0.25);
  });
});
