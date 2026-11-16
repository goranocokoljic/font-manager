import { flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockWatcher } from '../api/mock';
import { useSearchStore } from './search';
import { useToastStore } from './toast';
import { useWatchNoticeStore } from './watchNotice';

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(async () => {
  await useWatchNoticeStore().stop();
  useToastStore().clear();
  setClient(undefined);
});

describe('the folder watcher notice (E51.2)', () => {
  it('says how many fonts the watcher added from which folder, with Show', async () => {
    const open = vi.fn();
    await useWatchNoticeStore().start(open);
    mockWatcher().add(3);
    await vi.waitFor(() => expect(useToastStore().toast).not.toBeNull());
    const toast = useToastStore().toast!;
    expect(toast.message).toBe('3 fonts added from Fonts');
    expect(toast.action?.label).toBe('Show');
  });

  it('Show filters the wall to the added fonts and opens it', async () => {
    const open = vi.fn();
    await useWatchNoticeStore().start(open);
    const search = useSearchStore();
    search.setFilters({ favorite: true });
    const added = mockWatcher().add(2);
    await vi.waitFor(() => expect(useToastStore().toast).not.toBeNull());
    useToastStore().act();
    await flushPromises();
    expect(open).toHaveBeenCalledTimes(1);
    expect(search.only?.faces).toEqual(added);
    expect(search.only?.label).toBe('New in Fonts · 2');
    // No other filter hides them.
    expect(search.filters).toEqual({});
    await vi.waitFor(() =>
      expect(search.hits?.map((h) => h.family.name)).toEqual(['Arrival 1', 'Arrival 2']),
    );
    search.clearOnly();
    await vi.waitFor(() => expect(search.hits).toBeNull());
  });

  it('shows nothing for a removal, a scan or a user edit', async () => {
    await useWatchNoticeStore().start(() => undefined);
    const [face] = mockWatcher().add(1);
    await vi.waitFor(() => expect(useToastStore().toast).not.toBeNull());
    useToastStore().clear();
    mockWatcher().remove([face!]);
    await client.scan({ source: 1 });
    await client.updateFamilyNotes({ family: 1, notes: 'x' }).catch(() => undefined);
    await new Promise((r) => setTimeout(r, 20));
    await flushPromises();
    expect(useToastStore().toasts).toEqual([]);
  });
});
