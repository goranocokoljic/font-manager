import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { ApiError } from '../api/types';
import { useConflictsStore } from './conflicts';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(async () => {
  await useConflictsStore().stop();
  setClient(undefined);
});

describe('conflicts store', () => {
  it('lists every clash with its winner and follows activation events', async () => {
    const store = useConflictsStore();
    await store.load();
    expect(store.loaded).toBe(true);
    expect(store.clashes).toEqual([]);

    // Face 6 shares its PostScript name with a system font; face 1 does not.
    await client.activate({ target: { kind: 'faces', faces: [1, 6] }, onConflict: 'replace' });
    await vi.waitFor(() => expect(store.clashes).toHaveLength(1));
    expect(store.clashes[0]).toEqual({
      face: 6,
      postscriptName: 'JetBrainsMono-Regular',
      with: { kind: 'system', path: '/System/Library/Fonts/JetBrainsMono-Regular.ttf' },
      winner: 'face',
    });
    expect(store.nameOf(6)).toBe('JetBrains Mono Regular');

    await client.deactivate({ target: { kind: 'faces', faces: [6] } });
    await vi.waitFor(() => expect(store.clashes).toEqual([]));
  });

  it('shows the API error and keeps the last list', async () => {
    const failure: ApiError = { code: 'unsupported', message: 'No font service.', detail: null };
    setClient({ ...client, listConflicts: () => Promise.reject(failure) });
    const store = useConflictsStore();
    await store.refresh();
    expect(store.error).toEqual(failure);
    expect(store.clashes).toEqual([]);
  });

  it('names a face it has not read by its id', () => {
    expect(useConflictsStore().nameOf(42)).toBe('Face 42');
  });
});
