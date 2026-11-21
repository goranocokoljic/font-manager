// The TypeScript copies of `fm_api::cleanup_draft` and `DuplicateSet::frees` (E56.11):
// the same cases as the Rust unit tests, so the two mocks count and decide alike.
import { describe, expect, it } from 'vitest';
import { applyDraftUpdate, overviewFromPlan } from './mockCleanupDraft';
import { setFrees } from './mockDuplicates';
import type {
  CleanupAction,
  CleanupFile,
  CleanupPlan,
  DuplicateLevel,
  DuplicateSet,
  DuplicateSetCopy,
  StayReason,
} from './types';

const copy = (path: string, system = false, size = 10): DuplicateSetCopy => ({
  path,
  size,
  hash: 'h',
  face: null,
  version: null,
  format: null,
  source: 1,
  inLibraryFolder: false,
  system,
  inScope: true,
  identicalWith: [],
  otherSets: [],
  facts: null,
});

const set = (): DuplicateSet => ({
  id: 'name:A',
  level: 'otherVersion',
  postscriptName: 'A',
  formatsDiffer: false,
  copies: [copy('/a/A.otf'), copy('/b/A.otf'), copy('/sys/A.otf', true)],
  recommendation: null,
});

const file = (
  path: string,
  set: string,
  level: DuplicateLevel,
  action: CleanupAction,
  reason: StayReason | null,
): CleanupFile => ({
  path,
  size: 100,
  hash: 'h',
  faces: [1],
  set,
  level,
  action,
  reason,
  kept: null,
  alsoKept: [],
  sentence: '',
  activation: null,
});

describe('the mock cleanup draft', () => {
  it('counts what the plan removes per level, as DuplicatesOverview::from_plan', () => {
    const plan: CleanupPlan = {
      version: 1,
      plannedAt: 't',
      request: {},
      sets: 4,
      files: [
        file('/a/x.otf', 'identical:1', 'identical', 'stays', 'keeper'),
        file('/b/x.otf', 'identical:1', 'identical', 'trash', null),
        file('/c/x.otf', 'identical:1', 'identical', 'trash', null),
        file('/a/y.otf', 'name:Y', 'otherVersion', 'stays', 'keeper'),
        file('/b/y.otf', 'name:Y', 'otherVersion', 'hide', null),
        file('/a/z.otf', 'name:Z', 'otherVersion', 'stays', 'keepBoth'),
        file('/b/z.otf', 'name:Z', 'otherVersion', 'stays', 'keepBoth'),
        file('/a/w.otf', 'name:W', 'sameVersion', 'stays', 'active'),
      ],
      conflicts: [],
      frees: 200,
      myLanguages: 'notSet',
    };
    expect(overviewFromPlan(plan)).toEqual({
      fonts: 4,
      extraFiles: 3,
      folders: 2,
      frees: 200,
      identical: { sets: 1, files: 2, frees: 200, folders: 2 },
      sameVersion: { sets: 1, files: 0, frees: 0, folders: 0 },
      otherVersion: { sets: 2, files: 1, frees: 0, folders: 1 },
      keepBoth: 1,
      myLanguages: 'notSet',
    });
  });

  it('refuses a decision that keeps no copy or moves a system copy', () => {
    const empty = { identical: false, decisions: [] };
    const decide = (choices: [string, 'keep' | 'trash' | 'hide'][], s = set()) =>
      applyDraftUpdate(
        empty,
        {
          kind: 'decide',
          set: s.id,
          decision: { kind: 'copies', copies: choices.map(([path, choice]) => ({ path, choice })) },
        },
        't',
        () => s,
      );
    expect(decide([['/a/A.otf', 'trash']]).decisions).toHaveLength(1);
    expect(() => decide([['/sys/A.otf', 'trash']])).toThrow();
    expect(() => decide([['/c/A.otf', 'keep']])).toThrow();
    const two = { ...set(), copies: set().copies.slice(0, 2) };
    expect(() =>
      decide(
        [
          ['/a/A.otf', 'trash'],
          ['/b/A.otf', 'trash'],
        ],
        two,
      ),
    ).toThrow();
  });

  it('replaces a decision, takes it back, and refuses a set that is gone', () => {
    let draft = applyDraftUpdate(
      { identical: false, decisions: [] },
      { kind: 'decide', set: 'name:A', decision: { kind: 'keepAll' } },
      't1',
      () => set(),
    );
    draft = applyDraftUpdate(
      draft,
      { kind: 'decide', set: 'name:A', decision: { kind: 'notDuplicates' } },
      't2',
      () => set(),
    );
    expect(draft.decisions).toEqual([
      {
        set: 'name:A',
        level: 'otherVersion',
        postscriptName: 'A',
        decision: { kind: 'notDuplicates' },
        at: 't2',
      },
    ]);
    expect(() =>
      applyDraftUpdate(
        draft,
        { kind: 'decide', set: 'name:B', decision: { kind: 'keepAll' } },
        't',
        () => undefined,
      ),
    ).toThrow(expect.objectContaining({ code: 'notFound' }));
    draft = applyDraftUpdate(draft, { kind: 'identical', include: true }, 't', () => undefined);
    expect(draft.identical).toBe(true);
    draft = applyDraftUpdate(draft, { kind: 'takeBack', set: 'name:A' }, 't', () => undefined);
    expect(draft).toEqual({ identical: true, decisions: [], pairs: [] });
    expect(applyDraftUpdate(draft, { kind: 'clear' }, 't', () => undefined)).toEqual({
      identical: false,
      decisions: [],
      pairs: [],
    });
  });

  it('frees every copy outside the system folders but the largest without a recommendation', () => {
    const s = set();
    s.copies = [copy('/a', false, 30), copy('/b', false, 50), copy('/c', true, 99)];
    expect(setFrees(s)).toBe(30);
    s.recommendation = {
      outcome: 'keeper',
      keeper: '/a',
      reason: null,
      keep: [],
      remove: [
        {
          path: '/b',
          reason: { rule: 'path', kept: [], other: [], otherPath: '/a', sentence: '' },
        },
      ],
      myLanguages: 'notSet',
    };
    expect(setFrees(s)).toBe(50);
  });
});
