import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockExtraProblems } from '../api/mock';
import type { ApiError, Problem, ProblemKind } from '../api/types';
import { PROBLEM_GROUPS, groupOf, problemGroup, useProblemsStore } from './problems';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ extraProblems: mockExtraProblems });
  setClient(client);
});

afterEach(async () => {
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
