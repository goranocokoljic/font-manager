import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockDuplicateCopies } from '../api/mock';
import type { Event } from '../api/types';
import { useDuplicateSummaryStore } from './duplicateSummary';
import { useSourcesStore } from './sources';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;
let send: (e: Event) => void;
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(async () => {
  setActivePinia(createPinia());
  const mock = createMockClient({ copies: mockDuplicateCopies });
  // The store's events come from the test, as the app's come from the core.
  client = {
    ...mock,
    onEvent: async (handler) => {
      send = handler;
      return () => undefined;
    },
  };
  setClient(client);
  await useSourcesStore().load();
});

afterEach(() => setClient(undefined));

const scan = async (job: number, source: number, added: number) => {
  send({ type: 'scanStarted', job, source });
  send({ type: 'scanFinished', job, added, changed: 0, removed: 0, problems: 0 });
  await flush();
  await flush();
};

describe('the duplicate summary store (E56.15)', () => {
  it('counts the fonts with copies and the copies of each family', async () => {
    const store = useDuplicateSummaryStore();
    await store.start();
    await flush();
    expect(store.fonts).toBe(3);
    const inter = store.summary?.families.find((f) => f.copies === 3);
    expect(inter).toBeDefined();
    expect(store.of(inter!.family)?.copies).toBe(3);
    expect(store.of(null)).toBeUndefined();
  });

  it('shows the notice once per scan that added fonts with copies', async () => {
    const store = useDuplicateSummaryStore();
    await store.start();
    const source = useSourcesStore().sources[0]!;
    await scan(10, source.id, 4);
    expect(store.notice).toMatchObject({ job: 10, added: 4 });
    expect(store.notice!.copies).toBeGreaterThan(0);
    expect(store.notice!.folder).toBe(source.path.split(/[\\/]/).filter(Boolean).at(-1));

    // Dismissed, the same scan never shows it again.
    store.dismissNotice();
    send({ type: 'scanFinished', job: 10, added: 4, changed: 0, removed: 0, problems: 0 });
    await flush();
    expect(store.notice).toBeNull();
  });

  it('shows no notice for a scan that added nothing or scanned every source', async () => {
    const store = useDuplicateSummaryStore();
    await store.start();
    const source = useSourcesStore().sources[0]!;
    await scan(11, source.id, 0);
    expect(store.notice).toBeNull();
    send({ type: 'scanStarted', job: 12, source: null });
    send({ type: 'scanFinished', job: 12, added: 5, changed: 0, removed: 0, problems: 0 });
    await flush();
    expect(store.notice).toBeNull();
  });

  it('reads the counts again on a library change, not by polling', async () => {
    const store = useDuplicateSummaryStore();
    const spy = vi.spyOn(client, 'duplicateSummary');
    await store.start();
    await flush();
    const before = spy.mock.calls.length;
    send({ type: 'libraryChanged', source: null, added: [], removed: [], updated: [] });
    await flush();
    expect(spy.mock.calls.length).toBe(before + 1);
  });
});
