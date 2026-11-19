import { flushPromises, mount } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, shellNavigateHandlers, type MockOptions } from '../api/mock';
import type { InstallUpdateRequest } from '../api/types';
import { createAppRouter } from '../router';
import { useUpdatesStore } from '../stores/updates';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

afterEach(() => setClient(undefined));

function client(options: MockOptions = {}) {
  const installs: InstallUpdateRequest[] = [];
  const c: FmClient = createMockClient({
    setup: 'none',
    ...options,
    onInstall: (req) => installs.push(req),
  });
  const check = vi.spyOn(c, 'checkForUpdate');
  setClient(c);
  return { c, installs, check };
}

async function mountAt(path: string) {
  const router = createAppRouter();
  await router.push(path);
  const wrapper = mount(App, {
    global: { plugins: [createPinia(), router] },
    attachTo: document.body,
  });
  await router.isReady();
  await flushPromises();
  return wrapper;
}

type Wrapper = Awaited<ReturnType<typeof mountAt>>;

const button = (w: Wrapper, text: string) => w.findAll('button').find((b) => b.text() === text);
const dialog = (w: Wrapper) => w.find('[role="dialog"][aria-labelledby="fm-update-title"]');

async function checkNow(w: Wrapper) {
  await button(w, 'Check for updates')!.trigger('click');
  await flushPromises();
}

describe('Updates (Settings → About)', () => {
  it('never asks the release server without a user action (off by default)', async () => {
    const { c, check } = client({ update: 'patch' });
    expect((await c.getSettings()).checkUpdatesAutomatically).toBe(false);
    const home = await mountAt('/');
    home.unmount();
    const about = await mountAt('/settings?section=about');
    expect(check).not.toHaveBeenCalled();
    expect(dialog(about).exists()).toBe(false);
    about.unmount();
  });

  it('never checks by itself with "Check automatically" on: the shell does (E55.7)', async () => {
    const { c, check } = client({ update: 'patch' });
    await c.updateSettings({ checkUpdatesAutomatically: true });
    const w = await mountAt('/');
    expect(check).not.toHaveBeenCalled();
    expect(dialog(w).exists()).toBe(false);
    w.unmount();
  });

  it('records an update the shell found at start without opening the dialog', async () => {
    const { c, check } = client({ update: 'patch' });
    // The shell's check at start, before the window listened.
    await c.checkForUpdate();
    check.mockClear();
    const w = await mountAt('/');
    expect(check).not.toHaveBeenCalled();
    expect(dialog(w).exists()).toBe(false);
    expect(useUpdatesStore().status).toBe('available');
    expect(useUpdatesStore().update?.version).toBe('0.1.1');
    w.unmount();
  });

  it('an updateAvailable event records the update; the tray item opens the dialog', async () => {
    const { c, check } = client({ update: 'patch' });
    const w = await mountAt('/');
    const store = useUpdatesStore();
    expect(store.update).toBeNull();
    const found = (await c.checkForUpdate()).update!;
    check.mockClear();
    store.onEvent({ type: 'updateAvailable', update: found });
    await flushPromises();
    expect(dialog(w).exists()).toBe(false);
    expect(store.status).toBe('available');

    // "Update to 0.1.1…" in the tray: the shell shows the window, the UI opens the dialog.
    shellNavigateHandlers.forEach((h) => h('update'));
    await flushPromises();
    expect(dialog(w).text()).toContain('Font Fu 0.1.1 is available');
    expect(check).not.toHaveBeenCalled();
    w.unmount();
  });

  it('the tray item opens nothing when no update is on offer', async () => {
    const { check } = client({ update: 'patch' });
    const w = await mountAt('/');
    shellNavigateHandlers.forEach((h) => h('update'));
    await flushPromises();
    expect(dialog(w).exists()).toBe(false);
    expect(check).not.toHaveBeenCalled();
    w.unmount();
  });

  it('saves the "Check automatically" switch', async () => {
    const { c } = client();
    const w = await mountAt('/settings?section=about');
    const box = w.get('input[name="check-updates-automatically"]');
    expect((box.element as HTMLInputElement).checked).toBe(false);
    await box.setValue(true);
    await flushPromises();
    expect((await c.getSettings()).checkUpdatesAutomatically).toBe(true);
    w.unmount();
  });

  it('says so when the app is up to date', async () => {
    client();
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    expect(w.get('.updates [role="status"]').text()).toBe('Font Fu is up to date.');
    expect(dialog(w).exists()).toBe(false);
    w.unmount();
  });

  it('shows the reason when the release server cannot be reached', async () => {
    client({ update: 'offline' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    expect(w.get('.updates [role="alert"]').text()).toContain('could not reach its release server');
    w.unmount();
  });

  it('shows the version and release notes and installs only on confirmation', async () => {
    const { installs } = client({ update: 'patch' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    const d = dialog(w);
    expect(d.get('#fm-update-title').text()).toBe('Font Fu 0.1.1 is available');
    expect(d.text()).toContain('You have version 0.1.0.');
    expect(d.get('[aria-label="Release notes"]').text()).toContain('Scans skip unreadable folders');
    expect(installs).toEqual([]);

    // Later: nothing installs, the offer stays.
    await button(w, 'Later')!.trigger('click');
    await flushPromises();
    expect(dialog(w).exists()).toBe(false);
    expect(installs).toEqual([]);
    await button(w, 'Show version 0.1.1')!.trigger('click');
    await flushPromises();

    await button(w, 'Install and restart')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([{ version: '0.1.1', confirmMajor: false }]);
    expect(dialog(w).get('[role="status"]').text()).toContain('Font Fu restarts');
    w.unmount();
  });

  it('asks a second time before it installs a major update', async () => {
    const { installs } = client({ update: 'major' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    await button(w, 'Install and restart')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([]);
    expect(dialog(w).get('#fm-update-title').text()).toBe('Install a major update?');
    expect(dialog(w).text()).toContain('Version 1.0.0 is a major update from 0.1.0');
    expect(document.activeElement?.textContent?.trim()).toBe('Install version 1.0.0');

    // Cancel at the second question: nothing installs.
    await button(w, 'Cancel')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([]);

    await button(w, 'Show version 1.0.0')!.trigger('click');
    await flushPromises();
    expect(dialog(w).get('#fm-update-title').text()).toBe('Font Fu 1.0.0 is available');
    await button(w, 'Install and restart')!.trigger('click');
    await flushPromises();
    await button(w, 'Install version 1.0.0')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([{ version: '1.0.0', confirmMajor: true }]);
    w.unmount();
  });

  it('shows the refusal of an unsigned package and installs nothing', async () => {
    const { installs } = client({ update: 'unsigned' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    await button(w, 'Install and restart')!.trigger('click');
    await flushPromises();
    expect(dialog(w).get('[role="alert"]').text()).toContain(
      "not signed with Font Fu's release key",
    );
    expect(installs).toEqual([]);
    expect(button(w, 'Install and restart')!.attributes('disabled')).toBeUndefined();
    w.unmount();
  });

  it('hides the section where Flatpak installs the updates; the platform notes say why', async () => {
    const { check } = client({ update: 'managed' });
    const w = await mountAt('/settings?section=about');
    expect(w.find('.updates').exists()).toBe(false);
    expect(button(w, 'Check for updates')).toBeUndefined();
    expect(w.get('ul[aria-labelledby="platform-notes-title"]').text()).toContain(
      'Flatpak installs the updates of this copy',
    );
    expect(check).not.toHaveBeenCalled();
    w.unmount();
  });

  it('keeps focus and keys in the dialog; Escape closes it', async () => {
    client({ update: 'patch' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    const panel = dialog(w).element as HTMLElement;
    expect(document.activeElement?.textContent?.trim()).toBe('Install and restart');
    // A click on the notes' text leaves focus on the page: Tab goes back into the dialog.
    (document.activeElement as HTMLElement).blur();
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    expect(panel.contains(document.activeElement)).toBe(true);
    // Tab from the last stop wraps to the first: the scrollable release notes.
    (panel.querySelector('[data-primary]') as HTMLElement).focus();
    panel.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Release notes');
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await flushPromises();
    expect(dialog(w).exists()).toBe(false);
    w.unmount();
  });
});
