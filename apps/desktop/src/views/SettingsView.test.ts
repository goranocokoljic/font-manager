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
import { useCatalogsStore } from '../stores/catalogs';
import { pickOption, selectOptions } from '../components/ui/testing';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  client = createMockClient({ setup: 'none' });
  setClient(client);
  delete document.documentElement.dataset.theme;
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

type Wrapper = Awaited<ReturnType<typeof mountAt>>['wrapper'];

/** A choice card (E57.4) by its title. */
const card = (wrapper: Wrapper, title: string) =>
  wrapper.findAll('[role="radio"]').find((c) => c.get('.title').text() === title)!;

/** A setting row's switch, by its name. */
const toggle = (wrapper: Wrapper, name: string) => wrapper.get(`[role="switch"][name="${name}"]`);

/** The text of the element that labels (or first describes) `el`. */
const labelOf = (el: { attributes: (k: string) => string | undefined }, attr = 'aria-labelledby') =>
  document.getElementById(el.attributes(attr)!.split(' ')[0]!)?.textContent?.trim();

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
    // A card without a switch (E57.5): its name, one line, and "How it works".
    const adobe = wrapper.get('[data-catalog="adobeFonts"]');
    expect(adobe.find('[role="switch"]').exists()).toBe(false);
    expect(adobe.get('.name').text()).toBe('Adobe Fonts');
    expect(adobe.get('.line').text()).toBe(
      'Activated by Creative Cloud. Font Fu shows these fonts and cannot download them.',
    );
    const entry = adobe.get('button[aria-controls="settings-adobe-fonts"]');
    expect(entry.text()).toBe('How it works');
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
    const box = (w: Wrapper) => toggle(w, 'fetch-missing-fonts');
    const hint = (w: Wrapper) => labelOf(box(w), 'aria-describedby');
    // Neither: off and disabled, saying what it needs, as the row's one line.
    let { wrapper } = await mountAt('/settings?section=catalogs');
    expect(labelOf(box(wrapper))).toBe('Download fonts a document needs');
    expect(box(wrapper).attributes('aria-checked')).toBe('false');
    expect(box(wrapper).attributes('disabled')).toBeDefined();
    expect(hint(wrapper)).toBe(
      'Choose the folder for installed fonts and turn a catalog on first.',
    );
    wrapper.unmount();

    // A folder but no catalog on: still disabled.
    await client.updateSettings({ downloadFolder: '/Users/demo/Installed fonts' });
    ({ wrapper } = await mountAt('/settings?section=catalogs'));
    expect(box(wrapper).attributes('disabled')).toBeDefined();
    // A catalog turned on: enabled, saying it downloads only from those.
    await toggle(wrapper, 'catalog-googleFonts').trigger('click');
    await flushPromises();
    expect(box(wrapper).attributes('disabled')).toBeUndefined();
    expect(hint(wrapper)).toContain('It downloads only from catalogs you turned on.');
    await box(wrapper).trigger('click');
    await flushPromises();
    expect(box(wrapper).attributes('aria-checked')).toBe('true');
    expect((await client.getSettings()).fetchMissingFonts).toBe(true);
    wrapper.unmount();
  });

  it('says on its card that a catalog is being turned on, with its switch disabled, and a failure on that card (E57.5)', async () => {
    let release: () => void = () => undefined;
    const slow = new Promise<void>((r) => (release = r));
    setClient({
      ...client,
      refreshCatalog: async () => {
        await slow;
        throw { code: 'io', message: 'Fontshare could not be reached.', detail: null };
      },
    });
    const { wrapper } = await mountAt('/settings?section=catalogs');
    const fontshare = () => wrapper.get('[data-catalog="fontshare"]');
    const google = () => wrapper.get('[data-catalog="googleFonts"]');
    await toggle(wrapper, 'catalog-fontshare').trigger('click');
    await flushPromises();
    // Connecting: said on the card, its switch waits; the other card is untouched.
    expect(fontshare().get('[data-testid="catalog-card-line"]').text()).toBe('Connecting…');
    expect(fontshare().attributes('aria-busy')).toBe('true');
    expect(toggle(wrapper, 'catalog-fontshare').attributes('disabled')).toBeDefined();
    expect(toggle(wrapper, 'catalog-googleFonts').attributes('disabled')).toBeUndefined();

    release();
    await flushPromises();
    // The failure is said on Fontshare's card, describes its switch, and nowhere else.
    const error = fontshare().get('[data-testid="catalog-card-error"]');
    expect(error.attributes('role')).toBe('alert');
    expect(error.text()).toContain('Fontshare could not be reached.');
    expect(toggle(wrapper, 'catalog-fontshare').attributes('aria-describedby')).toContain(
      error.attributes('id'),
    );
    expect(toggle(wrapper, 'catalog-fontshare').attributes('disabled')).toBeUndefined();
    expect(google().find('[data-testid="catalog-card-error"]').exists()).toBe(false);
    expect(wrapper.findAll('[role="alert"]')).toHaveLength(1);
    wrapper.unmount();
    // The mock keeps the catalogs between tests.
    await client.setCatalogEnabled({ id: 'fontshare', enabled: false });
  });

  it('says a failed list fetch on the card of a catalog that is also open in the browser (E57.5)', async () => {
    setClient({
      ...client,
      refreshCatalog: async () => {
        throw { code: 'io', message: 'Fontshare could not be reached.', detail: null };
      },
    });
    const { wrapper } = await mountAt('/settings?section=catalogs');
    const catalogs = useCatalogsStore();
    catalogs.open = 'fontshare';
    await toggle(wrapper, 'catalog-fontshare').trigger('click');
    await flushPromises();
    // The store keeps it as the open catalog's refresh error; the card still says it.
    expect(catalogs.refreshError?.message).toBe('Fontshare could not be reached.');
    expect(
      wrapper.get('[data-catalog="fontshare"] [data-testid="catalog-card-error"]').text(),
    ).toContain('Fontshare could not be reached.');
    catalogs.open = null;
    wrapper.unmount();
    await client.setCatalogEnabled({ id: 'fontshare', enabled: false });
  });

  it('draws the folder tree with one line per catalog that is on (E57.5)', async () => {
    await client.updateSettings({ downloadFolder: 'D:\\Type\\Library' });
    await client.setCatalogEnabled({ id: 'googleFonts', enabled: true });
    const { wrapper } = await mountAt('/settings?section=catalogs');
    expect(wrapper.get('[data-testid="settings-download-folder-path"]').text()).toBe(
      'D:\\Type\\Library',
    );
    const tree = wrapper.get('[data-testid="settings-catalog-paths"]');
    expect(tree.attributes('aria-label')).toBe('Where each catalog installs');
    expect(tree.findAll('li').map((l) => l.text())).toEqual([
      'D:\\Type\\Library',
      'Google Fonts\\<Family>',
    ]);
    expect(tree.get('[data-catalog="googleFonts"] .placeholder').text()).toBe('<Family>');
    // Fontshare is off: no line for it.
    expect(tree.find('[data-catalog="fontshare"]').exists()).toBe(false);
    wrapper.unmount();
    // The mock keeps the catalogs between tests.
    await client.setCatalogEnabled({ id: 'googleFonts', enabled: false });
  });

  it('turns Google Fonts on and off under Online catalogs, saying it connects', async () => {
    const { wrapper } = await mountAt('/settings?section=catalogs');
    const box = toggle(wrapper, 'catalog-googleFonts');
    const card = wrapper.get('[data-catalog="googleFonts"]');
    expect(box.attributes('aria-checked')).toBe('false');
    expect(labelOf(box)).toBe('Google Fonts');
    expect(labelOf(box, 'aria-describedby')).toContain('connects to the internet');
    // A card like a source card (E57.5): mark, name, one line, "off", the switch.
    expect(card.get('.mark').text()).toBe('G');
    expect(card.get('[data-testid="catalog-card-count"]').text()).toBe('off');
    await box.trigger('click');
    await flushPromises();
    let [google] = await client.listCatalogs();
    expect(google?.enabled).toBe(true);
    expect(google?.familyCount, 'turning it on fetched the list').toBe(4);
    expect(box.attributes('aria-checked')).toBe('true');
    expect(card.get('[data-testid="catalog-card-count"]').text()).toBe('4 families');
    expect(card.get('[data-testid="catalog-card-line"]').text()).toBe(
      'Connected. Previews are fetched while you browse it.',
    );
    await box.trigger('click');
    await flushPromises();
    [google] = await client.listCatalogs();
    expect(google?.enabled).toBe(false);
    wrapper.unmount();
  });

  it('applies the theme the moment it changes and saves it; there is no density (E51.5)', async () => {
    const { wrapper } = await mountAt('/settings?section=appearance');
    await card(wrapper, 'Dark').trigger('click');
    await flushPromises();
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(await client.getSettings()).toMatchObject({ theme: 'dark' });
    expect(wrapper.find('input[name="density"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('Density');
    expect(document.documentElement.dataset.density).toBeUndefined();
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
    await card(wrapper, 'Dark').trigger('click');
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
    expect(labelOf(select)).toBe('Language');
    expect(labelOf(select, 'aria-describedby')).toContain('System follows the language');
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

  it('saves the activation choices, drawn as choice cards with their figures (E57.4)', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const groups = wrapper.findAll('[role="radiogroup"]');
    expect(groups.map((g) => g.attributes('aria-label'))).toEqual([
      'The Activate button activates fonts',
      'When a font has the same name as a system font or an active font',
    ]);
    for (const g of groups) expect(g.classes()).toContain('cols-2');
    expect(groups[0]!.findAll('[role="radio"]').map((c) => c.text())).toEqual([
      'For this sessionUntil Font Fu quits.',
      'PermanentlyAlso after a restart, until you deactivate them.',
    ]);
    expect(groups[0]!.findAll('[data-testid="activation-figure"]')).toHaveLength(2);
    expect(card(wrapper, 'For this session').attributes('aria-checked')).toBe('true');

    await card(wrapper, 'Permanently').trigger('click');
    await card(wrapper, 'Replace without asking').trigger('click');
    await flushPromises();
    expect(await client.getSettings()).toMatchObject({
      activationMode: 'permanent',
      onConflict: 'replace',
    });
    expect(card(wrapper, 'Permanently').attributes('aria-checked')).toBe('true');
    // The arrow keys move the choice and save it.
    await card(wrapper, 'Replace without asking').trigger('keydown', { key: 'ArrowLeft' });
    await flushPromises();
    expect((await client.getSettings()).onConflict).toBe('keepExisting');
    wrapper.unmount();
  });

  it('puts the three switches under "While the app runs", each with its one line (E57.4)', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const group = wrapper.get('.ui-setting-group');
    expect(group.get('h3').text()).toBe('While the app runs');
    const switches = group.findAll('[role="switch"]');
    expect(switches.map((sw) => labelOf(sw))).toEqual([
      'Reactivate on launch',
      'Keep running in the tray when the window is closed',
      'Start at login',
    ]);
    expect(labelOf(switches[0]!, 'aria-describedby')).toBe(
      'Activate the last session’s fonts again when the app starts.',
    );
    expect(wrapper.find('input[type="checkbox"], input[type="radio"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('saves "reactivate on launch"', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = toggle(wrapper, 'reactivate-on-launch');
    expect(box.attributes('aria-checked')).toBe('false');
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).reactivateOnLaunch).toBe(true);
    expect(box.attributes('aria-checked')).toBe('true');
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).reactivateOnLaunch).toBe(false);
    wrapper.unmount();
  });

  it('saves "keep running in the tray", on by default', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = toggle(wrapper, 'keep-in-tray');
    expect(labelOf(box)).toBe('Keep running in the tray when the window is closed');
    expect(box.attributes('aria-checked')).toBe('true');
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).keepInTray).toBe(false);
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).keepInTray).toBe(true);
    wrapper.unmount();
  });

  it('hides "Start at login" where fm-api does not offer it (a Flatpak sandbox)', async () => {
    client = createMockClient({ setup: 'none', startAtLogin: 'unsupported' });
    setClient(client);
    const { wrapper } = await mountAt('/settings?section=activation');
    expect((await client.getSettings()).startAtLogin).toBeNull();
    expect(wrapper.find('[name="start-at-login"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('Start at login');
    wrapper.unmount();
  });

  it('turns "Start at login" on and off, offered and off by default', async () => {
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = toggle(wrapper, 'start-at-login');
    expect(labelOf(box)).toBe('Start at login');
    expect(box.attributes('aria-checked')).toBe('false');
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).startAtLogin).toBe(true);
    await box.trigger('click');
    await flushPromises();
    expect((await client.getSettings()).startAtLogin).toBe(false);
    wrapper.unmount();
  });

  it('says on its row when the system refuses the login entry, and the switch goes back', async () => {
    client = createMockClient({ setup: 'none', startAtLogin: 'refuse' });
    setClient(client);
    const { wrapper } = await mountAt('/settings?section=activation');
    const box = toggle(wrapper, 'start-at-login');
    await box.trigger('click');
    await flushPromises();
    const row = box.element.closest('.ui-setting-row')!;
    expect(row.querySelector('[role="alert"]')?.textContent).toContain(
      'Start at login could not be turned on',
    );
    expect(box.attributes('aria-describedby')!.split(' ')).toHaveLength(2);
    expect(box.attributes('aria-checked')).toBe('false');
    expect((await client.getSettings()).startAtLogin).toBe(false);
    // Said once, on the row: no toast, no second alert above the section.
    expect(wrapper.findAll('[role="alert"]')).toHaveLength(1);
    expect(wrapper.find('[data-testid="toast"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('draws light or dark as three choice cards with figures, the themes in one row (E57.4)', async () => {
    const { wrapper } = await mountAt('/settings?section=appearance');
    const group = wrapper.get('[role="radiogroup"][aria-label="Light or dark"]');
    expect(group.classes()).toContain('cols-3');
    expect(group.findAll('[role="radio"]').map((c) => c.text())).toEqual([
      'Same as the system',
      'Light',
      'Dark',
    ]);
    const figures = group.findAll('[data-testid="scheme-figure"]');
    expect(figures.map((f) => f.classes()[1])).toEqual(['system', 'light', 'dark']);
    // Each figure is the active theme's own paper in that mode (Paper: light, dark).
    const style = (figures[2]!.element as HTMLElement).style;
    expect(style.getPropertyValue('--dark')).not.toBe('');
    expect(style.getPropertyValue('--light')).not.toBe(style.getPropertyValue('--dark'));
    expect(card(wrapper, 'Same as the system').attributes('aria-checked')).toBe('true');
    expect(wrapper.findAll('.theme-cards [data-testid="theme-card"]')).toHaveLength(4);
    expect(wrapper.find('input[type="radio"]:not(.sr-only)').exists()).toBe(false);
    wrapper.unmount();
  });

  it('lists the platform notes under About', async () => {
    const { wrapper } = await mountAt('/settings?section=about');
    const notes = wrapper.get('ul[aria-labelledby="platform-notes-title"]');
    expect(wrapper.get('#platform-notes-title').text()).toBe('Platform notes');
    expect(notes.findAll('li').map((li) => li.text())).toEqual(mockAppInfo.platformNotes);
    expect(notes.text()).toContain('closing the window quits Font Fu');
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
    // A setting row (E57.5): its one line describes the field; the unit stands after it.
    expect(document.getElementById(field(1).attributes('aria-describedby')!)?.textContent).toBe(
      '0 turns automatic rescans off.',
    );
    expect(field(1).attributes('aria-label')).toBe('Rescan sources every (minutes)');
    expect(wrapper.findAll('.unit').map((u) => u.text())).toEqual(['MB', 'minutes']);

    await commit(1, '0');
    expect((await client.getSettings()).rescanIntervalMinutes).toBe(0);
    // The logs folder: the path in the mono face, and "Copy path"; no "Open" (E57.5).
    const logs = wrapper.get('[data-testid="settings-logs-folder"]');
    expect(logs.get('.value').text()).toBe(mockFolders.logs);
    expect(logs.findAll('button').map((b) => b.text())).toEqual(['Copy path']);
    wrapper.unmount();
  });

  it('is the shell of board 15: navigation, the section heading and line, the x to close', async () => {
    const { wrapper, router } = await mountAt('/settings?section=activation');
    // No top bar with a text button "Close".
    expect(wrapper.find('[data-top-bar]').exists()).toBe(false);
    expect(wrapper.findAll('button').some((b) => b.text() === 'Close')).toBe(false);
    const nav = wrapper.get('nav.nav');
    expect(nav.get('h1').text()).toBe('Settings');
    expect(nav.findAll('[role="tab"] .ui-icon')).toHaveLength(8);
    const panel = wrapper.get('[role="tabpanel"]');
    expect(panel.attributes('id')).toBe('panel-activation');
    expect(panel.get('h2').text()).toBe('Activation');
    expect(panel.get('.lead').text()).toBe(
      'How the Activate button behaves, and what happens to active fonts when the window closes.',
    );
    // The x icon button, named for what it closes, with Esc in its tooltip.
    const close = wrapper.get('[data-testid="settings-close"]');
    expect(close.attributes('aria-label')).toBe('Close settings');
    expect(close.attributes('aria-keyshortcuts')).toBe('Escape');
    await close.trigger('focus');
    await flushPromises();
    expect(document.body.textContent).toContain('Esc');
    await close.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('home');
    wrapper.unmount();
  });

  it('opens the section in the URL, and moves focus to its heading on a click', async () => {
    const { wrapper, router } = await mountAt('/settings?section=advanced');
    expect(wrapper.get('[role="tab"][aria-selected="true"]').text()).toBe('Advanced');
    expect(document.activeElement?.id).toBe('tab-advanced');
    await wrapper.get('#tab-about').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.query.section).toBe('about');
    expect(document.activeElement?.tagName).toBe('H2');
    expect(document.activeElement?.textContent).toBe('About');
    // Sources keeps its own head; its heading takes focus the same way.
    await wrapper.get('#tab-sources').trigger('click');
    await flushPromises();
    expect(document.activeElement?.id).toBe('sources-title');
    wrapper.unmount();
  });

  it('closes on Escape when a click on plain text left focus on the page', async () => {
    const { wrapper, router } = await mountAt('/settings?section=about');
    (document.activeElement as HTMLElement | null)?.blur();
    expect(document.activeElement).toBe(document.body);
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('home');
    wrapper.unmount();
  });

  it('leaves Escape to a dialog outside the view', async () => {
    const { wrapper, router } = await mountAt('/settings?section=about');
    const dialog = document.createElement('div');
    dialog.tabIndex = -1;
    document.body.append(dialog);
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('settings');
    dialog.remove();
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
