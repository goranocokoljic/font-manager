import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useActivationStore } from './activation';

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
  await useActivationStore().stop();
  setClient(undefined);
});

describe('activation store', () => {
  it('counts the active faces and follows activation events', async () => {
    const store = useActivationStore();
    await store.load();
    expect(store.count).toBe(0);

    const listActive = vi.spyOn(client, 'listActive');
    await client.activate({ target: { kind: 'faces', faces: [1, 6] }, onConflict: 'replace' });
    await vi.waitFor(() => expect(store.count).toBe(2));
    expect(listActive).toHaveBeenCalledTimes(1);

    await client.deactivate({ target: { kind: 'faces', faces: [1] } });
    await vi.waitFor(() => expect(store.count).toBe(1));
  });

  it('stops following events after stop', async () => {
    const store = useActivationStore();
    await store.load();
    await store.stop();
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await Promise.resolve();
    expect(store.count).toBe(0);
  });

  it('keeps the count when the API fails', async () => {
    const store = useActivationStore();
    await store.load();
    await client.activate({ target: { kind: 'faces', faces: [1] } });
    await vi.waitFor(() => expect(store.count).toBe(1));
    vi.spyOn(client, 'listActive').mockRejectedValue({ code: 'io', message: 'x', detail: null });
    await store.refresh();
    expect(store.count).toBe(1);
  });
});
