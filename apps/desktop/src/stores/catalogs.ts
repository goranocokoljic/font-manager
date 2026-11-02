import { defineStore } from 'pinia';
import { computed, ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type {
  ApiError,
  Catalog,
  CatalogFamily,
  CatalogFileKind,
  CatalogId,
  CatalogUpdate,
  DownloadFamilyResponse,
  Event,
  FaceId,
  InstallFamilyResponse,
  JobId,
  SourceId,
} from '../api/types';
import { t } from '../i18n';
import { useQuickCollectionStore } from './quickCollection';
import { useSearchStore } from './search';
import { useSettingsStore } from './settings';

/**
 * The entries of the sidebar's "Online catalogs" group: each catalog of `list_catalogs`
 * (E22.2), and Adobe Fonts, a note only (E22.6).
 */
export type CatalogEntry = CatalogId | 'adobeFonts';

/** The preview-settings and view-mode id the catalog browser uses. */
export const CATALOG_VIEW = 'catalog';

/** Families one `list_catalog_families` call reads (the API caps a page at 500). */
export const CATALOG_PAGE = 200;

/**
 * Where a "Download..." of a catalog family is: `choosing` the source, `downloading` (the
 * core fetches and writes the files), `scanning` (the source's rescan brings the faces in),
 * `done`, or `failed` (nothing was written; the user may try again).
 */
export type DownloadPhase = 'choosing' | 'downloading' | 'scanning' | 'done' | 'failed';

/** The open "Download..." panel of the catalog browser (E22.3). */
export interface CatalogDownloadState {
  catalog: CatalogId;
  family: CatalogFamily;
  phase: DownloadPhase;
  /** The rescan's progress, from its `scanProgress` events. */
  scan: { done: number; total: number } | null;
  /** What the download wrote; set from `scanning` on. */
  result: DownloadFamilyResponse | null;
  /** Why the download (or its rescan) failed. */
  error: ApiError | null;
}

/**
 * Where an Install of a catalog family is (E22.7): `proposing` the download folder (the first
 * Install, while none is set), `downloading` (the core fetches and writes the files),
 * `scanning` (the rescan of the folder brings the faces in), `done` (the faces are active)
 * or `failed`.
 */
export type InstallPhase = 'proposing' | 'downloading' | 'scanning' | 'done' | 'failed';

/** The Install of the catalog browser: one at a time, shown in its progress line. */
export interface CatalogInstallState {
  catalog: CatalogId;
  family: CatalogFamily;
  phase: InstallPhase;
  /** The folder the first Install proposes; set while `proposing`. */
  proposed: string | null;
  /** What the install did; set when `done`. */
  result: InstallFamilyResponse | null;
  /** Why the install failed. */
  error: ApiError | null;
}

/**
 * The faces of an install that are active now: those activated, those that already were
 * (no outcome), not those that clashed, failed or were skipped.
 */
export function installedActive(result: InstallFamilyResponse): number {
  const outcome = new Map<FaceId, string>(result.outcomes.map((o) => [o.face, o.outcome.status]));
  return result.faces.filter((f) => {
    const status = outcome.get(f);
    return status === undefined || status === 'done' || status === 'unchanged';
  }).length;
}

function toApiError(e: unknown): ApiError {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) return e as ApiError;
  return { code: 'internal', message: String(e), detail: null };
}

/**
 * The online catalogs: which are on and when their list was refreshed, which one the
 * center shows instead of the font list, and that catalog's families for the search
 * text. The families are read page by page as the list scrolls; the core filters them
 * (`list_catalog_families`). Only `refresh`, `checkUpdates` and turning a never-refreshed
 * catalog on go to the network, each a user action. The downloaded families with an update
 * (E22.4) are known only after the user's "Check for updates" or a Refresh; nothing checks on
 * start or on a timer.
 */
export const useCatalogsStore = defineStore('catalogs', () => {
  const client = () => useClient();
  const quick = useQuickCollectionStore();

  const catalogs = ref<Catalog[]>([]);
  const open = ref<CatalogEntry | null>(null);
  const query = ref('');
  /** Families matching `query`, by index; a page arrives when the list reaches it. */
  const families = shallowRef<(CatalogFamily | undefined)[]>([]);
  const total = ref(0);
  const listed = ref(false);
  const busy = ref<CatalogId | null>(null);
  const refreshing = ref(false);
  /** The last failed refresh of the open catalog; its cached list stays. */
  const refreshError = ref<ApiError | null>(null);
  /** The last failure of anything else (the list, a switch). */
  const error = ref<ApiError | null>(null);
  /** The webview's own idea of the connection; `false` shows the offline note. */
  const online = ref(globalThis.navigator?.onLine ?? true);
  /** The "Download..." panel; `null` when it is closed. */
  const download = ref<CatalogDownloadState | null>(null);
  /** The Install progress line; `null` when there is none. */
  const install = ref<CatalogInstallState | null>(null);
  /** What the last update check found; `null` before the first one. */
  const updates = ref<CatalogUpdate[] | null>(null);
  const checking = ref(false);
  /** Why the last "Check for updates" failed. */
  const checkError = ref<ApiError | null>(null);
  /** Bumped by every new list, so pages of an older one are dropped. */
  let generation = 0;
  const pending = new Set<number>();

  if (typeof window !== 'undefined') {
    window.addEventListener('online', () => (online.value = true));
    window.addEventListener('offline', () => (online.value = false));
  }

  const current = computed<Catalog | null>(
    () => catalogs.value.find((c) => c.id === open.value) ?? null,
  );
  /** The cached list is shown though it could not be brought up to date. */
  const offline = computed(
    () => !!current.value?.refreshedAt && (!online.value || refreshError.value?.code === 'io'),
  );

  function put(catalog: Catalog): void {
    const i = catalogs.value.findIndex((c) => c.id === catalog.id);
    catalogs.value =
      i < 0 ? [...catalogs.value, catalog] : catalogs.value.map((c, j) => (j === i ? catalog : c));
  }

  async function load(): Promise<void> {
    try {
      catalogs.value = await client().listCatalogs();
      error.value = null;
      // A catalog opened before the list arrived reads its families now.
      if (current.value && !listed.value) await relist();
    } catch (e) {
      error.value = toApiError(e);
    }
  }

  /** Reads the page holding family `index` of the open catalog. */
  async function readPage(index: number): Promise<void> {
    const id = current.value?.id;
    const page = Math.floor(index / CATALOG_PAGE);
    if (!id || pending.has(page)) return;
    pending.add(page);
    const gen = generation;
    try {
      const got = await client().listCatalogFamilies({
        id,
        query: query.value,
        page: { offset: page * CATALOG_PAGE, limit: CATALOG_PAGE },
      });
      if (gen !== generation) return;
      const next = families.value.slice();
      next.length = got.total;
      got.items.forEach((f, i) => (next[page * CATALOG_PAGE + i] = f));
      families.value = next;
      total.value = got.total;
      listed.value = true;
      error.value = null;
    } catch (e) {
      if (gen === generation) error.value = toApiError(e);
    } finally {
      if (gen === generation) pending.delete(page);
    }
  }

  /** Starts the open catalog's list again from its first page. */
  async function relist(): Promise<void> {
    generation += 1;
    pending.clear();
    families.value = [];
    total.value = 0;
    listed.value = false;
    if (current.value?.enabled) await readPage(0);
  }

  /** Makes sure the families from `start` to `end` (exclusive) are read. */
  function want(start: number, end: number): void {
    for (let i = start; i < end; i += CATALOG_PAGE) if (!families.value[i]) void readPage(i);
    if (end > start && !families.value[end - 1]) void readPage(end - 1);
  }

  function show(entry: CatalogEntry): void {
    quick.close();
    if (open.value === entry) return;
    open.value = entry;
    closeDownload();
    closeInstall();
    query.value = '';
    refreshError.value = null;
    if (entry !== 'adobeFonts') void relist();
  }

  function close(): void {
    open.value = null;
    closeDownload();
    closeInstall();
  }

  /** The search text of the open catalog; the core filters by it. */
  function setQuery(text: string): void {
    if (text === query.value) return;
    query.value = text;
    void relist();
  }

  /**
   * Fetches the open catalog's list. A failure keeps the cached list and says why. The
   * Refresh button (`checks`) also compares the downloads with the new list, without
   * another request; the first fetch of a catalog just turned on does not.
   */
  async function refresh(
    id: CatalogId | undefined = current.value?.id,
    checks = true,
  ): Promise<boolean> {
    if (!id || refreshing.value) return false;
    refreshing.value = true;
    try {
      put(await client().refreshCatalog({ id }));
      refreshError.value = null;
      if (open.value === id) await relist();
      if (checks) await compare(true);
      return true;
    } catch (e) {
      if (open.value === id) refreshError.value = toApiError(e);
      else error.value = toApiError(e);
      return false;
    } finally {
      refreshing.value = false;
    }
  }

  /**
   * The "Turn on" switch of the sidebar and Settings. Turning on a catalog that was
   * never refreshed fetches its list at once: the switch says it connects.
   */
  async function setEnabled(id: CatalogId, enabled: boolean): Promise<void> {
    busy.value = id;
    try {
      const catalog = await client().setCatalogEnabled({ id, enabled });
      put(catalog);
      error.value = null;
      if (enabled && !catalog.refreshedAt) await refresh(id, false);
      else if (open.value === id) await relist();
    } catch (e) {
      error.value = toApiError(e);
    } finally {
      busy.value = null;
    }
  }

  /**
   * Asks the core which downloaded families have an update: `cachedOnly` compares with the
   * cached lists (after a Refresh), else the core refreshes the catalogs first.
   */
  async function compare(cachedOnly: boolean): Promise<boolean> {
    try {
      updates.value = await client().checkUpdates({ cachedOnly });
      checkError.value = null;
      return true;
    } catch (e) {
      checkError.value = toApiError(e);
      return false;
    }
  }

  /** The "Check for updates" button: refreshes the catalogs with downloads (requests). */
  async function checkUpdates(): Promise<boolean> {
    if (checking.value || refreshing.value) return false;
    checking.value = true;
    try {
      const done = await compare(false);
      // The refresh brought new lists: the catalogs' dates and the open list follow.
      if (done) {
        catalogs.value = await client().listCatalogs();
        if (current.value?.enabled) await relist();
      }
      return done;
    } catch (e) {
      error.value = toApiError(e);
      return false;
    } finally {
      checking.value = false;
    }
  }

  /** The update of `family` of `catalog`, when the last check found one. */
  function updateOf(catalog: CatalogId, family: string): CatalogUpdate | null {
    return (
      updates.value?.find(
        (u) => u.status === 'updateAvailable' && u.catalog === catalog && u.family === family,
      ) ?? null
    );
  }

  /** The update of the library family named `family`, from any catalog it came from. */
  function libraryUpdate(family: string): CatalogUpdate | null {
    return (
      updates.value?.find((u) => u.status === 'updateAvailable' && u.family === family) ?? null
    );
  }

  /**
   * A download or an install is running: the panel stays open, and no other download or
   * install starts (both buttons of every family are off).
   */
  const downloading = computed(
    () =>
      download.value?.phase === 'downloading' ||
      download.value?.phase === 'scanning' ||
      install.value?.phase === 'downloading' ||
      install.value?.phase === 'scanning',
  );

  /** Opens the "Download..." panel for a family of the open catalog. */
  function askDownload(family: CatalogFamily): void {
    const id = current.value?.id;
    if (!id || downloading.value) return;
    download.value = {
      catalog: id,
      family,
      phase: 'choosing',
      scan: null,
      result: null,
      error: null,
    };
  }

  /** Closes the panel; a running download keeps it open. */
  function closeDownload(): void {
    if (!downloading.value) download.value = null;
  }

  /**
   * Downloads the panel's family into `source` (a user action: the call goes to the
   * network), then follows the rescan it started through its scan events until it ends.
   */
  async function startDownload(source: SourceId, kind: CatalogFileKind): Promise<void> {
    const d = download.value;
    if (!d || (d.phase !== 'choosing' && d.phase !== 'failed')) return;
    d.phase = 'downloading';
    d.error = null;
    d.scan = null;
    // Listen before the call, so a fast rescan's events are not missed.
    let job: JobId | null = null;
    const early: Event[] = [];
    let unsubscribe: (() => void) | null = null;
    const follow = (event: Event): void => {
      if (job === null) {
        early.push(event);
        return;
      }
      if (!('job' in event) || event.job !== job) return;
      if (event.type === 'scanProgress') d.scan = { done: event.done, total: event.total };
      else if (event.type === 'scanFinished' || event.type === 'jobFailed') {
        if (event.type === 'jobFailed') d.error = event.error;
        d.phase = 'done';
        unsubscribe?.();
        unsubscribe = null;
      }
    };
    try {
      unsubscribe = await client().onEvent(follow);
    } catch {
      unsubscribe = null;
    }
    try {
      const result = await client().downloadFamily({
        id: d.catalog,
        family: d.family.name,
        kind,
        source,
      });
      d.result = result;
      d.phase = 'scanning';
      job = result.scan;
      early.splice(0).forEach(follow);
      // Without the event stream there is no progress to follow: the files are written.
      if (!unsubscribe && d.phase === 'scanning') d.phase = 'done';
    } catch (e) {
      d.error = toApiError(e);
      d.phase = 'failed';
      unsubscribe?.();
      unsubscribe = null;
    }
  }

  /**
   * The Install button of a family of the open catalog (E22.7). With a download folder set it
   * installs at once; the first time it proposes `<Documents>/Font Manager Fonts` and waits
   * for the user to use it or choose another folder.
   */
  async function askInstall(family: CatalogFamily): Promise<void> {
    const id = current.value?.id;
    if (!id || downloading.value) return;
    const settings = useSettingsStore();
    if (!settings.settings) await settings.load();
    install.value = {
      catalog: id,
      family,
      phase: 'proposing',
      proposed: null,
      result: null,
      error: null,
    };
    if (settings.settings?.downloadFolder) {
      await runInstall();
      return;
    }
    const i = install.value;
    try {
      i.proposed = await client().suggestedDownloadFolder();
    } catch (e) {
      i.error = toApiError(e);
      i.phase = 'failed';
    }
  }

  /** Saves `folder` as the download folder; false (and the install failed) when refused. */
  async function saveFolder(folder: string): Promise<boolean> {
    const settings = useSettingsStore();
    if (await settings.update({ downloadFolder: folder })) return true;
    if (install.value) {
      install.value.error = settings.error;
      install.value.phase = 'failed';
    }
    return false;
  }

  /** "Use this folder" of the first Install: saves the proposed folder, then installs. */
  async function useProposedFolder(): Promise<void> {
    const i = install.value;
    if (!i || i.phase !== 'proposing' || !i.proposed) return;
    if (await saveFolder(i.proposed)) await runInstall();
  }

  /** "Choose…" of the first Install: the native folder picker, then the install. */
  async function chooseInstallFolder(): Promise<void> {
    const i = install.value;
    if (!i || i.phase !== 'proposing') return;
    let path: string | null;
    try {
      path = await client().pickFolder(t('catalogs.install.folderTitle'));
    } catch (e) {
      i.error = toApiError(e);
      i.phase = 'failed';
      return;
    }
    if (path && (await saveFolder(path))) await runInstall();
  }

  /** Settings → Online catalogs "Change…": picks another download folder and saves it. */
  async function changeDownloadFolder(): Promise<boolean> {
    const path = await client().pickFolder(t('catalogs.install.folderTitle'));
    if (!path) return false;
    return useSettingsStore().update({ downloadFolder: path });
  }

  /**
   * Installs the progress line's family (a user action: the call goes to the network). The
   * call returns once the rescan is done and the faces are active; its `scanStarted` event
   * moves the line from downloading to scanning.
   */
  async function runInstall(): Promise<void> {
    const i = install.value;
    if (!i) return;
    i.phase = 'downloading';
    i.error = null;
    let unsubscribe: (() => void) | null;
    try {
      unsubscribe = await client().onEvent((event) => {
        if (event.type === 'scanStarted' && i.phase === 'downloading') i.phase = 'scanning';
      });
    } catch {
      unsubscribe = null;
    }
    try {
      i.result = await client().installFamily({
        id: i.catalog,
        family: i.family.name,
        // One file per style, as design apps list them; a family with only a variable font
        // gets that (the core picks the other kind).
        kind: 'static',
      });
      i.phase = 'done';
    } catch (e) {
      i.error = toApiError(e);
      i.phase = 'failed';
    } finally {
      unsubscribe?.();
    }
  }

  /** Closes the progress line; a running install keeps it. */
  function closeInstall(): void {
    if (install.value?.phase !== 'downloading' && install.value?.phase !== 'scanning')
      install.value = null;
  }

  // The Quick Collection review, a collection or All fonts takes the center back.
  watch(
    () => quick.open,
    (reviewing) => reviewing && close(),
  );
  watch(
    () => useSearchStore().filters,
    () => close(),
  );

  return {
    catalogs,
    open,
    current,
    query,
    families,
    total,
    listed,
    busy,
    refreshing,
    refreshError,
    error,
    online,
    offline,
    load,
    show,
    close,
    setQuery,
    want,
    refresh,
    setEnabled,
    download,
    downloading,
    askDownload,
    closeDownload,
    startDownload,
    install,
    askInstall,
    useProposedFolder,
    chooseInstallFolder,
    changeDownloadFolder,
    closeInstall,
    updates,
    checking,
    checkError,
    checkUpdates,
    updateOf,
    libraryUpdate,
  };
});
