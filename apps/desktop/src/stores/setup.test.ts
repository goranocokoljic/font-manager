import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import {
  MOCK_GONE_FOLDER,
  MOCK_SUGGESTED_DOWNLOAD_FOLDER,
  createMockClient,
  mockPickedFolders,
  type MockOptions,
} from '../api/mock';
import type { SetupStep } from '../api/types';
import { installPath, useCatalogsStore } from './catalogs';
import { screensFor, useSetupStore } from './setup';
import { useSourcesStore } from './sources';
import { useUpdatesStore } from './updates';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

function start(options: MockOptions = { setup: 'first' }): ReturnType<typeof useSetupStore> {
  setActivePinia(createPinia());
  client = createMockClient({ scanStepMs: 10, ...options });
  setClient(client);
  return useSetupStore();
}

beforeEach(() => vi.useFakeTimers());

afterEach(async () => {
  useSetupStore().close();
  await useSourcesStore().stop();
  setClient(undefined);
  mockPickedFolders.length = 0;
  vi.useRealTimers();
});

describe('setup store (E55.4)', () => {
  it('shows at a first start, with only the open steps it has a screen for', async () => {
    const setup = start();
    await setup.load();
    expect(setup.ready).toBe(true);
    expect(setup.visible).toBe(true);
    expect((await client.setupStatus()).open).toEqual(['fontFolders', 'catalogs', 'updates']);
    expect(setup.steps).toEqual(['fontFolders', 'catalogs', 'updates']);
    expect(setup.current).toBe('fontFolders');
  });

  it('never shows when no step is open', async () => {
    const setup = start({ setup: 'none' });
    await setup.load();
    expect(setup.ready).toBe(true);
    expect(setup.visible).toBe(false);
  });

  it('shows only the open screens when a watched folder closes the folders step', async () => {
    // The sample library has a watched folder: only catalogs and updates are open.
    const setup = start({});
    await setup.load();
    expect((await client.setupStatus()).open).not.toContain('fontFolders' as SetupStep);
    expect(setup.steps).toEqual(['catalogs', 'updates']);
    // Both answered, nothing is open: it does not show again.
    await setup.next();
    await setup.next();
    expect(setup.visible).toBe(false);
    const later = start({});
    await client.answerSetupStep({ step: 'catalogs', answer: 'answered' });
    await client.answerSetupStep({ step: 'updates', answer: 'answered' });
    await later.load();
    expect(later.visible).toBe(false);
  });

  it('lets the main window render when the status cannot be read', async () => {
    const setup = start();
    vi.spyOn(client, 'setupStatus').mockRejectedValue({ code: 'internal', message: 'x' });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    await setup.load();
    expect(setup.ready).toBe(true);
    expect(setup.visible).toBe(false);
  });

  it('adds each chosen folder at once, indexed in place, and starts its scan', async () => {
    const setup = start();
    const addNow = vi.spyOn(client, 'addSource');
    const scanNow = vi.spyOn(client, 'scan');
    await setup.load();
    mockPickedFolders.push('/Users/demo/Design fonts', '/Users/demo/Client fonts');
    await setup.addFolder();
    await setup.addFolder();
    expect(addNow.mock.calls.map((c) => c[0])).toEqual([
      { path: '/Users/demo/Design fonts', mode: { kind: 'indexInPlace' } },
      { path: '/Users/demo/Client fonts', mode: { kind: 'indexInPlace' } },
    ]);
    expect(setup.folders.map((s) => s.path)).toEqual([
      '/Users/demo/Design fonts',
      '/Users/demo/Client fonts',
    ]);
    expect(scanNow.mock.calls.map((c) => c[0].source)).toEqual(setup.folders.map((s) => s.id));
    // With a folder the step closes by itself, so Skip is gone.
    expect(setup.canSkip).toBe(false);
  });

  it('a cancelled picker adds nothing', async () => {
    const setup = start();
    await setup.load();
    const add = vi.spyOn(client, 'addSource');
    mockPickedFolders.push(null);
    await setup.addFolder();
    expect(add).not.toHaveBeenCalled();
    expect(setup.folders).toEqual([]);
    expect(setup.refused).toEqual([]);
  });

  it("keeps a refused folder as a row with the API's message and goes on with the others", async () => {
    const setup = start();
    await setup.load();
    mockPickedFolders.push(
      '/Users/demo/Fonts',
      '/Users/demo/Fonts',
      '/Users/demo/Fonts/Client',
      `${MOCK_GONE_FOLDER}/Archive`,
      '/Users/demo/Other',
    );
    for (let i = 0; i < 5; i++) await setup.addFolder();
    expect(setup.folders.map((s) => s.path)).toEqual(['/Users/demo/Fonts', '/Users/demo/Other']);
    expect(setup.refused.map((r) => [r.path, r.message])).toEqual([
      ['/Users/demo/Fonts', 'This folder is already in the library.'],
      [
        '/Users/demo/Fonts/Client',
        'This folder is inside /Users/demo/Fonts, which is already in the library.',
      ],
      [`${MOCK_GONE_FOLDER}/Archive`, `The folder ${MOCK_GONE_FOLDER}/Archive was not found.`],
    ]);
    setup.dismiss(setup.refused[0]!.key);
    expect(setup.refused).toHaveLength(2);
  });

  it('remove takes the folder out of the library and Skip comes back', async () => {
    const setup = start();
    await setup.load();
    mockPickedFolders.push('/Users/demo/Design fonts');
    await setup.addFolder();
    const id = setup.folders[0]!.id;
    const remove = vi.spyOn(client, 'removeSource');
    await setup.removeFolder(id);
    expect(remove).toHaveBeenCalledWith(id);
    expect(setup.folders).toEqual([]);
    expect((await client.listSources()).some((s) => s.id === id)).toBe(false);
    expect(setup.canSkip).toBe(true);
  });

  it("shows a refused remove's message on its row", async () => {
    const setup = start();
    await setup.load();
    mockPickedFolders.push('/Users/demo/Design fonts');
    await setup.addFolder();
    const id = setup.folders[0]!.id;
    vi.spyOn(client, 'removeSource').mockRejectedValue({
      code: 'conflict',
      message: 'Deactivate the fonts of this folder before removing it.',
    });
    await setup.removeFolder(id);
    expect(setup.folders.map((s) => s.id)).toEqual([id]);
    expect(setup.removeErrors[id]).toBe('Deactivate the fonts of this folder before removing it.');
  });

  it('Skip records the step as skipped and goes on; Continue answers nothing', async () => {
    const setup = start();
    await setup.load();
    const answer = vi.spyOn(client, 'answerSetupStep');
    expect(setup.canSkip).toBe(true);
    await setup.skip();
    expect(answer).toHaveBeenCalledWith({ step: 'fontFolders', answer: 'skipped' });
    expect(setup.current).toBe('catalogs');
    expect((await client.setupStatus()).open).not.toContain('fontFolders' as SetupStep);

    const again = start();
    await again.load();
    const answer2 = vi.spyOn(client, 'answerSetupStep');
    mockPickedFolders.push('/Users/demo/Design fonts');
    await again.addFolder();
    await again.next();
    expect(answer2, 'the folders step answers nothing').not.toHaveBeenCalled();
    expect(again.current).toBe('catalogs');
  });

  it('a failed Skip stays on the step and says why', async () => {
    const setup = start();
    await setup.load();
    vi.spyOn(client, 'answerSetupStep').mockRejectedValue({
      code: 'io',
      message: 'config.json could not be written.',
    });
    await setup.skip();
    expect(setup.visible).toBe(true);
    expect(setup.error).toBe('config.json could not be written.');
  });

  it('Run setup again opens every step with the current folders and changes nothing on its own', async () => {
    const setup = start({});
    await client.answerSetupStep({ step: 'catalogs', answer: 'skipped' });
    await client.answerSetupStep({ step: 'updates', answer: 'skipped' });
    await setup.load();
    expect(setup.visible).toBe(false);
    const add = vi.spyOn(client, 'addSource');
    const remove = vi.spyOn(client, 'removeSource');
    const answer = vi.spyOn(client, 'answerSetupStep');
    await setup.runAgain();
    expect(setup.visible).toBe(true);
    expect(setup.again).toBe(true);
    expect(setup.steps).toEqual(['fontFolders', 'catalogs', 'updates']);
    expect(setup.folders.map((s) => s.path)).toEqual(['/Users/demo/Fonts']);
    await setup.next();
    expect(setup.current).toBe('catalogs');
    expect(add).not.toHaveBeenCalled();
    expect(remove).not.toHaveBeenCalled();
    expect(answer).not.toHaveBeenCalled();
  });

  it('goTo moves between steps only when reopened from Settings', async () => {
    const setup = start();
    await setup.load();
    setup.steps = ['fontFolders', 'catalogs'];
    setup.goTo(1);
    expect(setup.index).toBe(0);
    setup.again = true;
    setup.goTo(1);
    expect(setup.index).toBe(1);
    setup.back();
    expect(setup.index).toBe(0);
  });

  it('the pane shows the first families found in the chosen folders and their number', async () => {
    const setup = start();
    await setup.load();
    expect(setup.found.total).toBe(0);
    const search = vi.spyOn(client, 'search');
    mockPickedFolders.push('/Users/demo/Design fonts');
    await setup.addFolder();
    const id = setup.folders[0]!.id;
    expect(search).toHaveBeenLastCalledWith({
      filters: { sources: [id] },
      order: { by: 'dateAdded', descending: false },
      page: { offset: 0, limit: 6 },
    });
    expect(setup.found.total).toBeGreaterThan(0);
    expect(setup.found.families.length).toBeLessThanOrEqual(6);
    // The scan's events re-read it.
    search.mockClear();
    await vi.advanceTimersByTimeAsync(1000);
    expect(search).toHaveBeenCalled();
  });
});

describe('setup store: the catalogs screen (E55.5)', () => {
  /** A first start with `paths` added in step one, then on to the catalogs screen. */
  async function onCatalogs(...paths: string[]): Promise<ReturnType<typeof useSetupStore>> {
    const setup = start();
    await setup.load();
    mockPickedFolders.push(...paths);
    for (let i = 0; i < paths.length; i++) await setup.addFolder();
    if (paths.length) await setup.next();
    else await setup.skip();
    await vi.advanceTimersByTimeAsync(0);
    expect(setup.current).toBe('catalogs');
    return setup;
  }

  it('maps the download folder step onto the catalogs screen, once', () => {
    expect(screensFor(['downloadFolder', 'updates'])).toEqual(['catalogs', 'updates']);
    expect(screensFor(['fontFolders', 'catalogs', 'downloadFolder'])).toEqual([
      'fontFolders',
      'catalogs',
    ]);
  });

  it('with no catalog on, Continue answers the step and saves no folder; Install still asks', async () => {
    const setup = await onCatalogs('/Users/demo/Design fonts');
    const catalogs = useCatalogsStore();
    expect(catalogs.catalogs.map((c) => [c.id, c.enabled])).toEqual([
      ['googleFonts', false],
      ['fontshare', false],
    ]);
    expect(setup.catalogsOn).toEqual([]);
    const answer = vi.spyOn(client, 'answerSetupStep');
    const update = vi.spyOn(client, 'updateSettings');
    await setup.next();
    expect(answer).toHaveBeenCalledWith({ step: 'catalogs', answer: 'answered' });
    expect(update).not.toHaveBeenCalled();
    expect(setup.current).toBe('updates');
    expect((await client.getSettings()).downloadFolder ?? null).toBeNull();
    expect((await client.setupStatus()).open).not.toContain('catalogs' as SetupStep);
    // A catalog turned on later: the first Install proposes the folder, as before.
    await catalogs.setEnabled('googleFonts', true);
    catalogs.show('googleFonts');
    await vi.advanceTimersByTimeAsync(0);
    await catalogs.askInstall(catalogs.families[0]!);
    expect(catalogs.install?.phase).toBe('proposing');
    expect(catalogs.install?.proposed).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
  });

  it('turning a catalog on fetches its list now', async () => {
    const setup = await onCatalogs('/Users/demo/Design fonts');
    const refresh = vi.spyOn(client, 'refreshCatalog');
    await setup.setCatalog('fontshare', true);
    expect(refresh).toHaveBeenCalledWith({ id: 'fontshare' });
    expect(setup.catalogsOn.map((c) => c.id)).toEqual(['fontshare']);
  });

  it('"Inside my font folder" saves the chosen font folder; Install is then one click', async () => {
    const setup = await onCatalogs('/Users/demo/Design fonts', '/Users/demo/Client fonts');
    await setup.setCatalog('googleFonts', true);
    expect(setup.canPlaceInFontFolder).toBe(true);
    expect(setup.place).toBe('fontFolder');
    expect(setup.downloadFolder).toBe('/Users/demo/Design fonts');
    setup.chooseFontFolder(setup.folders[1]!.id);
    expect(setup.downloadFolder).toBe('/Users/demo/Client fonts');
    const catalog = useCatalogsStore().catalogs[0]!;
    expect(installPath(setup.downloadFolder!, catalog, '<Family>')).toBe(
      '/Users/demo/Client fonts/Google Fonts/<Family>',
    );
    await setup.next();
    expect(setup.current).toBe('updates');
    expect((await client.getSettings()).downloadFolder).toBe('/Users/demo/Client fonts');
    expect((await client.setupStatus()).open).toEqual(['updates']);

    const catalogs = useCatalogsStore();
    catalogs.show('googleFonts');
    await vi.advanceTimersByTimeAsync(0);
    const install = vi.spyOn(client, 'installFamily');
    const suggest = vi.spyOn(client, 'suggestedDownloadFolder');
    const run = catalogs.askInstall(catalogs.families[0]!);
    await vi.advanceTimersByTimeAsync(5000);
    await run;
    expect(install).toHaveBeenCalledTimes(1);
    expect(suggest, 'no folder question').not.toHaveBeenCalled();
  });

  it('without a font folder only "Another folder" is offered, with the proposal; a pick replaces it', async () => {
    const setup = await onCatalogs();
    await setup.setCatalog('fontshare', true);
    expect(setup.canPlaceInFontFolder).toBe(false);
    expect(setup.place).toBe('other');
    expect(setup.downloadFolder).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    // A cancelled picker keeps the proposal.
    mockPickedFolders.push(null, '/Users/demo/Downloaded type');
    await setup.pickOtherFolder();
    expect(setup.downloadFolder).toBe(MOCK_SUGGESTED_DOWNLOAD_FOLDER);
    await setup.pickOtherFolder();
    expect(setup.downloadFolder).toBe('/Users/demo/Downloaded type');
    await setup.next();
    expect((await client.getSettings()).downloadFolder).toBe('/Users/demo/Downloaded type');
  });

  it('a system font folder is refused with the message of the API and the screen stays', async () => {
    const setup = await onCatalogs('/Users/demo/Design fonts');
    await setup.setCatalog('googleFonts', true);
    setup.choosePlace('other');
    mockPickedFolders.push('/System/Library/Fonts/Mine');
    await setup.pickOtherFolder();
    const answer = vi.spyOn(client, 'answerSetupStep');
    await setup.next();
    expect(setup.visible).toBe(true);
    expect(setup.current).toBe('catalogs');
    expect(setup.folderError).toBe(
      'Fonts cannot be installed into a system font folder. Choose another folder.',
    );
    expect(answer).not.toHaveBeenCalled();
    expect((await client.getSettings()).downloadFolder ?? null).toBeNull();
    // Another choice clears the message and goes on.
    setup.choosePlace('fontFolder');
    expect(setup.folderError).toBeNull();
    await setup.next();
    expect(setup.current).toBe('updates');
  });

  it('marks a font folder inside a cloud sync folder, and only that one', async () => {
    const setup = await onCatalogs('/Users/demo/Fonts here', '/Users/demo/Dropbox/Studio fonts');
    await setup.setCatalog('googleFonts', true);
    expect(setup.placeFolder?.cloud).toBe(false);
    setup.chooseFontFolder(setup.folders[1]!.id);
    expect(setup.placeFolder?.cloud).toBe(true);
    expect(setup.catalogsOn[0]!.licenseUrl).toBe('https://fonts.google.com/faq');
    const open = vi.spyOn(client, 'openUrl').mockResolvedValue();
    await setup.openLicense(setup.catalogsOn[0]!);
    expect(open).toHaveBeenCalledWith('https://fonts.google.com/faq');
    // "Another folder" is not a font folder: no cloud line.
    setup.choosePlace('other');
    expect(setup.placeFolder).toBeNull();
  });

  it('starts from the saved folder when run again', async () => {
    const setup = start({});
    await client.setCatalogEnabled({ id: 'googleFonts', enabled: true });
    await client.updateSettings({ downloadFolder: '/Users/demo/Fonts' });
    await setup.runAgain();
    setup.goTo(1);
    await vi.advanceTimersByTimeAsync(0);
    expect(setup.current).toBe('catalogs');
    expect(setup.place).toBe('fontFolder');
    expect(setup.downloadFolder).toBe('/Users/demo/Fonts');
  });
});

describe('setup store: the updates screen (E55.6)', () => {
  /** The requests to the release server the mock saw (`manual` or `automatic`). */
  const checks = () =>
    (globalThis as { __fmMockUpdateChecks?: string[] }).__fmMockUpdateChecks ?? [];

  beforeEach(() => {
    delete (globalThis as { __fmMockUpdateChecks?: string[] }).__fmMockUpdateChecks;
  });

  /** A first start, step one skipped and step two answered: on the updates screen. */
  async function onUpdates(options: MockOptions = { setup: 'first' }) {
    const setup = start(options);
    await setup.load();
    await setup.skip();
    await vi.advanceTimersByTimeAsync(0);
    await setup.next();
    expect(setup.current).toBe('updates');
    expect(setup.last).toBe(true);
    return setup;
  }

  it('is preselected on, names the host the API reports, and asks the server nothing', async () => {
    const setup = await onUpdates({ setup: 'first', update: 'patch' });
    const manual = vi.spyOn(client, 'checkForUpdate');
    expect(setup.checkAutomatically).toBe(true);
    expect(setup.releaseHost).toBe('github.com');
    await vi.advanceTimersByTimeAsync(1000);
    expect(manual).not.toHaveBeenCalled();
    expect(checks(), 'no request before Finish').toEqual([]);
    expect((await client.getSettings()).checkUpdatesAutomatically).toBe(false);
  });

  it('Finish with the switch on turns the automatic check on; its first check runs quietly', async () => {
    const setup = await onUpdates({ setup: 'first', update: 'patch' });
    const updates = useUpdatesStore();
    await updates.start();
    const answer = vi.spyOn(client, 'answerSetupStep');
    await setup.next();
    expect(setup.visible).toBe(false);
    expect((await client.getSettings()).checkUpdatesAutomatically).toBe(true);
    expect(answer).toHaveBeenCalledWith({ step: 'updates', answer: 'answered' });
    await vi.advanceTimersByTimeAsync(0);
    expect(checks()).toEqual(['automatic']);
    // Found quietly: offered, no dialog opens.
    expect(updates.status).toBe('available');
    expect(updates.open).toBe(false);
    expect((await client.setupStatus()).open).toEqual([]);
    await updates.stop();
  });

  it('Finish with the switch off keeps the setting off; Check for updates works as before', async () => {
    const setup = await onUpdates({ setup: 'first', update: 'patch' });
    const update = vi.spyOn(client, 'updateSettings');
    setup.checkAutomatically = false;
    await setup.next();
    expect(setup.visible).toBe(false);
    expect(update).not.toHaveBeenCalled();
    expect((await client.getSettings()).checkUpdatesAutomatically).toBe(false);
    expect((await client.setupStatus()).open, 'not asked again').toEqual([]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(checks()).toEqual([]);
    const updates = useUpdatesStore();
    await updates.check();
    expect(checks()).toEqual(['manual']);
    expect(updates.status).toBe('available');
  });

  it('a refused setting keeps the screen with its message and answers nothing', async () => {
    const setup = await onUpdates();
    vi.spyOn(client, 'updateSettings').mockRejectedValue({
      code: 'io',
      message: 'config.json could not be written.',
      detail: null,
    });
    const answer = vi.spyOn(client, 'answerSetupStep');
    await setup.next();
    expect(setup.visible).toBe(true);
    expect(setup.current).toBe('updates');
    expect(setup.error).toBe('config.json could not be written.');
    expect(answer).not.toHaveBeenCalled();
  });

  it('is not shown, and not counted, where this copy does not update itself', async () => {
    const setup = start({ setup: 'first', update: 'managed' });
    await setup.load();
    expect(setup.steps).toEqual(['fontFolders', 'catalogs']);
    setup.close();
    await setup.runAgain();
    expect(setup.steps).toEqual(['fontFolders', 'catalogs']);
    expect(checks()).toEqual([]);
  });

  it('reopened from Settings, the switch starts from the saved setting', async () => {
    const setup = start({});
    await client.updateSettings({ checkUpdatesAutomatically: true });
    await vi.advanceTimersByTimeAsync(0);
    await setup.runAgain();
    expect(setup.steps).toEqual(['fontFolders', 'catalogs', 'updates']);
    expect(setup.checkAutomatically).toBe(true);
    setup.goTo(2);
    setup.checkAutomatically = false;
    await setup.next();
    expect((await client.getSettings()).checkUpdatesAutomatically).toBe(false);

    setup.close();
    await setup.runAgain();
    expect(setup.checkAutomatically).toBe(false);
  });
});

describe('installPath', () => {
  const catalog = {
    id: 'fontshare',
    name: 'Fontshare',
    homepage: '',
    enabled: true,
    familyCount: 0,
    folderName: 'Fontshare',
  } as const;
  it('joins with the separator the folder uses, once', () => {
    expect(installPath('C:\\Users\\me\\Type\\', catalog, 'Satoshi')).toBe(
      'C:\\Users\\me\\Type\\Fontshare\\Satoshi',
    );
    expect(installPath('/home/me/Type', catalog, 'Satoshi')).toBe(
      '/home/me/Type/Fontshare/Satoshi',
    );
  });
});
