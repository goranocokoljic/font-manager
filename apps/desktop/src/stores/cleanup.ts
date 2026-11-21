// The plan, the apply and the history of the Duplicates module (E56.15). Everything about
// the fonts comes from the API: the plan of the draft (`planCleanupDraft`), the apply on a
// job whose progress arrives as `cleanupProgress` events (`applyCleanup`, `cancelCleanup`),
// the sessions and their undo (`listCleanupSessions`, `undoCleanupSession`), the hidden
// copies (`listHiddenCopies`, `showHiddenCopies`) and the remembered answers
// (`listDuplicateDecisions`, `takeBackDuplicateDecision`). The store keeps which value the
// user takes in each metadata conflict, and the job in progress; it never polls.
import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient, type Unsubscribe } from '../api/client';
import type {
  CleanupFile,
  CleanupPlan,
  CleanupSession,
  DuplicateDecision,
  Event,
  HiddenCopy,
  JobId,
  MetadataConflict,
  ResolutionId,
  StayReason,
} from '../api/types';
import { useDuplicatesStore } from './duplicates';

/** The order the plan lists the groups of files that stay in. */
export const STAY_ORDER: StayReason[] = [
  'keeper',
  'myLanguages',
  'keepBoth',
  'lastCopy',
  'active',
  'system',
  'keptByOtherSet',
  'notThere',
  'outsideScope',
  'onlyHere',
  'otherLevel',
];

/** One conflict's key: the kept face and the field (one conflict each in a plan). */
export const conflictKey = (c: Pick<MetadataConflict, 'keptFace' | 'field'>) =>
  `${c.keptFace}|${c.field}`;

/** The apply in progress, as its events report it. */
export interface CleanupRun {
  job: JobId;
  session: ResolutionId | null;
  done: number;
  total: number;
}

export const useCleanupStore = defineStore('cleanup', () => {
  const plan = ref<CleanupPlan | null>(null);
  const planning = ref(false);
  /** The conflicts whose other value the user takes, by `conflictKey`. */
  const taken = ref<Record<string, boolean>>({});
  const run = ref<CleanupRun | null>(null);
  const cancelling = ref(false);
  /** The session the last apply wrote, when it ended. */
  const result = ref<CleanupSession | null>(null);
  const sessions = ref<CleanupSession[] | null>(null);
  const hidden = ref<HiddenCopy[] | null>(null);
  const decisions = ref<DuplicateDecision[] | null>(null);
  const error = ref<unknown>(null);
  /** Jobs that ended before `applyCleanup` answered (a mock applies at once). */
  const ended = new Map<JobId, Extract<Event, { type: 'cleanupProgress' }>>();
  let unsubscribe: Promise<Unsubscribe> | undefined;
  let planRound = 0;

  const client = () => useClient();

  async function guard<T>(work: () => Promise<T>): Promise<T | undefined> {
    try {
      error.value = null;
      return await work();
    } catch (e) {
      error.value = e;
      return undefined;
    }
  }

  const files = (action: CleanupFile['action']) =>
    plan.value?.files.filter((f) => f.action === action) ?? [];
  const toTrash = computed(() => files('trash'));
  const toHide = computed(() => files('hide'));
  /** The files that stay, grouped by why, in `STAY_ORDER`. */
  const stays = computed(() => {
    const out: { reason: StayReason; files: CleanupFile[] }[] = [];
    for (const reason of STAY_ORDER) {
      const group = plan.value?.files.filter((f) => f.action === 'stays' && f.reason === reason);
      if (group?.length) out.push({ reason, files: group });
    }
    return out;
  });
  const leaving = computed(() => toTrash.value.length + toHide.value.length);

  /** Plans the draft again; the choices of conflicts that are still there are kept. */
  async function planDraft(): Promise<void> {
    const mine = ++planRound;
    planning.value = true;
    const p = await guard(() => client().planCleanupDraft({}));
    if (mine !== planRound) return;
    planning.value = false;
    if (!p) return;
    const keys = new Set(p.conflicts.map(conflictKey));
    taken.value = Object.fromEntries(Object.entries(taken.value).filter(([k]) => keys.has(k)));
    plan.value = p;
  }

  function take(c: MetadataConflict, other: boolean): void {
    taken.value = { ...taken.value, [conflictKey(c)]: other };
  }

  /**
   * The module's counts and lists after files moved or came back: the overview and the
   * draft, and the queue when it was opened (its total is the Sets count).
   */
  async function refreshDuplicates(): Promise<void> {
    const dupes = useDuplicatesStore();
    await Promise.all([dupes.load(), dupes.total != null ? dupes.reload() : undefined]);
  }

  async function finish(event: Extract<Event, { type: 'cleanupProgress' }>): Promise<void> {
    run.value = null;
    cancelling.value = false;
    plan.value = null;
    const list = await guard(() => client().listCleanupSessions());
    if (list) {
      sessions.value = list;
      result.value = list.find((s) => s.id === event.session) ?? null;
    }
    await Promise.all([refreshDuplicates(), planDraft()]);
  }

  function onEvent(event: Event): void {
    if (event.type === 'cleanupProgress') {
      const r = run.value;
      if (!r || r.job !== event.job) {
        if (event.finished) ended.set(event.job, event);
        return;
      }
      run.value = { ...r, session: event.session, done: event.done, total: event.total };
      if (event.finished) void finish(event);
    }
    if (event.type === 'libraryChanged' && !run.value) {
      if (sessions.value) void loadHistory();
      if (hidden.value) void loadHidden();
    }
  }

  /** Applies the plan in view with the values taken in its conflicts. */
  async function apply(): Promise<void> {
    const p = plan.value;
    if (!p || run.value) return;
    result.value = null;
    const choices = p.conflicts
      .filter((c) => taken.value[conflictKey(c)])
      .map((c) => ({ keptFace: c.keptFace, field: c.field, take: c.otherFace }));
    const total = p.files.filter((f) => f.action !== 'stays').length;
    const job = await guard(() => client().applyCleanup({ plan: p, choices }));
    if (job === undefined) return;
    run.value = { job, session: null, done: 0, total };
    const early = ended.get(job);
    if (early) {
      ended.delete(job);
      run.value = { ...run.value, session: early.session, done: early.done };
      await finish(early);
    }
  }

  /** Stops the apply between two files; the session keeps what was done. */
  async function cancel(): Promise<void> {
    const r = run.value;
    if (!r || cancelling.value) return;
    cancelling.value = true;
    await guard(() => client().cancelCleanup(r.job));
  }

  async function loadHistory(): Promise<void> {
    const list = await guard(() => client().listCleanupSessions());
    if (list) sessions.value = list;
  }

  /** Puts back a whole session, or the files named. */
  async function undo(session: ResolutionId, paths?: string[]): Promise<void> {
    const s = await guard(() => client().undoCleanupSession({ session, paths }));
    if (!s) return;
    if (result.value?.id === s.id) result.value = s;
    if (sessions.value) sessions.value = sessions.value.map((x) => (x.id === s.id ? s : x));
    else await loadHistory();
    await refreshDuplicates();
  }

  async function loadHidden(): Promise<void> {
    const list = await guard(() => client().listHiddenCopies());
    if (list) hidden.value = list;
  }

  async function showHidden(path: string): Promise<void> {
    if (!(await guard(() => client().showHiddenCopies({ paths: [path] })))) return;
    await Promise.all([loadHidden(), refreshDuplicates()]);
  }

  async function loadDecisions(): Promise<void> {
    const list = await guard(() => client().listDuplicateDecisions());
    if (list) decisions.value = list;
  }

  async function takeBackDecision(decision: DuplicateDecision['id']): Promise<void> {
    const done = await guard(async () => {
      await client().takeBackDuplicateDecision({ decision });
      return true;
    });
    if (!done) return;
    await Promise.all([loadDecisions(), refreshDuplicates()]);
  }

  /** Listens to the events (once). */
  async function start(): Promise<void> {
    unsubscribe ??= client().onEvent(onEvent);
    await unsubscribe.catch(() => undefined);
  }

  async function stop(): Promise<void> {
    const pending = unsubscribe;
    unsubscribe = undefined;
    (await pending?.catch(() => undefined))?.();
  }

  return {
    plan,
    planning,
    taken,
    run,
    cancelling,
    result,
    sessions,
    hidden,
    decisions,
    error,
    toTrash,
    toHide,
    stays,
    leaving,
    planDraft,
    take,
    apply,
    cancel,
    loadHistory,
    undo,
    loadHidden,
    showHidden,
    loadDecisions,
    takeBackDecision,
    start,
    stop,
  };
});
