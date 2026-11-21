import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockDuplicateCopies } from '../api/mock';
import { useDuplicatesStore } from './duplicates';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient({ copies: mockDuplicateCopies });
  setClient(client);
});

afterEach(() => setClient(undefined));

describe('the duplicates store', () => {
  it('loads the overview and the queue of the levels to decide, without identical sets', async () => {
    const store = useDuplicatesStore();
    await store.load();
    expect(store.overview?.fonts).toBe(3);
    expect(store.overview?.identical.sets).toBe(1);
    await store.reload(0);
    expect(store.total).toBe(2);
    expect(store.current?.id).toBe('name:Inter-Regular');
  });

  it('starts each copy at what the API plans for it and marks the API keeper', async () => {
    const store = useDuplicatesStore();
    await store.reload(0);
    expect(store.keeper).toBe('/Users/demo/Fonts/Inter/Inter[wght].ttf');
    expect(store.reason).toBe('Stays: it is the copy that is kept.');
    expect(store.choices).toEqual({
      '/Users/demo/Fonts/Inter/Inter[wght].ttf': 'keep',
      '/Users/demo/Fonts/Old/Inter-Regular.ttf': 'trash',
      '/Users/demo/Fonts/Older/Inter-Regular.ttf': 'trash',
    });
  });

  it('sends a decision to the API and shows the set that took its place', async () => {
    const store = useDuplicatesStore();
    await store.reload(0);
    store.keepOnly('/Users/demo/Fonts/Older/Inter-Regular.ttf');
    store.choose('/Users/demo/Fonts/Inter/Inter[wght].ttf', 'hide');
    expect(await store.decide('copies')).toBe(true);
    expect(store.current?.id).toBe('name:LegacyGrotesk');
    expect(store.total).toBe(1);
    const stored = await client.getCleanupDraft();
    expect(stored.decisions?.[0]).toMatchObject({
      set: 'name:Inter-Regular',
      decision: {
        kind: 'copies',
        copies: [
          { path: '/Users/demo/Fonts/Inter/Inter[wght].ttf', choice: 'hide' },
          { path: '/Users/demo/Fonts/Old/Inter-Regular.ttf', choice: 'trash' },
          { path: '/Users/demo/Fonts/Older/Inter-Regular.ttf', choice: 'keep' },
        ],
      },
    });
    expect(await store.decide('notDuplicates')).toBe(true);
    expect(store.current).toBeNull();
    expect(store.total).toBe(0);
    await store.takeBack('name:LegacyGrotesk');
    expect(store.current?.id).toBe('name:LegacyGrotesk');
  });

  it('asks the API for the filtered queue', async () => {
    const store = useDuplicatesStore();
    const list = vi.spyOn(client, 'listDuplicateSets');
    await store.reload(0);
    await store.setFilter({ level: 'sameVersion', format: 'type1' });
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({
        filter: {
          levels: ['sameVersion'],
          format: 'type1',
          recommendation: null,
          undecided: true,
        },
        sort: 'name',
      }),
    );
    expect(store.total).toBe(1);
    await store.setFilter({ level: null, format: null, recommendation: 'hasRecommendation' });
    expect(store.total).toBe(0);
    expect(store.current).toBeNull();
  });
});
