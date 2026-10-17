import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type FullSettings } from '../api/client';
import type { ApiError, AppFolders, SettingsUpdate, Source, SourceId } from '../api/types';

/**
 * The user's settings, the app folders and the sources, as fm-api reports them. Every
 * change is sent at once and the answer replaces the local copy, so what the window
 * shows is what was saved.
 */
export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<FullSettings | null>(null);
  const folders = ref<AppFolders | null>(null);
  const sources = ref<Source[]>([]);
  /** The last request that failed, for the Settings window to show. */
  const error = ref<ApiError | null>(null);

  // Changes run one after another, so a slow answer never overwrites a newer one.
  let queue: Promise<unknown> = Promise.resolve();
  function enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = queue.then(work, work);
    queue = next.catch(() => undefined);
    return next;
  }

  async function attempt(work: () => Promise<void>): Promise<boolean> {
    try {
      await work();
      error.value = null;
      return true;
    } catch (e) {
      error.value = e as ApiError;
      return false;
    }
  }

  /** Reads the settings (at app start, so the appearance applies everywhere). */
  function load(): Promise<boolean> {
    return attempt(async () => {
      settings.value = await useClient().getSettings();
    });
  }

  /** Reads what only the Settings window shows: the folders and the sources. */
  function loadWindow(): Promise<boolean> {
    return attempt(async () => {
      const client = useClient();
      const [f, s] = await Promise.all([client.getAppFolders(), client.listSources()]);
      folders.value = f;
      sources.value = s;
    });
  }

  /** Changes some settings; false (and `error` set) when the API refused. */
  function update(patch: SettingsUpdate): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        settings.value = await useClient().updateSettings(patch);
      }),
    );
  }

  function replaceSource(next: Source): void {
    sources.value = sources.value.map((s) => (s.id === next.id ? next : s));
  }

  /** Adds a folder as a source, indexed in place (ADR 0004). */
  function addSource(path: string): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        const added = await useClient().addSource({
          path: path.trim(),
          mode: { kind: 'indexInPlace' },
        });
        sources.value = [...sources.value, added];
      }),
    );
  }

  /** Forgets a source and its faces; the files stay where they are. */
  function removeSource(id: SourceId): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => {
        await useClient().removeSource(id);
        sources.value = sources.value.filter((s) => s.id !== id);
      }),
    );
  }

  function setSourcePaused(id: SourceId, paused: boolean): Promise<boolean> {
    return enqueue(() =>
      attempt(async () => replaceSource(await useClient().setSourcePaused({ source: id, paused }))),
    );
  }

  /** What the Activate button does; session until the settings are read. */
  const activationMode = computed(() => settings.value?.activationMode ?? 'session');
  /** What activation does on a name clash; keep the existing font until read. */
  const onConflict = computed(() => settings.value?.onConflict ?? 'keepExisting');

  return {
    settings,
    folders,
    sources,
    error,
    activationMode,
    onConflict,
    load,
    loadWindow,
    update,
    addSource,
    removeSource,
    setSourcePaused,
  };
});
