import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from './App.vue';
import { setClient, tauriClient, useClient, type FmClient } from './api/client';
import { mockAppInfo, mockClient } from './api/mock';
import type { ApiError } from './api/types';
import { formatApiVersion } from './api/version';
import { createAppRouter } from './router';
import { useAppStore } from './stores/app';
import { useCommandsStore } from './stores/commands';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => false }));

async function mountApp() {
  const router = createAppRouter();
  const wrapper = mount(App, { global: { plugins: [createPinia(), router] } });
  await router.isReady();
  await flushPromises();
  return wrapper;
}

afterEach(() => {
  setClient(undefined);
  invoke.mockReset();
});

describe('app shell', () => {
  it('shows the app info fetched through the API client', async () => {
    const text = (await mountApp()).text();
    expect(text).toContain(`${mockAppInfo.name} ${mockAppInfo.version}`);
    expect(text).toContain(formatApiVersion());
    expect(text).toContain(mockAppInfo.platform);
  });

  it('shows the API error message when the call fails', async () => {
    const failure: ApiError = {
      code: 'internal',
      message: 'The app could not finish the request.',
      detail: null,
    };
    setClient({ ...mockClient, getAppInfo: () => Promise.reject(failure) });
    const wrapper = await mountApp();
    expect(wrapper.get('[role="alert"]').text()).toBe(failure.message);
    expect(wrapper.find('[data-testid="app-info"]').exists()).toBe(false);
  });

  it('formats a version as major.minor.patch', () => {
    expect(formatApiVersion({ major: 1, minor: 12, patch: 3 })).toBe('1.12.3');
  });
});

describe('API client', () => {
  it('uses the mock outside Tauri', () => {
    expect(useClient()).toBe(mockClient);
  });

  it('calls the get_app_info Tauri command', async () => {
    invoke.mockResolvedValue(mockAppInfo);
    await expect(tauriClient.getAppInfo()).resolves.toEqual(mockAppInfo);
    expect(invoke).toHaveBeenCalledWith('get_app_info');
  });

  it('prints through the print_page command in the app', async () => {
    invoke.mockResolvedValue(null);
    await tauriClient.printPage();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith('print_page');
  });

  it("prints with the browser's own dialog in browser mode", async () => {
    const print = vi.spyOn(window, 'print').mockImplementation(() => undefined);
    await mockClient.printPage();
    expect(print).toHaveBeenCalledTimes(1);
    print.mockRestore();
  });
});

describe('app store', () => {
  it('clears an earlier error after a successful load', async () => {
    setActivePinia(createPinia());
    let fail = true;
    const client: FmClient = {
      ...mockClient,
      getAppInfo: () =>
        fail
          ? Promise.reject({ code: 'internal', message: 'x', detail: null })
          : Promise.resolve(mockAppInfo),
    };
    setClient(client);
    const store = useAppStore();
    await store.load();
    expect(store.error?.message).toBe('x');
    fail = false;
    await store.load();
    expect(store.error).toBeNull();
    expect(store.info).toEqual(mockAppInfo);
  });
});

describe('router', () => {
  it('sends an unknown path home', async () => {
    const router = createAppRouter();
    await router.push('/no/such/page');
    expect(router.currentRoute.value.name).toBe('home');
  });
});

describe('help', () => {
  it('lists one Help action per docs page in the palette, which opens that page', async () => {
    const opened: string[] = [];
    setClient({ ...mockClient, openHelp: async (page) => void opened.push(page) });
    await mountApp();
    const commands = useCommandsStore();
    const help = commands.actions.filter((a) => a.group === 'Help');
    expect(help.map((a) => a.title)).toEqual([
      'Help: Getting started',
      'Help: Sources',
      'Help: Activation',
      'Help: Keyboard shortcuts',
      'Help: Shared libraries',
    ]);
    expect(commands.run('help.keyboard-shortcuts')).toBe(true);
    expect(opened).toEqual(['keyboard-shortcuts']);
  });

  it('calls the open_help Tauri command with the page', async () => {
    invoke.mockResolvedValue(null);
    await tauriClient.openHelp('sources');
    expect(invoke).toHaveBeenCalledWith('open_help', { page: 'sources' });
  });

  it('opens the built page in browser mode and refuses an unknown page', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    await mockClient.openHelp('activation');
    expect(open).toHaveBeenCalledWith('/help/activation.html', '_blank');
    await expect(mockClient.openHelp('../secrets')).rejects.toMatchObject({ code: 'notFound' });
    expect(open).toHaveBeenCalledTimes(1);
    open.mockRestore();
  });
});
