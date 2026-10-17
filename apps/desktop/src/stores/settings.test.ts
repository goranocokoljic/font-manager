import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, tauriClient, type FmClient } from '../api/client';
import { createMockClient, defaultSettings, mockFolders } from '../api/mock';
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
  it('calls the settings Tauri commands with their arguments', async () => {
    invoke.mockResolvedValue(undefined);
    await tauriClient.getSettings();
    expect(invoke).toHaveBeenLastCalledWith('get_settings');
    await tauriClient.updateSettings({ theme: 'dark' });
    expect(invoke).toHaveBeenLastCalledWith('update_settings', { req: { theme: 'dark' } });
    await tauriClient.getAppFolders();
    expect(invoke).toHaveBeenLastCalledWith('get_app_folders');
  });
});

describe('settings store', () => {
  it('loads the settings, then the folders for the window', async () => {
    const store = useSettingsStore();
    expect(await store.load()).toBe(true);
    expect(store.settings).toEqual(defaultSettings);
    expect(await store.loadWindow()).toBe(true);
    expect(store.folders?.logs).toBe(mockFolders.logs);
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

  it('falls back to the request defaults before the settings are read', () => {
    const store = useSettingsStore();
    expect(store.activationMode).toBe('session');
    expect(store.onConflict).toBe('keepExisting');
  });
});
