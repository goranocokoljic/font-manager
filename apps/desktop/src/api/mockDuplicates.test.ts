import { describe, expect, it } from 'vitest';
import cases from '../../../../fixtures/duplicates/set-cases.json';
import library from '../../../../fixtures/duplicates/mock-library.json';
import { createMockClient } from './mock';
import { duplicateSets, listDuplicateSets, versionKey, type SetFile } from './mockDuplicates';
import type { DuplicateScope, DuplicateSet, FontFormat, Paged } from './types';

/** The fields of a set the case file states. */
const summary = (sets: DuplicateSet[]) =>
  sets.map((s) => ({
    id: s.id,
    level: s.level,
    postscriptName: s.postscriptName,
    formatsDiffer: s.formatsDiffer,
    copies: s.copies.map((c) => ({
      path: c.path,
      face: c.face,
      version: c.version,
      system: c.system,
      inScope: c.inScope,
      identicalWith: c.identicalWith,
      otherSets: c.otherSets,
    })),
  }));

describe('MockApi duplicate sets (E56.1)', () => {
  it('gives the core answer for every shared case', () => {
    expect(cases.length).toBeGreaterThanOrEqual(10);
    for (const c of cases) {
      const files: SetFile[] = c.files.map((f) => ({
        path: f.path,
        size: 1000,
        hash: f.hash,
        format: f.format as FontFormat | null,
        source: f.source,
        inLibraryFolder: f.kind === 'libraryFolder',
        system: f.kind === 'system',
        faces: f.faces,
      }));
      const scope = ((c as { scope?: DuplicateScope }).scope ?? {
        kind: 'library',
      }) as DuplicateScope;
      expect(summary(duplicateSets(files, scope)), c.name).toEqual(c.sets);
    }
  });

  it('compares versions by digit runs; no version is its own value', () => {
    expect(versionKey('Version 1.000')).toBe(versionKey(' version 1.0'));
    expect(versionKey('Version 2.9')).not.toBe(versionKey('Version 2.10'));
    expect(versionKey(null)).toBe(versionKey('  '));
    expect(versionKey(null)).not.toBe(versionKey('1.0'));
  });

  it('caps digit runs past u64 as fm-core does, and a page at 500 sets', () => {
    expect(versionKey('1.99999999999999999999')).toBe(versionKey('1.99999999999999999998'));
    const files: SetFile[] = Array.from({ length: 1002 }, (_, i) => ({
      path: `/f/${i}.ttf`,
      size: 1,
      hash: `h${Math.floor(i / 2)}`,
      format: 'ttf',
      source: 1,
      inLibraryFolder: false,
      system: false,
      faces: [],
    }));
    const page = listDuplicateSets(files, { page: { offset: 0, limit: 10_000 } });
    expect([page.total, page.items.length]).toEqual([501, 500]);
  });

  it('lists the sets the Rust mock lists for the same library', async () => {
    expect((await createMockClient().listDuplicateSets({})).total).toBe(0);
    const client = createMockClient({ copies: library.copies });
    const got: Paged<DuplicateSet> = await client.listDuplicateSets({});
    // The face ids of the added files are each mock's own counter.
    const added = new Set(library.copies.map((c) => c.path));
    for (const s of got.items) for (const c of s.copies) if (added.has(c.path)) c.face = null;
    expect(got).toEqual(library.sets);
  });

  it('pages, scopes and refuses an unknown source', async () => {
    const client = createMockClient({ copies: library.copies });
    const all = await client.listDuplicateSets({});
    const second = await client.listDuplicateSets({ page: { offset: 1, limit: 1 } });
    expect([second.total, second.items.length]).toEqual([2, 1]);
    expect(second.items[0]).toEqual(all.items[1]);
    const folder = await client.listDuplicateSets({
      scope: { kind: 'folder', path: '/Users/demo/Fonts/Older' },
    });
    expect(folder.total).toBe(1);
    expect(folder.items[0]!.level).toBe('otherVersion');
    expect(folder.items[0]!.copies.filter((c) => c.inScope)).toHaveLength(1);
    await expect(
      client.listDuplicateSets({ scope: { kind: 'source', source: 999 } }),
    ).rejects.toMatchObject({ code: 'notFound' });
  });

  it('compares two faces and refuses the same face twice or an unknown one', async () => {
    const client = createMockClient();
    const families = await client.listFamilies({});
    const [one, two] = families.items.map((f) => f.id);
    const a = (await client.getFamily(one!)).faces[0]!.id;
    const b = (await client.getFamily(two!)).faces[0]!.id;
    const c = await client.compareCopies({ a, b });
    expect([c.a.face, c.b.face]).toEqual([a, b]);
    expect(c.a.health).toEqual({ kind: 'ok' });
    expect(c.differences).not.toBeNull();
    expect(c.summary).toHaveLength(1);
    await expect(client.compareCopies({ a, b: a })).rejects.toMatchObject({
      code: 'invalidRequest',
    });
    await expect(client.compareCopies({ a, b: 999_999 })).rejects.toMatchObject({
      code: 'notFound',
    });
  });
});
