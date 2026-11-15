import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockMoreSources, mockPickedFolders } from '../api/mock';
import { createAppRouter } from '../router';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  client = createMockClient({ moreSources: mockMoreSources, scanStepMs: 5 });
  setClient(client);
});

afterEach(() => setClient(undefined));

async function mountSources() {
  const router = createAppRouter();
  await router.push('/settings?section=sources');
  const wrapper = mount(App, {
    global: { plugins: [createPinia(), router] },
    attachTo: document.body,
  });
  await router.isReady();
  await flushPromises();
  return { wrapper, router };
}

const card = (wrapper: VueWrapper, name: string) =>
  wrapper.get(`.cards > li[aria-label="${name}"]`);
const button = (wrapper: VueWrapper | ReturnType<VueWrapper['get']>, label: string) =>
  wrapper.get(`button[aria-label="${label}"]`);

/** Waits until `check` holds (the mock's scan steps on timers). */
async function until(check: () => boolean): Promise<void> {
  for (let i = 0; i < 200 && !check(); i++) {
    await new Promise((r) => setTimeout(r, 5));
    await flushPromises();
  }
  expect(check()).toBe(true);
}

describe('Settings › Sources (E50.13)', () => {
  it('has a title, one line of explanation and Add folder as the one primary button', async () => {
    const { wrapper } = await mountSources();
    const section = wrapper.get('section[aria-labelledby="sources-title"]');
    expect(section.get('h2').text()).toBe('Sources');
    expect(section.get('.note').text()).toContain('Nothing is copied or moved');
    const primaries = section.findAll('.head .primary');
    expect(primaries.map((b) => b.text())).toEqual(['Add folder']);
    wrapper.unmount();
  });

  it('shows one card per folder: icon, name, mono path, count, last scan, Watch, actions', async () => {
    const { wrapper } = await mountSources();
    const names = wrapper.findAll('.cards > li').map((li) => li.attributes('aria-label'));
    // The system fonts have their own card, not one in the list.
    expect(names).toEqual(['Fonts', 'Studio type', 'Old downloads']);

    const fonts = card(wrapper, 'Fonts');
    expect(fonts.get('.name').text()).toBe('Fonts');
    expect(fonts.get('.path').text()).toBe('/Users/demo/Fonts');
    expect(fonts.get('.count').text()).toBe('7 fonts');
    expect(fonts.get('.when').text()).toMatch(/^scanned /);
    expect(fonts.get('[role="switch"]').attributes('aria-label')).toBe('Watch Fonts');
    expect(fonts.get('[role="switch"]').attributes('aria-checked')).toBe('true');
    for (const label of [
      'Rescan Fonts',
      'Show Fonts in the file manager',
      'Remove Fonts from the library',
    ])
      expect(button(fonts, label).classes()).toContain('ui-icon-button');
    expect(fonts.find('.badge').exists()).toBe(false);
    wrapper.unmount();
  });

  it('draws a cloud for a folder a sync client keeps and a folder for the others', async () => {
    const { wrapper } = await mountSources();
    const icon = (name: string) => card(wrapper, name).get('.tile .ui-icon').html();
    expect(icon('Studio type')).not.toBe(icon('Fonts'));
    expect(icon('Old downloads')).toBe(icon('Fonts'));
    wrapper.unmount();
  });

  it('marks a missing folder with a warning badge and cannot rescan or show it', async () => {
    const { wrapper } = await mountSources();
    const old = card(wrapper, 'Old downloads');
    expect(old.classes()).toContain('missing');
    const badge = old.get('.badge');
    expect(badge.text()).toBe('Missing');
    expect(badge.classes()).toContain('tone-warning');
    expect(old.get('.when').text()).toBe('folder not found');
    expect(button(old, 'Rescan Old downloads').attributes('disabled')).toBeDefined();
    expect(
      button(old, 'Show Old downloads in the file manager').attributes('disabled'),
    ).toBeDefined();
    // A folder that is there has no Missing badge.
    expect(card(wrapper, 'Studio type').find('.badge').exists()).toBe(false);
    wrapper.unmount();
  });

  it('shows Scanning and a progress bar fed by the scan events, then the new count', async () => {
    const { wrapper } = await mountSources();
    await button(card(wrapper, 'Fonts'), 'Rescan Fonts').trigger('click');
    await until(() => card(wrapper, 'Fonts').find('progress').exists());
    const fonts = card(wrapper, 'Fonts');
    expect(fonts.get('.badge').text()).toBe('Scanning');
    const bar = fonts.get('progress');
    expect(bar.attributes('aria-label')).toBe('Scanning Fonts');
    expect(bar.attributes('max')).toBe('7');
    expect(fonts.get('.when').text()).toBe('scanning…');
    await until(() => !card(wrapper, 'Fonts').find('progress').exists());
    expect(card(wrapper, 'Fonts').find('.badge').exists()).toBe(false);
    expect(card(wrapper, 'Fonts').get('.when').text()).toMatch(/^scanned /);
    wrapper.unmount();
  });

  it('turns watching off and on with the Watch switch', async () => {
    const spy = vi.spyOn(client, 'setSourcePaused');
    const { wrapper } = await mountSources();
    const watch = () => card(wrapper, 'Fonts').get('[role="switch"]');
    await watch().trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenLastCalledWith({ source: 1, paused: true });
    expect(watch().attributes('aria-checked')).toBe('false');
    await watch().trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenLastCalledWith({ source: 1, paused: false });
    expect(watch().attributes('aria-checked')).toBe('true');
    wrapper.unmount();
  });

  it('shows the folder in the file manager by its source, not by a path', async () => {
    const spy = vi.spyOn(client, 'showSourceFolder');
    const { wrapper } = await mountSources();
    await button(card(wrapper, 'Studio type'), 'Show Studio type in the file manager').trigger(
      'click',
    );
    await flushPromises();
    expect(spy).toHaveBeenCalledWith(901);
    wrapper.unmount();
  });

  it('asks once before removing; Cancel and Escape keep the source, Remove forgets it', async () => {
    const spy = vi.spyOn(client, 'removeSource');
    const { wrapper, router } = await mountSources();
    const remove = () => button(card(wrapper, 'Fonts'), 'Remove Fonts from the library');

    await remove().trigger('click');
    await flushPromises();
    const dialog = card(wrapper, 'Fonts').get('[role="alertdialog"]');
    expect(dialog.text()).toContain('the files stay where they are');
    expect(document.activeElement?.textContent?.trim()).toBe('Remove');
    await dialog
      .findAll('button')
      .find((b) => b.text() === 'Cancel')!
      .trigger('click');
    await flushPromises();
    expect(card(wrapper, 'Fonts').find('[role="alertdialog"]').exists()).toBe(false);
    expect(document.activeElement).toBe(remove().element);

    await remove().trigger('click');
    await flushPromises();
    await card(wrapper, 'Fonts').get('[role="alertdialog"]').trigger('keydown', { key: 'Escape' });
    await flushPromises();
    expect(card(wrapper, 'Fonts').find('[role="alertdialog"]').exists()).toBe(false);
    // Escape answered the question; Settings stays open.
    expect(router.currentRoute.value.name).toBe('settings');
    expect(spy).not.toHaveBeenCalled();

    await remove().trigger('click');
    await flushPromises();
    await card(wrapper, 'Fonts').get('#source-confirm-1').trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith(1);
    expect(wrapper.find('.cards > li[aria-label="Fonts"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('adds a picked folder as a card and scans it', async () => {
    mockPickedFolders.push('/Users/demo/Client brand');
    const { wrapper } = await mountSources();
    await wrapper.get('#sources-add').trigger('click');
    await until(() => wrapper.find('.cards > li[aria-label="Client brand"]').exists());
    await until(() => card(wrapper, 'Client brand').get('.count').text() === '12 fonts');
    wrapper.unmount();
  });

  it('keeps the System fonts card with its Show switch and the read-only line', async () => {
    const spy = vi.spyOn(client, 'setSourcePaused');
    const { wrapper } = await mountSources();
    const system = wrapper.get('.system');
    expect(system.text()).toContain('the app never writes to system folders');
    const show = system.get('[role="switch"]');
    expect(show.attributes('aria-label')).toBe('Show system fonts in the library');
    expect(show.attributes('aria-checked')).toBe('true');
    await show.trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenLastCalledWith({ source: 2, paused: true });
    // System fonts are never removed.
    expect(system.find('button[aria-label*="Remove"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('has the drop zone under the cards, and it is the empty state without folders', async () => {
    const { wrapper } = await mountSources();
    const zone = wrapper.get('button[data-testid="sources-drop"]');
    expect(zone.text()).toBe('Choose a folder to add');
    // A click opens the folder picker, as Add folder does (no drop: ADR 0017).
    mockPickedFolders.push('/Users/demo/Client brand');
    await zone.trigger('click');
    await until(() => wrapper.find('.cards > li[aria-label="Client brand"]').exists());
    wrapper.unmount();

    for (const s of await client.listSources())
      if (s.kind !== 'system') await client.removeSource(s.id);
    const empty = await mountSources();
    expect(empty.wrapper.find('.cards').exists()).toBe(false);
    expect(empty.wrapper.get('[data-testid="sources-drop"]').text()).toContain('No folders yet');
    empty.wrapper.unmount();
  });

  it('links to Problems with the counts of each kind, and hides the strip without problems', async () => {
    const problems = await client.listProblems();
    const { wrapper } = await mountSources();
    const strip = wrapper.find('[data-testid="sources-problems"]');
    if (problems.length === 0) {
      expect(strip.exists()).toBe(false);
    } else {
      expect(strip.get('strong').text()).toBe(
        `${problems.length} problem${problems.length === 1 ? '' : 's'}`,
      );
      expect(strip.get('a').text()).toBe('Open Problems');
      expect(strip.get('a').attributes('href')).toContain('/problems');
    }
    wrapper.unmount();

    vi.spyOn(client, 'listProblems').mockResolvedValue([
      { kind: 'duplicateFile', face: 1, path: '/a.otf', other: '/b.otf', message: 'x' },
      { kind: 'duplicateFace', face: 2, path: '/c.otf', other: '/d.otf', message: 'x' },
      { kind: 'broken', face: null, path: '/e.otf', other: null, message: 'x' },
    ]);
    const counted = await mountSources();
    const text = counted.wrapper.get('[data-testid="sources-problems"]').text();
    expect(text).toContain('3 problems');
    expect(text).toContain('2 duplicates, 1 broken file');
    counted.wrapper.unmount();

    vi.spyOn(client, 'listProblems').mockResolvedValue([]);
    const none = await mountSources();
    expect(none.wrapper.find('[data-testid="sources-problems"]').exists()).toBe(false);
    none.wrapper.unmount();
  });
});
