import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useTextLayoutsStore } from './textLayouts';

beforeEach(() => setActivePinia(createPinia()));
afterEach(() => setClient(undefined));

describe('text layouts store', () => {
  it('loads the presets once and finds one by id', async () => {
    const client = createMockClient();
    const spy = vi.spyOn(client, 'layoutPresets');
    setClient(client);
    const store = useTextLayoutsStore();
    expect(store.byPreset('quote')).toBeNull();
    await Promise.all([store.load(), store.load()]);
    await store.load();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(store.byPreset('quote')?.name).toBe('Quote');
  });

  it('keeps the error and tries again on the next load', async () => {
    const client = createMockClient();
    const spy = vi
      .spyOn(client, 'layoutPresets')
      .mockRejectedValueOnce({ code: 'internal', message: 'boom' });
    setClient(client);
    const store = useTextLayoutsStore();
    await store.load();
    expect(store.error?.message).toBe('boom');
    await store.load();
    expect(store.error).toBeNull();
    expect(store.presets).toHaveLength(6);
    expect(spy).toHaveBeenCalledTimes(2);
  });
});
