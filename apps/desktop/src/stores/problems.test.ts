import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockDuplicates, mockExtraProblems } from '../api/mock';
import type { ApiError, Problem, ProblemKind } from '../api/types';
import {
  PROBLEM_GROUPS,
  UNDO_WINDOW_MS,
  groupOf,
  problemGroup,
  useProblemsStore,
} from './problems';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ extraProblems: mockExtraProblems, duplicates: mockDuplicates });
  setClient(client);
});

afterEach(async () => {
  vi.useRealTimers();
  await useProblemsStore().stop();
  setClient(undefined);
});

const ALL_KINDS: ProblemKind[] = [
  'duplicateFile',
  'duplicateFace',
  'broken',
  'noUnicodeCmap',
  'systemConflict',
  'activeConflict',
  'unsupported',
  'sidecarConflict',
  'missingSource',
  'indexRebuilt',
];

describe('problem groups', () => {
  it('put every kind in exactly one group', () => {
    for (const kind of ALL_KINDS) {
      expect(PROBLEM_GROUPS.filter((g) => g.kinds.includes(kind))).toHaveLength(1);
    }
    expect(PROBLEM_GROUPS.flatMap((g) => g.kinds).sort()).toEqual([...ALL_KINDS].sort());
    expect(groupOf('unsupported')?.label).toBe('Legacy formats');
    expect(problemGroup('nope')).toBeUndefined();
  });
});

describe('problems store', () => {
  it('lists every kind of entry and counts each group', async () => {
    const store = useProblemsStore();
    await store.load();
    expect(store.loaded).toBe(true);
    // Face 6 clashes with a system font, face 7 is Type 1; the extras add the rest.
    expect(store.problems.map((p) => p.kind)).toEqual([
      'systemConflict',
      'unsupported',
      'broken',
      'duplicateFile',
      'sidecarConflict',
    ]);
    expect(store.total).toBe(5);
    expect(store.counts).toEqual({
      duplicates: 1,
      broken: 1,
      legacy: 1,
      sidecar: 1,
      activation: 1,
      library: 0,
    });
  });

  it('filters by group, and lists everything without one', async () => {
    const store = useProblemsStore();
    await store.load();
    expect(store.inGroup('legacy').map((p) => p.path)).toEqual([
      '/Users/demo/Fonts/legacy/LEGGR___.PFB',
    ]);
    expect(store.inGroup('duplicates').map((p) => p.other)).toEqual([
      '/Users/demo/Fonts/Quarto-Regular.otf',
    ]);
    expect(store.inGroup('library')).toEqual([]);
    expect(store.inGroup(undefined)).toHaveLength(5);
  });

  it('reads the list again on library, scan and activation events', async () => {
    const extra: Problem[] = [];
    let reads = 0;
    setClient({
      ...client,
      listProblems: async () => {
        reads += 1;
        return [...(await client.listProblems()), ...extra];
      },
    });
    const store = useProblemsStore();
    await store.load();
    expect(reads).toBe(1);
    extra.push({
      kind: 'missingSource',
      face: null,
      path: '/Volumes/Gone',
      other: null,
      message: 'The source folder cannot be found.',
    });
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await vi.waitFor(() => expect(store.counts.library).toBe(1));
    await client.removeSource(1);
    await vi.waitFor(() => expect(reads).toBe(3));
  });

  it('shows the API error and keeps the last list', async () => {
    const store = useProblemsStore();
    await store.load();
    const failure: ApiError = { code: 'internal', message: 'The index is busy.', detail: null };
    setClient({ ...client, listProblems: () => Promise.reject(failure) });
    await store.refresh();
    expect(store.error).toEqual(failure);
    expect(store.total).toBe(5);
  });
});

describe('keep this one', () => {
  const KEEP = '/Users/demo/Fonts/Quarto-Regular.otf';
  const COPY = '/Users/demo/Downloads/Quarto-Regular.otf';

  async function planned() {
    const store = useProblemsStore();
    await store.load();
    expect(store.duplicates).toHaveLength(1);
    await store.planKeep(store.duplicates[0]!, KEEP);
    return store;
  }

  it('shows the dry run of what will move and moves nothing', async () => {
    const store = await planned();
    expect(store.plan?.dryRun).toBe(true);
    expect(store.plan?.kept).toBe(KEEP);
    expect(store.plan?.moves).toEqual([{ path: COPY, status: 'planned', message: null }]);
    expect(await client.listDuplicates()).toHaveLength(1);
    expect(store.history).toEqual([]);
    store.cancelPlan();
    expect(store.plan).toBeNull();
    expect(await client.listDuplicates()).toHaveLength(1);
  });

  it('moves the other copy on confirm and offers undo in a toast for 10 s', async () => {
    const store = await planned();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    await store.confirmPlan();
    expect(store.plan).toBeNull();
    expect(store.toast?.moves).toEqual([{ path: COPY, status: 'trashed', message: null }]);
    expect(store.duplicates).toEqual([]);
    expect(store.counts.duplicates).toBe(0);
    expect(store.history).toHaveLength(1);

    vi.advanceTimersByTime(UNDO_WINDOW_MS - 1);
    expect(store.toast).not.toBeNull();
    vi.advanceTimersByTime(1);
    expect(store.toast).toBeNull();
  });

  it('undoes from the toast', async () => {
    const store = await planned();
    await store.confirmPlan();
    const id = store.toast!.id!;
    const r = await store.undo(id);
    expect(r?.undone).toBe(true);
    expect(store.toast).toBeNull();
    expect(store.duplicates).toHaveLength(1);
    expect(store.counts.duplicates).toBe(1);
    expect(store.history[0]?.undone).toBe(true);
  });

  it('undoes from the history after the toast is gone', async () => {
    const store = await planned();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    await store.confirmPlan();
    vi.advanceTimersByTime(UNDO_WINDOW_MS);
    expect(store.toast).toBeNull();
    await store.undo(store.history[0]!.id!);
    expect(store.duplicates).toHaveLength(1);
    expect(store.history[0]?.moves[0]?.status).toBe('restored');
  });

  it('shows why a copy could not come back', async () => {
    const store = await planned();
    await store.confirmPlan();
    setClient({
      ...client,
      undoResolution: async (id) => ({
        ...(await client.undoResolution(id)),
        undone: false,
        moves: [
          {
            path: COPY,
            status: 'failed',
            message: 'Another file is at this path now, so the copy was left in the trash.',
          },
        ],
      }),
    });
    await store.undo(store.history[0]!.id!);
    expect(store.actionError?.message).toMatch(/left in the trash/);
  });

  it('shows the API error of a refused keep', async () => {
    const store = useProblemsStore();
    await store.load();
    await store.planKeep(
      { kind: 'exact', postscriptName: null, copies: [] },
      '/Users/demo/notes.txt',
    );
    expect(store.plan).toBeNull();
    expect(store.actionError?.code).toBe('invalidRequest');
  });
});
