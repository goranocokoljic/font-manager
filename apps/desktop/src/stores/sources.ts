import { defineStore } from 'pinia';
import { computed, reactive, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type { ApiError, Event, JobId, Source, SourceId } from '../api/types';
import { t } from '../i18n';

/** How far a running scan is. */
export interface ScanProgress {
  done: number;
  total: number;
}

/**
 * The sidebar's sources: watched folders, the library folder and the system
 * fonts, with the progress of running scans from the event stream.
 */
export const useSourcesStore = defineStore('sources', () => {
  const sources = ref<Source[]>([]);
  const error = ref<ApiError | null>(null);
  /** Running scans by job. */
  const scans = reactive(new Map<JobId, ScanProgress>());
  /** The source a job scans, from its start event or the store's own request (`null`: every source). */
  const jobSource = reactive(new Map<JobId, SourceId | null>());
  /** Jobs whose end arrived; a late `scan()` reply must not bring them back. */
  const ended = new Set<JobId>();
  let unsubscribe: Promise<Unsubscribe> | undefined;

  async function attempt<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await work();
      error.value = null;
      return result;
    } catch (e) {
      error.value = e as ApiError;
      return undefined;
    }
  }

  async function refresh(): Promise<void> {
    await attempt(async () => {
      sources.value = await useClient().listSources();
    });
  }

  function endJob(job: JobId): void {
    ended.add(job);
    scans.delete(job);
    jobSource.delete(job);
  }

  function onEvent(event: Event): void {
    switch (event.type) {
      case 'scanStarted':
        if (ended.has(event.job)) break;
        if (!scans.has(event.job)) scans.set(event.job, { done: 0, total: 0 });
        if (!jobSource.has(event.job)) jobSource.set(event.job, event.source ?? null);
        break;
      case 'scanProgress':
        if (!ended.has(event.job)) scans.set(event.job, { done: event.done, total: event.total });
        break;
      case 'scanFinished':
        endJob(event.job);
        void refresh();
        break;
      case 'jobFailed':
        if (scans.has(event.job) || jobSource.has(event.job)) error.value = event.error;
        endJob(event.job);
        break;
      case 'libraryChanged':
        void refresh();
        break;
    }
  }

  /** Subscribes to events (once) and loads the sources. */
  async function load(): Promise<void> {
    unsubscribe ??= useClient().onEvent(onEvent);
    await attempt(() => unsubscribe!);
    await refresh();
  }

  /** Stops listening to events (the sidebar left the page). */
  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  /** Scans one source and attributes its progress to it. */
  async function rescan(source: SourceId | null): Promise<void> {
    const job = await attempt(() => useClient().scan({ source }));
    if (job !== undefined && !ended.has(job)) jobSource.set(job, source);
  }

  /** Picks a folder with the native picker, adds it and scans it. */
  async function add(): Promise<void> {
    const path = await attempt(() => useClient().pickFolder(t('sources.addFolderTitle')));
    if (!path) return;
    const source = await attempt(() =>
      useClient().addSource({ path, mode: { kind: 'indexInPlace' } }),
    );
    if (!source) return;
    sources.value = [...sources.value, source];
    await rescan(source.id);
  }

  async function setPaused(source: SourceId, paused: boolean): Promise<void> {
    const updated = await attempt(() => useClient().setSourcePaused({ source, paused }));
    if (updated) sources.value = sources.value.map((s) => (s.id === source ? updated : s));
  }

  /** Removes a source from the library; its files stay where they are. */
  async function remove(source: SourceId): Promise<void> {
    const done = await attempt(async () => {
      await useClient().removeSource(source);
      return true;
    });
    if (done) sources.value = sources.value.filter((s) => s.id !== source);
  }

  /** The running scan of one source, wherever it was started. */
  function progressOf(source: SourceId): ScanProgress | null {
    for (const [job, s] of jobSource) if (s === source) return scans.get(job) ?? null;
    return null;
  }

  /** Scans not tied to one source (every source, or started elsewhere), summed. */
  const otherScan = computed<ScanProgress | null>(() => {
    const others = [...scans].filter(([job]) => jobSource.get(job) == null);
    if (!others.length) return null;
    return others.reduce<ScanProgress>(
      (sum, [, p]) => ({ done: sum.done + p.done, total: sum.total + p.total }),
      { done: 0, total: 0 },
    );
  });

  return {
    sources,
    error,
    otherScan,
    load,
    stop,
    add,
    rescan,
    setPaused,
    remove,
    progressOf,
  };
});
