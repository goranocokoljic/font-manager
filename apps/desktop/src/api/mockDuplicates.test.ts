import { describe, expect, it } from 'vitest';
import cases from '../../../../fixtures/duplicates/set-cases.json';
import library from '../../../../fixtures/duplicates/mock-library.json';
import { createMockClient, mockCopyCompareCopies, mockDuplicateCopies } from './mock';
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
      const decided = new Set((c as { decided?: string[] }).decided ?? []);
      expect(summary(duplicateSets(files, scope, decided)), c.name).toEqual(c.sets);
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
    expect(c.summary.length).toBeGreaterThan(0);
    await expect(client.compareCopies({ a, b: a })).rejects.toMatchObject({
      code: 'invalidRequest',
    });
    await expect(client.compareCopies({ a, b: 999_999 })).rejects.toMatchObject({
      code: 'notFound',
    });
  });
});

describe('MockApi copy comparison (E56.13)', () => {
  /** The faces of the Inter copies of `?mockDuplicates=1` and of the WOFF2 copy. */
  async function interCopies() {
    const client = createMockClient({ copies: [...mockDuplicateCopies, ...mockCopyCompareCopies] });
    const sets = await client.listDuplicateSets({});
    const inter = sets.items.find((s) => s.postscriptName === 'Inter-Regular')!;
    const legacy = sets.items.find((s) => s.postscriptName === 'LegacyGrotesk')!;
    const at = (set: typeof inter, end: string) =>
      set.copies.find((c) => c.path.endsWith(end))!.face!;
    return {
      client,
      sample: at(inter, 'Inter[wght].ttf'),
      old: at(inter, 'Old/Inter-Regular.ttf'),
      older: at(inter, 'Older/Inter-Regular.ttf'),
      woff2: at(inter, 'Inter-Regular.woff2'),
      legacy: legacy.copies.map((c) => c.face!),
    };
  }

  it('names a language and its glyphs one copy lacks, each side with its own coverage', async () => {
    const { client, sample, older } = await interCopies();
    const c = await client.compareCopies({ a: sample, b: older });
    expect(c.a.languages).toContain('sr-Cyrl');
    expect(c.b.languages).not.toContain('sr-Cyrl');
    expect(c.b.languages).toContain('en');
    const d = c.differences!;
    expect(d.languagesLost).toEqual(['sr-Cyrl']);
    expect(d.languagesGained).toEqual([]);
    expect(d.glyphsRemoved.count).toBe(30);
    expect(d.glyphsRemoved.codePoints).toHaveLength(d.glyphsRemoved.first.length);
    expect(String.fromCodePoint(d.glyphsRemoved.codePoints![0]!)).toBe('а');
    expect(d.mayReflow).toBe(false);
    expect(c.summary[0]).toBe(
      'The first copy covers Serbian (Cyrillic) in full; the second does not.',
    );
    // The other way round it is gained.
    const back = await client.compareCopies({ a: older, b: sample });
    expect(back.differences!.languagesGained).toEqual(['sr-Cyrl']);
  });

  it('names locl on one side only and changed metrics with the reflow', async () => {
    const { client, sample, old } = await interCopies();
    const c = await client.compareCopies({ a: sample, b: old });
    const d = c.differences!;
    expect(c.a.locl).toEqual(['sr-Cyrl']);
    expect(c.b.locl).toEqual([]);
    expect(d.loclLost).toEqual(['sr-Cyrl']);
    expect(d.languagesLost).toEqual([]);
    expect(d.verticalMetricsChanged).toBe(true);
    expect(d.advanceWidthsChanged).toBe(14);
    expect(d.mayReflow).toBe(true);
    expect(d.containerOnly).toBe(false);
    expect(c.summary).toContain('Text set in one copy may reflow in the other.');
    // The static copy without ss01: the first copy alone has the axis and the feature.
    expect(d.axesRemoved).toEqual(['wght']);
    expect(d.featuresRemoved).toEqual(['ss01']);
    expect(d.axesAdded).toEqual([]);
    expect(d.featuresAdded).toEqual([]);
  });

  it('says only the container differs for the same font in another format, and not otherwise', async () => {
    const { client, sample, woff2, older } = await interCopies();
    const c = await client.compareCopies({ a: sample, b: woff2 });
    expect(c.differences!.containerOnly).toBe(true);
    expect(c.summary).toEqual([
      'Only the container differs: both files hold the same font (TTF and WOFF2).',
    ]);
    // Another format with other glyphs is not a container-only pair.
    const other = await client.compareCopies({ a: woff2, b: older });
    expect(other.differences!.containerOnly).toBe(false);
  });

  it('gives the broken side its reason and no differences', async () => {
    const { client, legacy } = await interCopies();
    const [first, second] = legacy;
    const c = await client.compareCopies({ a: first!, b: second! });
    const broken = [c.a, c.b].find((s) => s.health.kind === 'broken')!;
    expect(broken.health).toEqual({
      kind: 'broken',
      reason: "The file is truncated: the 'glyf' table ends before its declared length.",
    });
    expect(broken.languages).toEqual([]);
    expect(c.differences).toBeNull();
    expect(c.summary[0]).toMatch(
      /could not be read, so the copies are not compared: The file is truncated/,
    );
  });
});

describe('MockApi keeper policy (E56.4)', () => {
  it('stores the policy normalized and refuses a relative preferred folder', async () => {
    const client = createMockClient();
    expect((await client.getSettings()).keeperPolicy?.rules).toEqual([
      'myLanguages',
      'newestVersion',
      'format',
      'libraryFolder',
      'path',
    ]);
    const saved = await client.updateSettings({
      keeperPolicy: { rules: ['path', 'mostGlyphs', 'mostGlyphs'], preferredFolders: [' /a ', ''] },
    });
    expect(saved.keeperPolicy?.rules).toEqual(['mostGlyphs', 'path']);
    expect(saved.keeperPolicy?.preferredFolders).toEqual(['/a']);
    await expect(
      client.updateSettings({ keeperPolicy: { preferredFolders: ['fonts'] } }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
  });

  it('previews a policy over a scope and refuses an unknown source', async () => {
    const client = createMockClient();
    const preview = await client.previewKeeperPolicy({ policy: { rules: ['path'] } });
    expect(preview).toMatchObject({ keepBoth: 0, changed: 0, myLanguages: 'notSet' });
    expect(preview.withKeeper).toBeLessThanOrEqual(preview.sets);
    await expect(
      client.previewKeeperPolicy({ scope: { kind: 'source', source: 999 }, policy: {} }),
    ).rejects.toMatchObject({ code: 'notFound' });
  });
});

describe('MockApi hidden copies and answered sets (E56.8)', () => {
  it('leaves an answered set out until it is taken back', async () => {
    const client = createMockClient({ copies: library.copies });
    const before = await client.listDuplicateSets({});
    const set = before.items[0]!;
    const d = await client.decideDuplicateSet({ set: set.id, answer: 'keepBoth' });
    expect(d.hashes.length).toBe(set.copies.length);
    expect((await client.listDuplicateSets({})).total).toBe(before.total - 1);
    expect((await client.listDuplicateDecisions())[0]!.outdated).toBe(false);
    await expect(
      client.decideDuplicateSet({ set: set.id, answer: 'keepBoth' }),
    ).rejects.toMatchObject({
      code: 'notFound',
    });
    await client.takeBackDuplicateDecision({ decision: d.id });
    expect((await client.listDuplicateSets({})).total).toBe(before.total);
    expect(await client.listDuplicateDecisions()).toEqual([]);
  });

  it('hides a copy, lists it, and shows it again', async () => {
    const client = createMockClient({ copies: library.copies });
    const plan = await client.planCleanup({ removeAs: 'hide' });
    const leaving = plan.files.filter((f) => f.action === 'hide');
    expect(leaving.length).toBeGreaterThan(0);
    await client.applyCleanup({ plan });
    const hidden = await client.listHiddenCopies();
    expect(hidden.map((h) => h.path).sort()).toEqual(leaving.map((f) => f.path).sort());
    const session = (await client.listCleanupSessions())[0]!;
    expect(session.hidden).toBe(leaving.length);
    expect(session.trashed).toBe(0);
    const shown = await client.showHiddenCopies({ paths: [hidden[0]!.path] });
    expect(shown[0]!.path).toBe(hidden[0]!.path);
    expect((await client.listHiddenCopies()).length).toBe(hidden.length - 1);
    await expect(client.showHiddenCopies({ paths: [hidden[0]!.path] })).rejects.toMatchObject({
      code: 'invalidRequest',
    });
  });
});
