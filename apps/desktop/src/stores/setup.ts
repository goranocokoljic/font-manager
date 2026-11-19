import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Event, FamilyHit, SetupStep, Source } from '../api/types';
import { t } from '../i18n';
import { useSourcesStore } from './sources';

/**
 * The setup steps this build has a screen for, in the order the setup asks them. The
 * catalogs step (with the download folder, E55.5) and the updates step (E55.6) join this
 * list when they are built; until then an open step without a screen is not shown, so
 * the setup never opens for a question it cannot ask (ADR 0026: asked once, in order).
 */
export const SETUP_SCREENS: readonly SetupStep[] = ['fontFolders'];

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

  /** Reads the open steps once at start; the setup shows when one of them has a screen. */
  async function load(): Promise<void> {
    try {
      const status = await useClient().setupStatus();
      const shown = status.open.filter((s) => SETUP_SCREENS.includes(s));
      if (shown.length) await open(shown, false);
    } catch (e) {
      console.warn('the setup status could not be read', e);
    } finally {
      ready.value = true;
    }
  }

  /** "Run setup again" in Settings: every step with the current values. */
  async function runAgain(): Promise<void> {
    await open([...SETUP_SCREENS], true);
  }

  async function open(shown: SetupStep[], reopened: boolean): Promise<void> {
    steps.value = shown;
    again.value = reopened;
    index.value = 0;
    refused.value = [];
    removeErrors.value = {};
    error.value = null;
    visible.value = true;
    unsubscribe ??= useClient().onEvent(onEvent);
    await sources.load();
    await refreshFound();
  }

  /** Leaves the setup; the scans go on behind it. */
  function close(): void {
    visible.value = false;
    clearTimeout(foundTimer);
    const pending = unsubscribe;
    unsubscribe = undefined;
    void pending?.then((stop) => stop()).catch(() => undefined);
  }

  /** Continue (or Finish on the last step): only goes on, it answers nothing. */
  function next(): void {
    if (last.value) close();
    else index.value += 1;
  }

  function back(): void {
    if (index.value > 0) index.value -= 1;
  }

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
      next();
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
