import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, tauriClient, type FmClient } from '../api/client';
import { createMockClient, defaultSettings, mockFolders, mockSources } from '../api/mock';
import { useSettingsStore } from './settings';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => {
  setClient(undefined);
  invoke.mockReset();
});

describe('settings client', () => {
  it('calls the settings and source Tauri commands with their arguments', async () => {
    invoke.mockResolvedValue(undefined);
    await tauriClient.getSettings();
    expect(invoke).toHaveBeenLastCalledWith('get_settings');
    await tauriClient.updateSettings({ theme: 'dark' });
    expect(invoke).toHaveBeenLastCalledWith('update_settings', { req: { theme: 'dark' } });
    await tauriClient.getAppFolders();
    expect(invoke).toHaveBeenLastCalledWith('get_app_folders');
    await tauriClient.listSources();
    expect(invoke).toHaveBeenLastCalledWith('list_sources');
    await tauriClient.removeSource(3);
    expect(invoke).toHaveBeenLastCalledWith('remove_source', { source: 3 });
    await tauriClient.setSourcePaused({ source: 1, paused: true });
    expect(invoke).toHaveBeenLastCalledWith('set_source_paused', {
      req: { source: 1, paused: true },
    });
  });
});

describe('settings store', () => {
  it('loads the settings, then the folders and sources for the window', async () => {
    const store = useSettingsStore();
    expect(await store.load()).toBe(true);
    expect(store.settings).toEqual(defaultSettings);
    expect(await store.loadWindow()).toBe(true);
    expect(store.folders?.logs).toBe(mockFolders.logs);
    expect(store.sources).toEqual(mockSources);
  });

  it('applies an update from the answer and keeps the other settings', async () => {
    const store = useSettingsStore();
    await store.load();
    expect(await store.update({ density: 'compact' })).toBe(true);
    expect(store.settings?.density).toBe('compact');
    expect(store.settings?.theme).toBe('system');
    expect(await client.getSettings()).toEqual(store.settings);
  });

  it('keeps the saved value and shows the error when the API refuses', async () => {
    const store = useSettingsStore();
    await store.load();
    expect(await store.update({ cacheLimitMb: 1 })).toBe(false);
    expect(store.error?.code).toBe('invalidRequest');
    expect(store.settings?.cacheLimitMb).toBe(1024);
    expect(await store.update({ cacheLimitMb: 2048 })).toBe(true);
    expect(store.error).toBeNull();
  });

  it('sends changes one at a time, so the last one wins', async () => {
    const order: string[] = [];
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    setClient({
      ...client,
      updateSettings: async (req) => {
        if (req.theme === 'dark') await gate;
        order.push(req.theme ?? '');
        return client.updateSettings(req);
      },
    });
    const store = useSettingsStore();
    const first = store.update({ theme: 'dark' });
    const second = store.update({ theme: 'light' });
    release();
    await Promise.all([first, second]);
    expect(order).toEqual(['dark', 'light']);
    expect(store.settings?.theme).toBe('light');
  });

  it('adds, pauses and removes sources, and refuses to remove the system one', async () => {
    const store = useSettingsStore();
    await store.loadWindow();
    expect(await store.addSource('  /Users/demo/Ünïcødé 字体  ')).toBe(true);
    const added = store.sources.at(-1)!;
    expect(added.path).toBe('/Users/demo/Ünïcødé 字体');
    expect(await store.addSource(added.path)).toBe(false);
    expect(store.error?.code).toBe('conflict');
    expect(await store.addSource('   ')).toBe(false);
    expect(store.error?.code).toBe('invalidRequest');

    expect(await store.setSourcePaused(1, true)).toBe(true);
    expect(store.sources.find((s) => s.id === 1)?.paused).toBe(true);

    expect(await store.removeSource(2)).toBe(false);
    expect(store.error?.code).toBe('permissionDenied');
    expect(store.sources.some((s) => s.id === 2)).toBe(true);
    expect(await store.removeSource(added.id)).toBe(true);
    expect(store.sources.some((s) => s.id === added.id)).toBe(false);
  });

  it('falls back to the request defaults before the settings are read', () => {
    const store = useSettingsStore();
    expect(store.activationMode).toBe('session');
    expect(store.onConflict).toBe('keepExisting');
  });
});
