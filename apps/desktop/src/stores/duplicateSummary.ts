import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  DuplicateSummary,
  Event,
  FamilyCopies,
  FamilyId,
  JobId,
  SourceId,
} from '../api/types';
import { useSourcesStore } from './sources';

/** A folder's own name, as the Sources cards show it. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** The notice after a scan that found copies (E56.15). */
export interface ScanNotice {
  /** The scan, so each scan has its notice once. */
  job: JobId;
  /** The scanned folder's own name. */
  folder: string;
  /** The fonts the scan added. */
  added: number;
  /** The files of the folder that are a copy of a font. */
  copies: number;
}

/**
 * The ways into the Duplicates module (E56.15): `duplicateSummary` as fm-api counts it — the
 * fonts with more than one copy (the sidebar row, the Problems line) and the families with
 * copies (the "N copies" mark on a wall card and on the family page) — and the notice after
 * a scan that found copies. Read again on library, scan and cleanup events; never polled.
 */
export const useDuplicateSummaryStore = defineStore('duplicateSummary', () => {
  const summary = ref<DuplicateSummary | null>(null);
  const notice = ref<ScanNotice | null>(null);
  /** The scans running, with the source each scans (`null`: every source). */
  const scans = new Map<JobId, SourceId | null>();
  /** The scans whose notice was shown, so a scan never shows two. */
  const noticed = new Set<JobId>();
  let unsubscribe: Promise<Unsubscribe> | undefined;
  let round = 0;

  const fonts = computed(() => summary.value?.fonts ?? 0);
  const byFamily = computed(
    () =>
      new Map<FamilyId, FamilyCopies>((summary.value?.families ?? []).map((f) => [f.family, f])),
  );
  /** The family's duplicates, when it has copies. */
  const of = (family: FamilyId | null | undefined): FamilyCopies | undefined =>
    family == null ? undefined : byFamily.value.get(family);

  async function refresh(): Promise<void> {
    const mine = ++round;
    try {
      const s = await useClient().duplicateSummary({});
      if (mine === round) summary.value = s;
    } catch {
      // The ways in stay as they were; the module itself reports errors.
    }
  }

  async function scanned(job: JobId, source: SourceId, added: number): Promise<void> {
    if (noticed.has(job) || added === 0) return;
    let copies: number;
    try {
      copies = (await useClient().duplicateSummary({ scope: { kind: 'source', source } })).copies;
    } catch {
      return;
    }
    if (copies === 0 || noticed.has(job)) return;
    const path = useSourcesStore().sources.find((s) => s.id === source)?.path;
    if (!path) return;
    noticed.add(job);
    notice.value = { job, folder: folderName(path), added, copies };
  }

  function onEvent(event: Event): void {
    if (event.type === 'scanStarted') scans.set(event.job, event.source ?? null);
    if (event.type === 'scanFinished') {
      const source = scans.get(event.job);
      scans.delete(event.job);
      if (source != null) void scanned(event.job, source, event.added);
    }
    if (
      event.type === 'libraryChanged' ||
      event.type === 'scanFinished' ||
      (event.type === 'cleanupProgress' && event.finished)
    )
      void refresh();
  }

  /** Listens to the events (once) and reads the counts. */
  async function start(): Promise<void> {
    if (!unsubscribe) {
      unsubscribe = useClient().onEvent(onEvent);
      void refresh();
    }
    await unsubscribe.catch(() => undefined);
  }

  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  function dismissNotice(): void {
    notice.value = null;
  }

  return { summary, fonts, notice, of, refresh, start, stop, dismissNotice };
});
