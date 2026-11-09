import { flushPromises, mount } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import {
  createMockClient,
  mockAppInfo,
  mockFolders,
  mockPickedFolders,
  shellLocale,
} from '../api/mock';
import { setLocale } from '../i18n';
import { createAppRouter } from '../router';
import { pickOption, selectOptions } from '../components/ui/testing';

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
  it('has the eight sections as tabs and opens Sources first', async () => {
    const { wrapper } = await mountAt('/settings');
    expect(wrapper.findAll('[role="tab"]').map((t) => t.text())).toEqual([
      'Sources',
      'Activation',
      'Appearance',
      'Online catalogs',
      'Advanced',
      'Local API',
      'AI assistants',
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

  it('shows the folder for installed fonts under Online catalogs, with Change…', async () => {
    mockPickedFolders.length = 0;
    const { wrapper } = await mountAt('/settings?section=catalogs');
    const field = wrapper.get('[data-testid="settings-download-folder"]');
    expect(field.text()).toContain('Folder for installed fonts');
    expect(field.text()).toContain('Not chosen yet. The first Install asks for it.');
    const change = field.get('[data-testid="settings-download-folder-change"]');
    expect(change.text()).toBe('Change…');
    mockPickedFolders.push('/Users/demo/Installed fonts');
    await change.trigger('click');
    await flushPromises();
    expect(field.get('[data-testid="settings-download-folder-path"]').text()).toBe(
      '/Users/demo/Installed fonts',
    );
    expect((await client.getSettings()).downloadFolder).toBe('/Users/demo/Installed fonts');
    wrapper.unmount();
  });

  it('offers to download fonts a document needs only with a download folder and a catalog on', async () => {
    const box = (w: Awaited<ReturnType<typeof mountAt>>['wrapper']) =>
      w.get('input[name="fetch-missing-fonts"]').element as HTMLInputElement;
    const hint = (w: Awaited<ReturnType<typeof mountAt>>['wrapper']) =>
      w.get('[data-testid="fetch-missing-fonts-hint"]').text();
    // Neither: off and disabled, saying what it needs.
    let { wrapper } = await mountAt('/settings?section=catalogs');
    expect(box(wrapper).closest('label')?.textContent?.trim()).toBe(
      'Download fonts a document needs from the catalogs you turned on',
    );
    expect(box(wrapper).checked).toBe(false);
    expect(box(wrapper).disabled).toBe(true);
    expect(hint(wrapper)).toBe(
      'Choose the folder for installed fonts and turn a catalog on first.',
    );
    wrapper.unmount();

    // A folder but no catalog on: still disabled.
    await client.updateSettings({ downloadFolder: '/Users/demo/Installed fonts' });
    ({ wrapper } = await mountAt('/settings?section=catalogs'));
    expect(box(wrapper).disabled).toBe(true);
    // A catalog turned on: enabled, saying it downloads only from those.
    await wrapper.get('input[name="catalog-googleFonts"]').setValue(true);
    await flushPromises();
    expect(box(wrapper).disabled).toBe(false);
    expect(hint(wrapper)).toContain('It downloads only from catalogs you turned on.');
    await wrapper.get('input[name="fetch-missing-fonts"]').setValue(true);
    await flushPromises();
    expect((await client.getSettings()).fetchMissingFonts).toBe(true);
    wrapper.unmount();
  });

  it('turns Google Fonts on and off under Online catalogs, saying it connects', async () => {
    const { wrapper } = await mountAt('/settings?section=catalogs');
    const box = wrapper.get('input[name="catalog-googleFonts"]');
    expect(box.attributes('type')).toBe('checkbox');
    expect((box.element as HTMLInputElement).checked).toBe(false);
    expect(box.element.closest('label')?.textContent?.trim()).toBe('Google Fonts');
    expect(wrapper.get(`#${box.attributes('aria-describedby')}`).text()).toContain(
      'connects to the internet',
    );
    await box.setValue(true);
    await flushPromises();
    let [google] = await client.listCatalogs();
    expect(google?.enabled).toBe(true);
    expect(google?.familyCount, 'turning it on fetched the list').toBe(4);
    await box.setValue(false);
    await flushPromises();
    [google] = await client.listCatalogs();
    expect(google?.enabled).toBe(false);
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

  it('shows the themes as cards in their own tokens and applies and saves a choice (E47.2)', async () => {
    document.documentElement.removeAttribute('style');
    const { wrapper } = await mountAt('/settings?section=appearance');
    const cards = wrapper.findAll('[data-testid="theme-card"]');
    expect(cards.map((c) => c.get('.name').text())).toEqual([
      'Paper',
      'Graphite',
      'Indigo',
      'Moss',
    ]);
    const moss = cards[3]!;
    // A live mini wall: three specimens, one selected, on the theme's own (light) surface.
    expect(moss.findAll('[data-testid="theme-specimen"]')).toHaveLength(3);
    expect(moss.findAll('.specimen.selected')).toHaveLength(1);
    const style = (moss.element as HTMLElement).style;
    expect(style.getPropertyValue('--fm-accent')).toBe('#4f7033');
    expect(style.getPropertyValue('--fm-surface')).toBe('#f6f7f1');
    expect(cards[0]!.classes()).toContain('checked');
    expect(moss.classes()).not.toContain('checked');

    await moss.get('input[type="radio"]').setValue(true);
    await flushPromises();
    expect(document.documentElement.style.getPropertyValue('--fm-accent')).toBe('#4f7033');
    expect((await client.getSettings()).themeId).toBe('moss');
    expect(wrapper.get('[data-theme-id="moss"]').classes()).toContain('checked');

    // The cards follow the light or dark mode.
    await radio(wrapper, 'Dark').setValue(true);
    await flushPromises();
    const dark = (wrapper.get('[data-theme-id="moss"]').element as HTMLElement).style;
    expect(dark.getPropertyValue('--fm-accent')).toBe('#8fb573');
    expect(document.documentElement.style.getPropertyValue('--fm-accent')).toBe('#8fb573');
    wrapper.unmount();
  });

  it('applies the saved theme at start, from the settings (E47.2)', async () => {
    document.documentElement.removeAttribute('style');
    await client.updateSettings({ themeId: 'indigo' });
    const { wrapper } = await mountAt('/');
    expect(document.documentElement.style.getPropertyValue('--fm-accent')).toBe('#4f46c8');
    wrapper.unmount();
  });

  it('offers the languages and relabels the window the moment one is chosen', async () => {
    const spy = vi.spyOn(client, 'updateSettings');
    const { wrapper } = await mountAt('/settings?section=appearance');
    const select = wrapper.get('[role="combobox"][name="language"]');
    expect(select.element.closest('label')!.textContent).toContain('Language');
    const options = await selectOptions(select);
    expect(options.map(([value]) => value)).toEqual(['system', 'en', 'de']);
    expect(options.map(([, label]) => label)).toEqual(['System (English)', 'English', 'Deutsch']);
    expect(wrapper.get('h2').text()).toBe('Appearance');
    expect(shellLocale.value, 'the native shell is told the locale at start').toBe('en');
    try {
      await pickOption(select, 'de');
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

  it('saves a committed number, and puts the saved value back after a bad one', async () => {
    const { wrapper } = await mountAt('/settings?section=advanced');
    const field = (i: number) => wrapper.findAll('.number input')[i];
    const commit = async (i: number, value: string) => {
      await field(i).setValue(value); // fires input and change, as leaving the field does
      await flushPromises();
    };
    // Typing alone sends nothing: "1" on the way to "1024" must not be refused.
    // (`setValue` also fires `change`, so type through the element and `input` only.)
    (field(0).element as HTMLInputElement).value = '1';
    await field(0).trigger('input');
    await flushPromises();
    expect(wrapper.text()).not.toContain('between 64 and 16384');
    expect((await client.getSettings()).cacheLimitMb).toBe(1024);

    await commit(0, '2048');
    expect((await client.getSettings()).cacheLimitMb).toBe(2048);

    await commit(0, '1');
    expect(wrapper.get('[role="alert"]').text()).toContain('between 64 and 16384');
    expect((field(0).element as HTMLInputElement).value).toBe('2048');

    await commit(1, '');
    expect((field(1).element as HTMLInputElement).value).toBe('60');
    expect((await client.getSettings()).rescanIntervalMinutes).toBe(60);
    expect(field(1).attributes('aria-describedby')).toBe('rescan-hint');

    await commit(1, '0');
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
