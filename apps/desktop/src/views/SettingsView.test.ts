import { flushPromises, mount } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockAppInfo, mockFolders, shellLocale } from '../api/mock';
import { setLocale } from '../i18n';
import { createAppRouter } from '../router';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  client = createMockClient();
  setClient(client);
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.density;
});

afterEach(() => setClient(undefined));

async function mountAt(path: string) {
  const router = createAppRouter();
  await router.push(path);
  const wrapper = mount(App, {
    global: { plugins: [createPinia(), router] },
    attachTo: document.body,
  });
  await router.isReady();
  await flushPromises();
  return { wrapper, router };
}

const radio = (wrapper: Awaited<ReturnType<typeof mountAt>>['wrapper'], label: string) =>
  wrapper
    .findAll('label')
    .find((l) => l.text() === label)!
    .get('input');

describe('Settings window', () => {
  it('has the six sections as tabs and opens Sources first', async () => {
    const { wrapper } = await mountAt('/settings');
    expect(wrapper.findAll('[role="tab"]').map((t) => t.text())).toEqual([
      'Sources',
      'Activation',
      'Appearance',
      'Online catalogs',
      'Advanced',
      'About',
    ]);
    expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('Sources');
    expect(wrapper.text()).toContain('/Users/demo/Fonts');
    expect(wrapper.text()).toContain('System fonts');
    wrapper.unmount();
  });

  it('lists Adobe Fonts under Online catalogs and opens its note', async () => {
    const { wrapper } = await mountAt('/settings?section=catalogs');
    expect(wrapper.get('h2').text()).toBe('Online catalogs');
    const entry = wrapper.get('button[aria-controls="settings-adobe-fonts"]');
    expect(entry.text()).toBe('Adobe Fonts');
    expect(entry.attributes('aria-expanded')).toBe('false');
    expect(wrapper.find('#settings-adobe-fonts').exists()).toBe(false);
    await entry.trigger('click');
    expect(entry.attributes('aria-expanded')).toBe('true');
    const note = wrapper.get('#settings-adobe-fonts');
    expect(note.text()).toContain('no third-party app can manage them');
    expect(note.get('a').text()).toBe('Open Adobe Fonts');
    wrapper.unmount();
  });

  it('applies the theme and density the moment they change, and saves them', async () => {
    const { wrapper } = await mountAt('/settings?section=appearance');
    await radio(wrapper, 'Dark').setValue(true);
    await flushPromises();
    expect(document.documentElement.dataset.theme).toBe('dark');
    await radio(wrapper, 'Compact').setValue(true);
    await flushPromises();
    expect(document.documentElement.dataset.density).toBe('compact');
    expect(await client.getSettings()).toMatchObject({ theme: 'dark', density: 'compact' });
    wrapper.unmount();
  });

  it('offers the languages and relabels the window the moment one is chosen', async () => {
    const spy = vi.spyOn(client, 'updateSettings');
    const { wrapper } = await mountAt('/settings?section=appearance');
    const select = wrapper.get('select[name="language"]');
    expect(select.element.closest('label')!.textContent).toContain('Language');
    expect(select.findAll('option').map((o) => o.element.value)).toEqual(['system', 'en', 'de']);
    expect(select.findAll('option').map((o) => o.text())).toEqual([
      'System (English)',
      'English',
      'Deutsch',
    ]);
    expect(wrapper.get('h2').text()).toBe('Appearance');
    expect(shellLocale.value, 'the native shell is told the locale at start').toBe('en');
    try {
      await select.setValue('de');
      await flushPromises();
      expect(spy).toHaveBeenCalledWith({ language: 'de' });
      expect((await client.getSettings()).language).toBe('de');
      expect(wrapper.get('h2').text()).toBe('Darstellung');
      expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('Darstellung');
      expect(shellLocale.value, 'the Help menu and tray follow the choice').toBe('de');
    } finally {
      wrapper.unmount();
      setLocale('en');
    }
  });

  it('tells the native shell the OS language when the setting is "System"', async () => {
    const languages = vi.spyOn(navigator, 'languages', 'get').mockReturnValue(['de-DE', 'en-US']);
    shellLocale.value = '';
    const { wrapper } = await mountAt('/settings?section=appearance');
    try {
      expect((await client.getSettings()).language).toBe('system');
      expect(shellLocale.value).toBe('de');
      expect(wrapper.get('h2').text()).toBe('Darstellung');
    } finally {
      wrapper.unmount();
      languages.mockRestore();
      setLocale('en');
    }
  });

  it('saves the activation choices', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    await radio(wrapper, 'Permanently (also after a restart)').setValue(true);
    await radio(wrapper, 'Replace the other font for this session without asking').setValue(true);
    await flushPromises();
    expect(await client.getSettings()).toMatchObject({
      activationMode: 'permanent',
      onConflict: 'replace',
    });
    wrapper.unmount();
  });

  it('saves "reactivate on launch"', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = wrapper.get('input[name="reactivate-on-launch"]');
    expect((box.element as HTMLInputElement).checked).toBe(false);
    await box.setValue(true);
    await flushPromises();
    expect((await client.getSettings()).reactivateOnLaunch).toBe(true);
    await box.setValue(false);
    await flushPromises();
    expect((await client.getSettings()).reactivateOnLaunch).toBe(false);
    wrapper.unmount();
  });

  it('saves "keep running in the tray", on by default', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = wrapper.get('input[name="keep-in-tray"]');
    expect(box.element.closest('label')!.textContent).toContain(
      'Keep running in the tray when the window is closed',
    );
    expect((box.element as HTMLInputElement).checked).toBe(true);
    await box.setValue(false);
    await flushPromises();
    expect((await client.getSettings()).keepInTray).toBe(false);
    await box.setValue(true);
    await flushPromises();
    expect((await client.getSettings()).keepInTray).toBe(true);
    wrapper.unmount();
  });

  it('hides "Start at login" where fm-api does not offer it (a Flatpak sandbox)', async () => {
    client = createMockClient({ startAtLogin: 'unsupported' });
    setClient(client);
    const { wrapper } = await mountAt('/settings?section=activation');
    expect((await client.getSettings()).startAtLogin).toBeNull();
    expect(wrapper.find('input[name="start-at-login"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('Start at login');
    wrapper.unmount();
  });

  it('turns "Start at login" on and off, offered and off by default', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = wrapper.get('input[name="start-at-login"]');
    expect(box.element.closest('label')!.textContent!.trim()).toBe('Start at login');
    expect((box.element as HTMLInputElement).checked).toBe(false);
    await box.setValue(true);
    await flushPromises();
    expect((await client.getSettings()).startAtLogin).toBe(true);
    await box.setValue(false);
    await flushPromises();
    expect((await client.getSettings()).startAtLogin).toBe(false);
    wrapper.unmount();
  });

  it('shows a toast and the real state when the system refuses the login entry', async () => {
    client = createMockClient({ startAtLogin: 'refuse' });
    setClient(client);
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = wrapper.get('input[name="start-at-login"]');
    await box.setValue(true);
    await flushPromises();
    expect(wrapper.get('[data-testid="toast"]').text()).toContain(
      'Start at login could not be turned on',
    );
    expect((box.element as HTMLInputElement).checked).toBe(false);
    expect((await client.getSettings()).startAtLogin).toBe(false);
    wrapper.unmount();
  });

  it('lists the platform notes under About', async () => {
    const { wrapper } = await mountAt('/settings?section=about');
    const notes = wrapper.get('ul[aria-labelledby="platform-notes-title"]');
    expect(wrapper.get('#platform-notes-title').text()).toBe('Platform notes');
    expect(notes.findAll('li').map((li) => li.text())).toEqual(mockAppInfo.platformNotes);
    expect(notes.text()).toContain('closing the window quits Font Manager');
    wrapper.unmount();
  });

  it('says so when the platform has no notes', async () => {
    setClient({ ...client, getAppInfo: async () => ({ ...mockAppInfo, platformNotes: [] }) });
    const { wrapper } = await mountAt('/settings?section=about');
    expect(wrapper.find('ul[aria-labelledby="platform-notes-title"]').exists()).toBe(false);
    expect(wrapper.text()).toContain('Everything works the same on this system.');
    wrapper.unmount();
  });

  it('saves a valid number, and puts the saved value back after a bad one', async () => {
    const { wrapper } = await mountAt('/settings?section=advanced');
    const [cache, rescan] = wrapper.findAll('input[type="number"]');
    await cache.setValue('2048');
    await flushPromises();
    expect((await client.getSettings()).cacheLimitMb).toBe(2048);

    await cache.setValue('1');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('between 64 and 16384');
    expect((cache.element as HTMLInputElement).value).toBe('2048');

    await rescan.setValue('');
    await flushPromises();
    expect((rescan.element as HTMLInputElement).value).toBe('60');
    expect((await client.getSettings()).rescanIntervalMinutes).toBe(60);

    await rescan.setValue('0');
    await flushPromises();
    expect((await client.getSettings()).rescanIntervalMinutes).toBe(0);
    expect(wrapper.text()).toContain(mockFolders.logs);
    wrapper.unmount();
  });

  it('moves between sections with the arrow keys and closes on Escape', async () => {
    const { wrapper, router } = await mountAt('/settings');
    const tablist = wrapper.get('[role="tablist"]');
    await tablist.trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(router.currentRoute.value.query.section).toBe('activation');
    expect(document.activeElement?.id).toBe('tab-activation');
    await tablist.trigger('keydown', { key: 'End' });
    await flushPromises();
    expect(router.currentRoute.value.query.section).toBe('about');
    await tablist.trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(router.currentRoute.value.query.section).toBe('sources');

    await wrapper.get('.settings').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('home');
    wrapper.unmount();
  });
});
