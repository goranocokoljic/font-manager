import { flushPromises, mount } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, shellNavigateHandlers, type MockOptions } from '../api/mock';
import type { InstallUpdateRequest } from '../api/types';
import { createAppRouter } from '../router';
import { useToastStore } from '../stores/toast';
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

  it('one control of three levels sets the two settings (E55.10)', async () => {
    const { c, check } = client();
    const w = await mountAt('/settings?section=about');
    const radios = () => w.findAll('input[name="update-level"]');
    const labels = () => radios().map((r) => r.element.parentElement!.textContent!.trim());
    expect(labels()).toEqual([
      'Off',
      'Tell me when there is an update',
      'Download updates and install them when I quit',
    ]);
    const checked = () => radios().findIndex((r) => (r.element as HTMLInputElement).checked);
    expect(checked()).toBe(0);
    const pick = async (i: number) => {
      await radios()[i]!.setValue(true);
      await flushPromises();
      const s = await c.getSettings();
      return [s.checkUpdatesAutomatically, s.installUpdatesOnQuit];
    };
    expect(await pick(2)).toEqual([true, true]);
    expect(checked()).toBe(2);
    expect(await pick(1)).toEqual([true, false]);
    expect(checked()).toBe(1);
    // Off turns both off, so the check turned on again never installs by surprise.
    await pick(2);
    expect(await pick(0)).toEqual([false, false]);
    expect(checked()).toBe(0);
    // The third choice names the two consequences, tied to it for screen readers.
    const third = radios()[2]!.element as HTMLInputElement;
    const notes = document.getElementById(third.getAttribute('aria-describedby')!)!;
    expect(notes.textContent).toContain('A major version always asks you first.');
    expect(notes.textContent).toContain('Fonts activated for the session end when you quit');
    expect(radios()[0]!.attributes('aria-describedby')).toBeUndefined();
    // Choosing a level asks the release server nothing by itself (the shell's timer does).
    expect(check).toHaveBeenCalledTimes(0);
    w.unmount();
  });

  it('under the control: the last check, and the waiting update with Install now (E55.10)', async () => {
    client({ updateWaiting: true });
    const w = await mountAt('/settings?section=about');
    expect(w.text()).toContain('version 0.1.1 was available');
    expect(w.text()).toContain('0.1.1 will be installed when you quit.');
    expect(dialog(w).exists()).toBe(false);
    await button(w, 'Install now')!.trigger('click');
    await flushPromises();
    expect(dialog(w).text()).toContain('Font Fu 0.1.1 is available');
    w.unmount();
  });

  it('says when no check ran yet, and what a failed check said', async () => {
    client({ update: 'offline' });
    const w = await mountAt('/settings?section=about');
    expect(w.text()).toContain('Font Fu has not checked for updates yet.');
    await checkNow(w);
    expect(w.text()).toContain('the check failed. Font Fu could not reach its release server.');
    w.unmount();
  });

  it('a Flatpak copy shows no level control', async () => {
    client({ update: 'managed' });
    const w = await mountAt('/settings?section=about');
    expect(w.findAll('input[name="update-level"]')).toHaveLength(0);
    w.unmount();
  });

  it('fm-api refuses "Install updates when I quit" where this copy does not update itself', async () => {
    const { c } = client({ update: 'managed' });
    await expect(c.updateSettings({ installUpdatesOnQuit: true })).rejects.toMatchObject({
      code: 'unsupported',
    });
    expect((await c.getSettings()).installUpdatesOnQuit).toBe(false);
    await c.updateSettings({ installUpdatesOnQuit: false });
    await expect(c.updateSettings({ updateLevel: 'installOnQuit' })).rejects.toMatchObject({
      code: 'unsupported',
    });
    await c.updateSettings({ updateLevel: 'notify' });
    expect((await c.getSettings()).checkUpdatesAutomatically).toBe(true);
  });

  it('the start after an install at quit says once "Updated to <version>" with a link', async () => {
    const { c } = client({
      updateNotice: {
        type: 'updated',
        version: '0.1.1',
        notesUrl: 'https://example.invalid/v0.1.1',
      },
    });
    const open = vi.spyOn(c, 'openUrl').mockResolvedValue();
    const w = await mountAt('/');
    const toasts = useToastStore();
    expect(toasts.toast?.message).toBe('Updated to 0.1.1.');
    expect(toasts.toast?.action?.label).toBe('What’s new');
    toasts.act();
    expect(open).toHaveBeenCalledWith('https://example.invalid/v0.1.1');
    w.unmount();
    toasts.clear();
    // Said once: the next start says nothing.
    await mountAt('/');
    expect(useToastStore().toast).toBeNull();
  });

  it('the start after a failed install says so once and offers the dialog', async () => {
    const { check } = client({
      update: 'patch',
      updateNotice: {
        type: 'notInstalled',
        version: '0.1.1',
        message:
          'Font Fu could not install version 0.1.1 when it quit, so version 0.1.0 started as before.',
      },
    });
    const w = await mountAt('/');
    const toasts = useToastStore();
    expect(toasts.toast?.message).toContain('could not install version 0.1.1');
    expect(dialog(w).exists()).toBe(false);
    expect(check).not.toHaveBeenCalled();
    toasts.act();
    await flushPromises();
    expect(check).toHaveBeenCalledTimes(1);
    expect(dialog(w).text()).toContain('Font Fu 0.1.1 is available');
    w.unmount();
  });

  it('a package refused while the app runs is reported once (updateNotice event)', async () => {
    const { c } = client();
    const take = vi.spyOn(c, 'takeUpdateNotice');
    const w = await mountAt('/');
    take.mockClear();
    useUpdatesStore().onEvent({
      type: 'updateNotice',
      notice: {
        type: 'notInstalled',
        version: '0.1.1',
        message: "The update was not installed: it is not signed with Font Fu's release key.",
      },
    });
    await flushPromises();
    expect(useToastStore().toast?.message).toContain('not signed');
    expect(take).toHaveBeenCalledTimes(1);
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

describe('An update found automatically is shown quietly (E55.8)', () => {
  const notice = (w: Wrapper) => w.find('[data-testid="update-notice"]');
  const dot = (w: Wrapper) => w.find('[data-testid="update-dot"]');

  /** The app with "Check automatically" on: the mock runs the shell's quiet check at start. */
  async function quietStart(update: MockOptions['update'] = 'patch') {
    const handles = client({ update, checkUpdatesAutomatically: true });
    const w = await mountAt('/');
    await new Promise((r) => setTimeout(r, 0));
    await flushPromises();
    return { ...handles, w };
  }

  it('shows a notice and a dot, never the dialog; asks nothing more', async () => {
    const { c, check, w } = await quietStart();
    const pending = vi.spyOn(c, 'pendingUpdate');
    expect(dialog(w).exists()).toBe(false);
    expect(notice(w).text()).toContain('Font Fu 0.1.1 is available.');
    expect(notice(w).text()).toContain("See what's new");
    expect(dot(w).text()).toBe("Font Fu 0.1.1 is available: see what's new");
    expect(check).not.toHaveBeenCalled();
    // The UI hears of finds by the event; it does not poll.
    await new Promise((r) => setTimeout(r, 50));
    expect(pending).not.toHaveBeenCalled();
    w.unmount();
  });

  it('a dismissed notice stays away until the next start; the dot and About stay', async () => {
    const { c, w } = await quietStart();
    await notice(w).get('button[aria-label="Dismiss"]').trigger('click');
    await flushPromises();
    expect(notice(w).exists()).toBe(false);
    expect(dot(w).exists()).toBe(true);
    // The shell's daily check finds the same release again: still no notice.
    useUpdatesStore().onEvent({ type: 'updateAvailable', update: (await c.pendingUpdate())! });
    await flushPromises();
    expect(notice(w).exists()).toBe(false);
    w.unmount();

    const about = await mountAt('/settings?section=about');
    expect(about.get('.updates [role="status"]').text()).toBe('Version 0.1.1 is available.');
    about.unmount();
    // The next start: a new app, the shell's find is on offer again, so is the notice.
    const next = await mountAt('/');
    expect(notice(next).exists()).toBe(true);
    next.unmount();
  });

  it('the notice opens the update dialog and goes; Later keeps the dot', async () => {
    const { w } = await quietStart();
    await button(w, "See what's new")!.trigger('click');
    await flushPromises();
    expect(dialog(w).text()).toContain('Font Fu 0.1.1 is available');
    expect(notice(w).exists()).toBe(false);
    await button(w, 'Later')!.trigger('click');
    await flushPromises();
    expect(dialog(w).exists()).toBe(false);
    expect(notice(w).exists()).toBe(false);
    expect(dot(w).exists()).toBe(true);
    w.unmount();
  });

  it('the dot opens the same dialog, where a major update asks twice', async () => {
    const { installs, w } = await quietStart('major');
    await dot(w).trigger('click');
    await flushPromises();
    expect(dialog(w).text()).toContain('Font Fu 1.0.0 is available');
    await button(w, 'Install and restart')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([]);
    expect(dialog(w).text()).toContain('Install a major update?');
    await button(w, 'Install version 1.0.0')!.trigger('click');
    await flushPromises();
    expect(installs).toEqual([{ version: '1.0.0', confirmMajor: true }]);
    w.unmount();
  });

  it('a check the user started opens the dialog at once and shows no notice', async () => {
    const { check } = client({ update: 'patch' });
    const w = await mountAt('/settings?section=about');
    await checkNow(w);
    expect(check).toHaveBeenCalledOnce();
    expect(dialog(w).exists()).toBe(true);
    await button(w, 'Later')!.trigger('click');
    await flushPromises();
    expect(notice(w).exists()).toBe(false);
    w.unmount();
  });

  it('no notice and no dot without an update', async () => {
    const { w } = await quietStart('none');
    expect(notice(w).exists()).toBe(false);
    expect(dot(w).exists()).toBe(false);
    w.unmount();
  });
});
