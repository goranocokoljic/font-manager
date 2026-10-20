import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useSetsStore } from './sets';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({
    sets: [
      { id: 1, name: 'Client', faces: [1] },
      { id: 2, name: 'Book', faces: [6] },
    ],
  });
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('sets store', () => {
  it('loads the sets with no current set', async () => {
    const store = useSetsStore();
    await store.load();
    expect(store.sets.map((s) => s.name)).toEqual(['Client', 'Book']);
    expect(store.current).toBeNull();
  });

  it('picks a current set, switches it, and clears it on a second pick', async () => {
    const store = useSetsStore();
    await store.load();
    store.pick(1);
    expect(store.current?.name).toBe('Client');
    store.pick(2);
    expect(store.current?.name).toBe('Book');
    store.pick(2);
    expect(store.current).toBeNull();
  });

  it('keeps no current set when the picked set is gone after a reload', async () => {
    const store = useSetsStore();
    await store.load();
    store.pick(2);
    vi.spyOn(client, 'listSets').mockResolvedValue([{ id: 1, name: 'Client', faces: [1] }]);
    await store.load();
    expect(store.current).toBeNull();
  });

  it('reports a failed read', async () => {
    vi.spyOn(client, 'listSets').mockRejectedValue({
      code: 'io',
      message: 'The index is locked.',
      detail: null,
    });
    const store = useSetsStore();
    await store.load();
    expect(store.error?.message).toBe('The index is locked.');
    expect(store.sets).toEqual([]);
  });
});
