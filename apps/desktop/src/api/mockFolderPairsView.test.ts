import { describe, expect, it } from 'vitest';
import { createMockClient, mockFolderPairCopies, MOCK_FOLDER_PAIR_FACES } from './mock';

// The folder pairs of `?mockFolderPairs=1` (E56.12) through the mock client: the
// Folders view's Playwright spec relies on a contained pair, a partial pair and a
// system folder that can only be kept, and on the draft's "Keep this side".
const client = () =>
  createMockClient({ extraFaces: MOCK_FOLDER_PAIR_FACES, copies: mockFolderPairCopies });

const SAMPLES = '/Users/demo/Fonts/Samples';
const BACKUP = '/Users/demo/Backup 2014/Samples';
const OLD = '/Users/demo/Downloads/fonts-old';
const SYSTEM = '/System/Library/Fonts/Supplemental';

describe('MockApi folder pairs of the Folders view (E56.12)', () => {
  it('lists a contained pair, a partial pair and a protected system side', async () => {
    const pairs = (await client().listFolderPairs({})).items;
    const of = (path: string) => pairs.find((p) => p.a.path === path || p.b.path === path)!;
    const side = (path: string) => {
      const p = of(path);
      return p.a.path === path ? p.a : p.b;
    };
    expect(pairs).toHaveLength(3);
    expect(side(BACKUP)).toMatchObject({ contained: true, fontFiles: 6, protected: null });
    expect(side(OLD)).toMatchObject({
      contained: false,
      fontFiles: 6,
      shared: { identical: 5, otherVersion: 1 },
      protected: null,
    });
    expect(side(SYSTEM)).toMatchObject({ contained: true, protected: 'systemFolder' });
    const frees = pairs.map((p) => p.frees);
    expect([...frees].sort((a, b) => b - a)).toEqual(frees);
  });

  it('names what both have, what one side has and the copies that are not identical', async () => {
    const c = client();
    const files = (req: Partial<Parameters<typeof c.listFolderPairFiles>[0]>) =>
      c.listFolderPairFiles({ a: OLD, b: SAMPLES, ...req });
    expect((await files({ side: 'a', levels: ['identical'] })).total).toBe(5);
    const other = await files({ side: 'a', levels: ['sameVersion', 'otherVersion'] });
    expect(other.items.map((f) => f.path)).toEqual([`${OLD}/Sample12-Regular.ttf`]);
    expect((await files({ side: 'b', onlyHere: true })).total).toBe(12);
  });

  it("'Keep this side' adds the pair with its identical copies and refuses a system side", async () => {
    const c = client();
    const draft = await c.updateCleanupDraft({ kind: 'keepSide', keep: SAMPLES, remove: OLD });
    expect(draft.pairs).toEqual([
      expect.objectContaining({ keep: SAMPLES, remove: OLD, identical: 5 }),
    ]);
    // The other way round replaces it: one decision per pair of folders.
    const again = await c.updateCleanupDraft({ kind: 'keepSide', keep: OLD, remove: SAMPLES });
    expect(again.pairs).toHaveLength(1);
    expect(again.pairs![0]).toMatchObject({ remove: SAMPLES, identical: 5 });
    await expect(
      c.updateCleanupDraft({ kind: 'keepSide', keep: SAMPLES, remove: SYSTEM }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
    await expect(
      c.updateCleanupDraft({ kind: 'keepSide', keep: SAMPLES, remove: '/nowhere' }),
    ).rejects.toMatchObject({ code: 'notFound' });
    const back = await c.updateCleanupDraft({ kind: 'takeBackPair', a: SAMPLES, b: OLD });
    expect(back.pairs).toEqual([]);
  });

  it('opens a side in the file manager only when it holds library fonts', async () => {
    const c = client();
    const w = globalThis as { __fmMockShownFolders?: string[] };
    w.__fmMockShownFolders = [];
    await c.showPairFolder(BACKUP, SAMPLES, 'a');
    expect(w.__fmMockShownFolders).toEqual([BACKUP]);
    await expect(c.showPairFolder('/empty', SAMPLES, 'a')).rejects.toMatchObject({
      code: 'notFound',
    });
  });
});
