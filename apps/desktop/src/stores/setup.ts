import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Catalog, Event, FamilyHit, SetupStep, Source } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useCatalogsStore } from './catalogs';
import { useSettingsStore } from './settings';
import { useSourcesStore } from './sources';
import { useUpdatesStore } from './updates';

/**
 * The setup screens this build has, in the order the setup asks them. An open step without a
 * screen is not shown, so the setup never opens for a question it cannot ask (ADR 0026: asked
 * once, in order). The updates screen (E55.6) is shown only where this copy updates itself:
 * fm-core opens that step only there, and "Run setup again" leaves it out elsewhere.
 */
export const SETUP_SCREENS: readonly SetupStep[] = ['fontFolders', 'catalogs', 'updates'];

/**
 * The screen a step is asked on: the download folder is the second half of the "Online
 * catalogs" screen (board 13 has three screens at most), so a catalog turned on outside the
 * setup brings that screen back until a folder is chosen (ADR 0026).
 */
export function screenOf(step: SetupStep): SetupStep {
  return step === 'downloadFolder' ? 'catalogs' : step;
}

/** The screens for the open steps, in order, each once; only those this build has. */
export function screensFor(open: readonly SetupStep[]): SetupStep[] {
  const shown: SetupStep[] = [];
  for (const step of open.map(screenOf))
    if (SETUP_SCREENS.includes(step) && !shown.includes(step)) shown.push(step);
  return shown;
}

/** Where downloaded fonts go (E55.5): inside a font folder of step one, or another folder. */
export type DownloadPlace = 'fontFolder' | 'other';

/** A folder the API refused: its row shows the API's message (E55.4). */
export interface RefusedFolder {
  key: number;
  path: string;
  message: string;
}

/** The families the right pane shows: the first ones found in the chosen folders. */
const FOUND_SHOWN = 6;
/** The pane re-reads the library at most this often while a scan reports progress. */
const FOUND_THROTTLE_MS = 500;

/**
 * The first-run setup (board 13, E55.4): shown in the main window in place of the library
 * while a step it can ask is open. The open steps come from `setupStatus` (decided in
 * fm-core); this store only keeps which one is on screen and the folders step's rows.
 */
export const useSetupStore = defineStore('setup', () => {
  const sources = useSourcesStore();
  const catalogs = useCatalogsStore();
  const settings = useSettingsStore();
  const updates = useUpdatesStore();
  /** The setup is on screen. */
  const visible = ref(false);
  /** The first `setupStatus` answer arrived (or failed): the main window may render. */
  const ready = ref(false);
  /** Reopened from Settings: every step, the step names are links, a close button. */
  const again = ref(false);
  /** The steps shown, in order; fixed while the setup is on screen. */
  const steps = ref<SetupStep[]>([]);
  const index = ref(0);
  /** Folders the API refused, as rows with its message. */
  const refused = ref<RefusedFolder[]>([]);
  /** A failed remove, by source: its row shows the API's message. */
  const removeErrors = ref<Record<number, string>>({});
  /** The first families found in the chosen folders, and how many there are. */
  const found = ref<{ families: FamilyHit[]; total: number }>({ families: [], total: 0 });
  /** The answer to a step (Skip) failed; shown under the actions. */
  const error = ref<string | null>(null);
  /** The catalogs screen: where downloaded fonts go. */
  const place = ref<DownloadPlace>('other');
  /** The font folder chosen with "Inside my font folder". */
  const placeSource = ref<number | null>(null);
  /** The folder of "Another folder": the proposal, until the user picks one. */
  const otherFolder = ref<string | null>(null);
  /** The API refused the download folder; the catalogs screen shows its message. */
  const folderError = ref<string | null>(null);
  /**
   * The updates screen's switch, "Check for updates automatically": on at a first start (the
   * owner's decision: Finish is the user action rule 7 asks for), the saved setting when the
   * setup is reopened. Nothing is saved and no request is made until Finish (E55.6).
   */
  const checkAutomatically = ref(true);
  /** The user chose on the catalogs screen; entering it again keeps that choice. */
  let placeChosen = false;
  let nextKey = 0;
  let unsubscribe: Promise<Unsubscribe> | undefined;
  let foundTimer: ReturnType<typeof setTimeout> | undefined;
  let foundRequest = 0;

  const current = computed<SetupStep | undefined>(() => steps.value[index.value]);
  const last = computed(() => index.value >= steps.value.length - 1);
  /** The folders the user chose (and the library folder); the system fonts are included anyway. */
  const folders = computed<Source[]>(() => sources.sources.filter((s) => s.kind !== 'system'));
  /** "Skip for now" is offered on the folders step only while no folder is added. */
  const canSkip = computed(() => current.value === 'fontFolders' && folders.value.length === 0);
  /** The catalogs the user turned on (the screen lists every catalog; Adobe Fonts is none). */
  const catalogsOn = computed<Catalog[]>(() => catalogs.catalogs.filter((c) => c.enabled));
  /** "Inside my font folder" is offered while step one has a folder. */
  const canPlaceInFontFolder = computed(() => folders.value.length > 0);
  /** The choice shown: "Inside my font folder" only while there is a font folder. */
  const placeShown = computed<DownloadPlace>(() =>
    place.value === 'fontFolder' && canPlaceInFontFolder.value ? 'fontFolder' : 'other',
  );
  /** The font folder of "Inside my font folder", while that is the choice. */
  const placeFolder = computed<Source | null>(() =>
    placeShown.value === 'fontFolder'
      ? (folders.value.find((s) => s.id === placeSource.value) ?? folders.value[0] ?? null)
      : null,
  );
  /**
   * The release server a check asks (`github.com`), as the API names it from the endpoint the
   * app was built with; `null` while unknown (the screen then says "the release server").
   */
  const releaseHost = computed<string | null>(() => updates.support?.releaseHost ?? null);
  /** The download folder the answer saves; `null` while there is none to save. */
  const downloadFolder = computed<string | null>(() =>
    placeShown.value === 'fontFolder' ? (placeFolder.value?.path ?? null) : otherFolder.value,
  );

  /** Reads the open steps once at start; the setup shows when one of them has a screen. */
  async function load(): Promise<void> {
    try {
      const status = await useClient().setupStatus();
      const shown = screensFor(status.open);
      if (shown.length) await open(shown, false);
    } catch (e) {
      console.warn('the setup status could not be read', e);
    } finally {
      ready.value = true;
    }
  }

  /**
   * "Run setup again" in Settings: every step with the current values. The updates step only
   * where this copy updates itself (`UpdateSupport.offered`); elsewhere it is not shown and the
   * indicator does not count it (E55.6).
   */
  async function runAgain(): Promise<void> {
    const support = updates.support ?? (await updates.loadSupport());
    await open(
      SETUP_SCREENS.filter((step) => step !== 'updates' || support?.offered === true),
      true,
    );
  }

  async function open(shown: SetupStep[], reopened: boolean): Promise<void> {
    steps.value = shown;
    again.value = reopened;
    index.value = 0;
    refused.value = [];
    removeErrors.value = {};
    error.value = null;
    folderError.value = null;
    placeChosen = false;
    otherFolder.value = null;
    visible.value = true;
    unsubscribe ??= useClient().onEvent(onEvent);
    await sources.load();
    await refreshFound();
    if (current.value === 'catalogs') await prepareCatalogs();
    if (shown.includes('updates')) await prepareUpdates(reopened);
  }

  /**
   * The updates screen starts on at a first start (fm-core opens the step only while the
   * automatic check is off) and from the saved setting when reopened. It reads what this copy
   * offers (`updateSupport`, no request to the release server) for the host it names.
   */
  async function prepareUpdates(reopened: boolean): Promise<void> {
    await Promise.all([
      updates.support ? undefined : updates.loadSupport(),
      !reopened || settings.settings ? undefined : settings.load(),
    ]);
    checkAutomatically.value = reopened
      ? settings.settings?.checkUpdatesAutomatically === true
      : true;
  }

  /**
   * Finish on the updates screen. On, it turns `checkUpdatesAutomatically` on: the shell then
   * runs the first check quietly and once a day after (E55.7, E55.8). Off, the setting stays
   * off (or is turned off, when the setup was reopened with it on) and "Check for updates" in
   * Settings → About works as before. Either way the step is answered and not asked again. A
   * setting the API refuses keeps the screen with its message.
   */
  async function answerUpdates(): Promise<boolean> {
    const on = checkAutomatically.value;
    if (!settings.settings) await settings.load();
    if (
      (settings.settings?.checkUpdatesAutomatically === true) !== on &&
      !(await settings.update({ checkUpdatesAutomatically: on }))
    ) {
      error.value = errorMessage(settings.error);
      return false;
    }
    try {
      await useClient().answerSetupStep({ step: 'updates', answer: 'answered' });
    } catch (e) {
      error.value = (e as ApiError).message;
      return false;
    }
    return true;
  }

  /**
   * The catalogs screen, each time it is entered, starts from what is saved: a download folder
   * that is one of the font folders is "Inside my font folder", any other is "Another folder";
   * with none, a font folder of step one is offered first and "Another folder" proposes
   * `<Documents>/Font Fu Fonts`, as the first Install does. A choice the user made on the
   * screen is kept when they go back and come again.
   */
  async function prepareCatalogs(): Promise<void> {
    await Promise.all([catalogs.load(), settings.settings ? undefined : settings.load()]);
    const saved = settings.settings?.downloadFolder ?? null;
    const inside = saved ? folders.value.find((s) => s.path === saved) : undefined;
    if (!placeChosen) {
      place.value = inside || (!saved && folders.value.length) ? 'fontFolder' : 'other';
      placeSource.value = (inside ?? folders.value[0])?.id ?? null;
    }
    if (!otherFolder.value && saved && !inside) otherFolder.value = saved;
    if (!otherFolder.value) {
      try {
        otherFolder.value = await useClient().suggestedDownloadFolder();
      } catch (e) {
        console.warn('no folder to propose for installed fonts', e);
      }
    }
  }

  /** A catalog's switch: on fetches its list now (the switch says so). */
  async function setCatalog(id: Catalog['id'], on: boolean): Promise<void> {
    await catalogs.setEnabled(id, on);
  }

  function choosePlace(next: DownloadPlace): void {
    place.value = next === 'fontFolder' && !canPlaceInFontFolder.value ? 'other' : next;
    placeChosen = true;
    folderError.value = null;
  }

  function chooseFontFolder(id: number): void {
    placeSource.value = id;
    choosePlace('fontFolder');
  }

  /** "Another folder": the native picker; a cancel keeps the folder shown. */
  async function pickOtherFolder(): Promise<void> {
    let path: string | null;
    try {
      path = await useClient().pickFolder(t('catalogs.install.folderTitle'));
    } catch (e) {
      folderError.value = errorMessage(e);
      return;
    }
    choosePlace('other');
    if (path) otherFolder.value = path;
  }

  /** A license link next to the cloud line: opened in the browser (a user action). */
  async function openLicense(catalog: Catalog): Promise<void> {
    if (!catalog.licenseUrl) return;
    try {
      await useClient().openUrl(catalog.licenseUrl);
    } catch (e) {
      error.value = errorMessage(e);
    }
  }

  /**
   * Continue on the catalogs screen. With no catalog on it records the step as answered and
   * asks nothing more: the first Install asks for the folder, as before. With one on it saves
   * the download folder first; a folder the API refuses keeps the screen with its message.
   */
  async function answerCatalogs(): Promise<boolean> {
    if (catalogsOn.value.length) {
      const folder = downloadFolder.value;
      if (!folder) {
        folderError.value = t('setup.catalogs.noFolder');
        return false;
      }
      if (
        settings.settings?.downloadFolder !== folder &&
        !(await settings.update({ downloadFolder: folder }))
      ) {
        folderError.value = errorMessage(settings.error);
        return false;
      }
    }
    folderError.value = null;
    try {
      await useClient().answerSetupStep({ step: 'catalogs', answer: 'answered' });
    } catch (e) {
      error.value = (e as ApiError).message;
      return false;
    }
    return true;
  }

  /** Leaves the setup; the scans go on behind it. */
  function close(): void {
    visible.value = false;
    clearTimeout(foundTimer);
    const pending = unsubscribe;
    unsubscribe = undefined;
    void pending?.then((stop) => stop()).catch(() => undefined);
  }

  /**
   * Continue (or Finish on the last step). The folders step only goes on (its folders are
   * added as they are chosen); the catalogs and updates steps save their answer first and stay
   * when the API refuses it.
   */
  async function next(): Promise<void> {
    if (current.value === 'catalogs' && !(await answerCatalogs())) return;
    if (current.value === 'updates' && !(await answerUpdates())) return;
    error.value = null;
    advance();
  }

  function advance(): void {
    if (last.value) close();
    else index.value += 1;
  }

  function back(): void {
    if (index.value > 0) index.value -= 1;
  }

  // Entering the catalogs screen (Continue, Back, a step link) reads what it starts from:
  // step one may have added the folder it offers.
  watch(current, (step, before) => {
    if (visible.value && step === 'catalogs' && before !== undefined) void prepareCatalogs();
  });

  /** A step name in the indicator, reopened from Settings. */
  function goTo(i: number): void {
    if (again.value && i >= 0 && i < steps.value.length) index.value = i;
  }

  /** "Skip for now": records the step as skipped and goes on. */
  async function skip(): Promise<void> {
    const step = current.value;
    if (!step) return;
    try {
      await useClient().answerSetupStep({ step, answer: 'skipped' });
      error.value = null;
      advance();
    } catch (e) {
      error.value = (e as ApiError).message;
    }
  }

  /** "Add folder…": the native picker; the folder is added at once and its scan starts. */
  async function addFolder(): Promise<void> {
    let path: string | null;
    try {
      path = await useClient().pickFolder(t('setup.folders.pickerTitle'));
    } catch (e) {
      error.value = (e as ApiError).message;
      return;
    }
    if (!path) return;
    refused.value = refused.value.filter((r) => r.path !== path);
    try {
      await sources.addPath(path);
      await refreshFound();
    } catch (e) {
      refused.value = [
        ...refused.value,
        { key: ++nextKey, path, message: (e as ApiError).message },
      ];
    }
  }

  /** Takes a folder out of the library again; no file is touched. */
  async function removeFolder(id: number): Promise<void> {
    try {
      await sources.removeOrThrow(id);
      const rest = { ...removeErrors.value };
      delete rest[id];
      removeErrors.value = rest;
      await refreshFound();
    } catch (e) {
      removeErrors.value = { ...removeErrors.value, [id]: (e as ApiError).message };
    }
  }

  /** Drops a refused folder's row. */
  function dismiss(key: number): void {
    refused.value = refused.value.filter((r) => r.key !== key);
  }

  /** The first families found in the chosen folders, oldest first, and their number. */
  async function refreshFound(): Promise<void> {
    const ids = folders.value.map((s) => s.id);
    const request = ++foundRequest;
    if (!ids.length) {
      found.value = { families: [], total: 0 };
      return;
    }
    try {
      const res = await useClient().search({
        filters: { sources: ids },
        order: { by: 'dateAdded', descending: false },
        page: { offset: 0, limit: FOUND_SHOWN },
      });
      // A later request (another folder, another scan step) wins.
      if (request === foundRequest) found.value = { families: res.families, total: res.total };
    } catch (e) {
      console.warn('the families found so far could not be read', e);
    }
  }

  function refreshFoundSoon(): void {
    if (foundTimer !== undefined) return;
    foundTimer = setTimeout(() => {
      foundTimer = undefined;
      void refreshFound();
    }, FOUND_THROTTLE_MS);
  }

  function onEvent(event: Event): void {
    if (!visible.value) return;
    if (event.type === 'scanFinished' || event.type === 'libraryChanged') void refreshFound();
    else if (event.type === 'scanProgress') refreshFoundSoon();
  }

  return {
    visible,
    ready,
    again,
    steps,
    index,
    current,
    last,
    folders,
    canSkip,
    catalogsOn,
    canPlaceInFontFolder,
    place: placeShown,
    placeFolder,
    otherFolder,
    downloadFolder,
    folderError,
    checkAutomatically,
    releaseHost,
    setCatalog,
    choosePlace,
    chooseFontFolder,
    pickOtherFolder,
    openLicense,
    refused,
    removeErrors,
    found,
    error,
    load,
    runAgain,
    close,
    next,
    back,
    goTo,
    skip,
    addFolder,
    removeFolder,
    dismiss,
  };
});
