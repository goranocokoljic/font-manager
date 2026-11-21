import { describe, expect, it } from 'vitest';
import cases from '../../../../fixtures/duplicates/pair-cases.json';
import { createMockClient } from './mock';
import type { SetFile } from './mockDuplicates';
import { listFolderPairs, type PairRoot } from './mockFolderPairs';
import type { FontFormat, ListFolderPairsRequest } from './types';

describe('MockApi folder pairs (E56.6)', () => {
  it('gives the core answer for every shared case', () => {
    expect(cases.length).toBeGreaterThanOrEqual(9);
    for (const c of cases) {
      const files: SetFile[] = c.files.map((f) => ({
        path: f.path,
        size: (f as { size?: number }).size ?? 1000,
        hash: f.hash,
        format: f.format as FontFormat | null,
        source: f.source,
        inLibraryFolder: f.kind === 'libraryFolder',
        system: f.kind === 'system',
        faces: f.faces,
      }));
      const got = listFolderPairs(
        files,
        c.roots as PairRoot[],
        c.request as ListFolderPairsRequest,
      );
      expect(got, c.name).toEqual(c.pairs);
    }
  });

  it('lists a pair through the client and names the files only one side has', async () => {
    const client = createMockClient();
    expect((await client.listFolderPairs({})).total).toBe(0);
    await expect(
      client.listFolderPairs({ scope: { kind: 'source', source: 999 } }),
    ).rejects.toMatchObject({ code: 'notFound' });
    await expect(client.listFolderPairFiles({ a: '/x', b: '/x' })).rejects.toMatchObject({
      code: 'invalidRequest',
    });
  });
});
