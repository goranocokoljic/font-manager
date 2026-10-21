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

  it('loads the core bounds with the presets', async () => {
    setClient(createMockClient());
    const store = useTextLayoutsStore();
    expect(store.bounds).toBeNull();
    await store.load();
    expect(store.bounds?.sizePt).toMatchObject({ min: 8, max: 600 });
    expect(store.bounds?.lineHeight).toMatchObject({ min: 0.1, max: 3 });
  });

  it('an edited block shows at once, comes back clamped by the core and survives a preset switch', async () => {
    const client = createMockClient();
    const clamp = vi.spyOn(client, 'clampLayout');
    setClient(client);
    const store = useTextLayoutsStore();
    await store.load();
    const done = store.editBlock('quote', 0, { sizePt: 9000, text: 'Mine' });
    expect(store.byPreset('quote')?.blocks[0]).toMatchObject({ sizePt: 9000, text: 'Mine' });
    await done;
    expect(clamp).toHaveBeenCalledTimes(1);
    expect(store.byPreset('quote')?.blocks[0]).toMatchObject({ sizePt: 600, text: 'Mine' });
    // Other blocks and presets are untouched.
    expect(store.byPreset('quote')?.blocks[1]).toEqual(store.presets[3]!.blocks[1]);
    expect(store.byPreset('book')).toEqual(store.presets[0]);
    expect(store.presets[3]!.blocks[0]!.text).not.toBe('Mine');
  });

  it('a late core answer does not undo a newer edit', async () => {
    const client = createMockClient();
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const real = client.clampLayout.bind(client);
    vi.spyOn(client, 'clampLayout').mockImplementationOnce(async (l) => {
      await gate;
      return real(l);
    });
    setClient(client);
    const store = useTextLayoutsStore();
    await store.load();
    const first = store.editBlock('book', 0, { sizePt: 30 });
    await store.editBlock('book', 0, { sizePt: 40 });
    release();
    await first;
    expect(store.byPreset('book')?.blocks[0]?.sizePt).toBe(40);
  });

  it('an edit of a block that is not there is ignored', async () => {
    setClient(createMockClient());
    const store = useTextLayoutsStore();
    await store.editBlock('book', 0, { sizePt: 30 });
    await store.load();
    await store.editBlock('book', 99, { sizePt: 30 });
    expect(store.byPreset('book')).toEqual(store.presets[0]);
  });
});
