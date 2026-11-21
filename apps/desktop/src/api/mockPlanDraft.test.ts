// The TypeScript copies of `fm_api::cleanup_draft::plan_draft` and
// `DuplicateSummary::from_sets` (E56.15), and the mock's apply of a draft plan: a
// metadata conflict, Cancel between two files, and the draft after the apply.
import { describe, expect, it } from 'vitest';
import { createMockClient, mockDuplicateCopies } from './mock';
import { planDraft, summaryFromSets, takePlanned } from './mockCleanupDraft';
import type {
  CleanupFile,
  CleanupPlan,
  DuplicateSet,
  DuplicateSetCopy,
  Event,
  PlanCleanupRequest,
} from './types';

const copy = (path: string, face: number | null = null): DuplicateSetCopy => ({
  path,
  size: 100,
  hash: 'h',
  face,
  version: null,
  format: null,
  source: 1,
  inLibraryFolder: false,
  system: false,
  inScope: true,
  identicalWith: [],
  otherSets: [],
  facts: null,
});

const named = (id: string, paths: string[]): DuplicateSet => ({
  id,
  level: 'otherVersion',
  postscriptName: 'A',
  formatsDiffer: false,
  copies: paths.map((p) => copy(p)),
  recommendation: null,
});

/** The services' plan in small: the named keeper (else the first copy) stays, the rest go. */
const fakePlan =
  (sets: DuplicateSet[]) =>
  async (req: PlanCleanupRequest): Promise<CleanupPlan> => {
    const files: CleanupFile[] = [];
    const scope = req.scope ?? { kind: 'library' };
    if (scope.kind === 'sets')
      for (const id of scope.ids) {
        const s = sets.find((x) => x.id === id)!;
        const keep = req.keepers?.find((k) => k.set === id)?.keep ?? s.copies[0]!.path;
        for (const c of s.copies)
          files.push({
            path: c.path,
            size: 100,
            hash: 'h',
            faces: [1],
            set: id,
            level: s.level,
            action: c.path === keep ? 'stays' : 'trash',
            reason: c.path === keep ? 'keeper' : null,
            kept: keep,
            sentence: c.path === keep ? 'Stays.' : 'Goes to the trash; A.otf stays.',
          });
      }
    if (scope.kind === 'folderPair')
      files.push({
        path: `${scope.remove}/A.otf`,
        size: 100,
        hash: 'h',
        faces: [1],
        set: 'name:A',
        level: 'identical',
        action: 'trash',
        reason: null,
        kept: `${scope.keep}/A.otf`,
        sentence: '',
      });
    return {
      version: 1,
      plannedAt: 't0',
      request: req,
      sets: 1,
      files,
      conflicts: [],
      frees: 0,
      myLanguages: 'notSet',
    };
  };

const pair = (keep: string, remove: string) => ({
  keep,
  remove,
  identical: 1,
  identicalSize: 10,
  at: 't0',
});

describe('planDraft', () => {
  it('plans each copy as the user chose and answers the rest', async () => {
    const sets = [
      named('name:A', ['/a/A.otf', '/b/A.otf', '/c/A.otf', '/d/A.otf']),
      named('name:B', ['/a/B.otf', '/b/B.otf']),
    ];
    const plan = await planDraft(
      {
        decisions: [
          {
            set: 'name:A',
            level: 'otherVersion',
            postscriptName: null,
            at: 't0',
            decision: {
              kind: 'copies',
              copies: [
                { path: '/a/A.otf', choice: 'trash' },
                { path: '/b/A.otf', choice: 'keep' },
                { path: '/c/A.otf', choice: 'hide' },
              ],
            },
          },
          {
            set: 'name:B',
            level: 'otherVersion',
            postscriptName: null,
            at: 't0',
            decision: { kind: 'keepAll' },
          },
          {
            set: 'name:Gone',
            level: 'otherVersion',
            postscriptName: null,
            at: 't0',
            decision: { kind: 'notDuplicates' },
          },
        ],
      },
      sets,
      {},
      fakePlan(sets),
    );
    expect(plan.request.keepers).toEqual([{ set: 'name:A', keep: '/b/A.otf' }]);
    const of = (p: string) => {
      const f = plan.files.find((x) => x.path === p)!;
      return [f.action, f.reason];
    };
    expect(of('/a/A.otf')).toEqual(['trash', null]);
    expect(of('/b/A.otf')).toEqual(['stays', 'keeper']);
    expect(of('/c/A.otf')).toEqual(['hide', null]);
    expect(of('/d/A.otf')).toEqual(['stays', 'keeper']);
    expect(plan.files.find((f) => f.path === '/c/A.otf')!.sentence).toMatch(/^Is hidden/);
    expect(plan.frees).toBe(100);
    expect(plan.answers).toEqual([{ set: 'name:B', postscriptName: 'A', answer: 'keepBoth' }]);
    expect(plan.draft?.sets).toEqual(['name:A', 'name:B']);
  });

  it('never lets one part remove the copy another part keeps', async () => {
    const plan = await planDraft(
      { pairs: [pair('/b', '/a'), pair('/a', '/b')] },
      [],
      {},
      fakePlan([]),
    );
    expect(plan.files.filter((f) => f.action !== 'stays')).toEqual([]);
    expect(plan.files.every((f) => f.reason === 'keptByOtherSet')).toBe(true);
    expect(plan.draft?.pairs).toHaveLength(2);
  });

  it('lets an answered set lose no copy to the identical sets or a folder pair', async () => {
    const identical = (id: string, paths: string[]): DuplicateSet => ({
      ...named(id, paths),
      level: 'identical',
    });
    const sets = [
      identical('identical:h', ['/a/S.otf', '/b/S.otf']),
      identical('identical:k', ['/a/K.otf', '/b/K.otf']),
      named('name:A', ['/a/A.otf', '/z/A.otf']),
    ];
    const answer = (set: string, kind: 'keepAll' | 'notDuplicates') => ({
      set,
      level: 'identical' as const,
      postscriptName: null,
      at: 't0',
      decision: { kind },
    });
    const plan = await planDraft(
      {
        identical: true,
        decisions: [answer('identical:h', 'keepAll'), answer('name:A', 'notDuplicates')],
        pairs: [pair('/z', '/a')],
      },
      sets,
      {},
      fakePlan(sets),
    );
    expect(plan.files.filter((f) => f.action !== 'stays').map((f) => f.path)).toEqual(['/b/K.otf']);
    expect(plan.files.some((f) => f.set === 'identical:h' || f.set === 'name:A')).toBe(false);
    expect(plan.request.scope).toEqual({ kind: 'sets', ids: ['identical:k'] });
    expect(plan.answers).toHaveLength(2);
  });

  it('takes out of the draft only what the plan held', () => {
    const draft = takePlanned(
      {
        identical: true,
        decisions: [
          {
            set: 'name:A',
            level: 'otherVersion',
            postscriptName: null,
            at: 't0',
            decision: { kind: 'keepAll' },
          },
          {
            set: 'name:B',
            level: 'otherVersion',
            postscriptName: null,
            at: 't0',
            decision: { kind: 'keepAll' },
          },
        ],
        pairs: [pair('/b', '/a')],
      },
      { identical: true, sets: ['name:A'], pairs: [] },
    );
    expect(draft.identical).toBe(false);
    expect(draft.decisions?.map((d) => d.set)).toEqual(['name:B']);
    expect(draft.pairs).toHaveLength(1);
  });
});

describe('summaryFromSets', () => {
  it('counts the fonts, the copies in the scope and the families', () => {
    const a = named('name:A', ['/a/A.otf', '/b/A.otf', '/c/A.otf']);
    a.copies.forEach((c, i) => (c.face = i + 1));
    const b = named('name:B', ['/a/B.otf', '/b/B.otf']);
    b.copies[0]!.face = 10;
    const families = new Map([
      [1, 1],
      [2, 1],
      [10, 1],
    ]);
    const s = summaryFromSets(
      [a, b],
      (p) => p.startsWith('/a/') || p.startsWith('/b/'),
      (f) => families.get(f),
    );
    expect(s).toEqual({
      fonts: 2,
      copies: 4,
      families: [{ family: 1, copies: 3, sets: ['name:A', 'name:B'] }],
    });
  });
});

describe('the mock applies a plan of the draft', () => {
  const inter = '/Users/demo/Fonts/Old/Inter-Regular.ttf';
  const client = (cleanupStepMs = 0) =>
    createMockClient({
      copies: mockDuplicateCopies.map((c) => (c.path === inter ? { ...c, rating: 3 } : c)),
      ratings: { 1: 5 },
      cleanupStepMs,
    });

  it('lists the rating conflict, takes the chosen value and empties the draft', async () => {
    const c = client();
    const sets = await c.listDuplicateSets({});
    const interSet = sets.items.find((s) => s.copies.some((x) => x.path === inter))!;
    const keep = interSet.copies.find((x) => x.face === 1)!.path;
    await c.updateCleanupDraft({
      kind: 'decide',
      set: interSet.id,
      decision: {
        kind: 'copies',
        copies: interSet.copies.map((x) => ({
          path: x.path,
          choice: x.path === keep ? 'keep' : 'trash',
        })),
      },
    });
    const plan = await c.planCleanupDraft({});
    expect(plan.conflicts).toHaveLength(1);
    const conflict = plan.conflicts[0]!;
    expect(conflict.field).toBe('rating');
    expect(conflict.default).toEqual({ kind: 'rating', stars: 5 });
    expect(conflict.other).toEqual({ kind: 'rating', stars: 3 });
    await expect(
      c.applyCleanup({ plan, choices: [{ keptFace: 999, field: 'rating', take: 1 }] }),
    ).rejects.toMatchObject({ code: 'invalidRequest' });
    await c.applyCleanup({
      plan,
      choices: [{ keptFace: conflict.keptFace, field: 'rating', take: conflict.otherFace }],
    });
    expect((await c.getFace(1)).rating).toBe(3);
    expect((await c.getCleanupDraft()).decisions).toEqual([]);
    const [session] = await c.listCleanupSessions();
    expect(session!.trashed).toBe(2);
  });

  it('runs file by file and Cancel stops it between two files', async () => {
    const c = client(5);
    await c.updateCleanupDraft({ kind: 'identical', include: true });
    const sets = await c.listDuplicateSets({});
    for (const s of sets.items.filter((s) => s.level !== 'identical'))
      await c.updateCleanupDraft({
        kind: 'decide',
        set: s.id,
        decision: {
          kind: 'copies',
          copies: s.copies.map((x, i) => ({ path: x.path, choice: i === 0 ? 'keep' : 'trash' })),
        },
      });
    const plan = await c.planCleanupDraft({});
    const leaving = plan.files.filter((f) => f.action !== 'stays').length;
    expect(leaving).toBeGreaterThan(2);
    const events: Event[] = [];
    const ended = new Promise<void>((resolve) => {
      void c.onEvent((e) => {
        if (e.type !== 'cleanupProgress') return;
        events.push(e);
        if (e.done === 1 && !e.finished) void c.cancelCleanup(e.job);
        if (e.finished) resolve();
      });
    });
    const job = await c.applyCleanup({ plan });
    expect(events).toEqual([]);
    await ended;
    const last = events.at(-1)!;
    expect(last).toMatchObject({ type: 'cleanupProgress', job, finished: true, cancelled: true });
    expect(last.type === 'cleanupProgress' && last.done).toBe(1);
    const [session] = await c.listCleanupSessions();
    expect(session).toMatchObject({ cancelled: true, running: false });
    expect(session!.files).toHaveLength(1);
    await expect(c.cancelCleanup(job)).rejects.toMatchObject({ code: 'notFound' });
  });
});
