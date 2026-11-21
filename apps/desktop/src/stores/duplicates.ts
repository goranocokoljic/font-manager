// The Duplicates module (E56.11): the overview, the queue of sets the user works
// through, the set in view with a choice per copy, and the draft plan the decisions
// collect into. Everything about the fonts comes from the API: the overview's counts
// (`duplicatesOverview`), the queue's filter, order and pages (`listDuplicateSets`),
// what a cleanup would do with each copy and why (`planCleanup` on the one set), and
// the stored draft (`getCleanupDraft` / `updateCleanupDraft`). The store keeps which
// set is in view and which outcome the user picked for each copy, and nothing else.
import { defineStore } from 'pinia';
import { computed, ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type {
  CleanupDraft,
  CleanupDraftUpdate,
  CleanupFile,
  CopyChoice,
  CopyComparison,
  DuplicateLevel,
  DuplicateScope,
  DuplicateSet,
  DuplicateSetSort,
  DuplicatesOverview,
  FolderPair,
  FontFormat,
  LanguagePreset,
  RecommendationFilter,
  SetDecision,
} from '../api/types';

/** The sets one request asks for. */
export const QUEUE_PAGE = 100;

/** The levels the queue holds: identical sets are one decision of their own. */
export const QUEUE_LEVELS: DuplicateLevel[] = ['sameVersion', 'otherVersion'];

/** The stay reasons that mean a copy cannot leave: it shows why instead of a choice. */
const FIXED = new Set(['active', 'system', 'keptByOtherSet', 'notThere']);

/** The stay reasons that protect a copy: it starts as kept and is never preselected to go. */
const PROTECTED = new Set(['myLanguages', 'keepBoth', 'lastCopy']);

/** The queue's filter, as the request's `filter` and `scope`. */
export interface QueueFilter {
  /** One level, or both queue levels when null. */
  level: DuplicateLevel | null;
  recommendation: RecommendationFilter | null;
  format: FontFormat | null;
  scope: DuplicateScope;
}

const emptyDraft = (): CleanupDraft => ({ identical: false, decisions: [] });

export const useDuplicatesStore = defineStore('duplicates', () => {
  const overview = ref<DuplicatesOverview | null>(null);
  const draft = ref<CleanupDraft>(emptyDraft());
  const pairs = ref<FolderPair[]>([]);
  const pairTotal = ref<number | null>(null);
  const sessions = ref<number | null>(null);
  const presets = ref<LanguagePreset[]>([]);
  const error = ref<unknown>(null);

  const filter = ref<QueueFilter>({
    level: null,
    recommendation: null,
    format: null,
    scope: { kind: 'library' },
  });
  const sort = ref<DuplicateSetSort>('name');
  /** The sets that pass the filter and have no decision yet. */
  const total = ref<number | null>(null);
  /** Loaded pages by page number; a new filter or a decision drops them. */
  const pages = shallowRef(new Map<number, DuplicateSet[]>());
  /** The position of the set in view. */
  const cursor = ref(0);
  /** The set in view and what a cleanup would do with its copies. */
  const current = ref<DuplicateSet | null>(null);
  const planFiles = ref<CleanupFile[]>([]);
  /** The outcome the user picked per copy path. */
  const choices = ref<Record<string, CopyChoice>>({});
  const comparison = ref<CopyComparison | null>(null);
  /** Bumped on every queue reload, so a late page or plan of an older queue is dropped. */
  let generation = 0;

  const client = () => useClient();

  async function guard<T>(run: () => Promise<T>): Promise<T | undefined> {
    try {
      error.value = null;
      return await run();
    } catch (e) {
      error.value = e;
      return undefined;
    }
  }

  /** The overview, the draft, the top folder pairs, the history count and the presets. */
  async function load(): Promise<void> {
    await guard(async () => {
      const c = client();
      const [o, d, p, s, l] = await Promise.all([
        c.duplicatesOverview({}),
        c.getCleanupDraft(),
        c.listFolderPairs({ page: { offset: 0, limit: 2 } }),
        c.listCleanupSessions(),
        presets.value.length ? Promise.resolve(presets.value) : c.listLanguages(),
      ]);
      overview.value = o;
      draft.value = d;
      pairs.value = p.items;
      pairTotal.value = p.total;
      sessions.value = s.length;
      presets.value = l;
    });
  }

  function request(offset: number) {
    const f = filter.value;
    return {
      scope: f.scope,
      page: { offset, limit: QUEUE_PAGE },
      filter: {
        levels: f.level ? [f.level] : QUEUE_LEVELS,
        format: f.format,
        recommendation: f.recommendation,
        undecided: true,
      },
      sort: sort.value,
    };
  }

  /** The page holding position `index`, loaded when it is not yet. */
  async function ensure(index: number): Promise<void> {
    const n = Math.floor(index / QUEUE_PAGE);
    if (pages.value.has(n)) return;
    const asked = generation;
    const page = await guard(() => client().listDuplicateSets(request(n * QUEUE_PAGE)));
    if (!page || asked !== generation) return;
    total.value = page.total;
    const next = new Map(pages.value);
    next.set(n, page.items);
    pages.value = next;
  }

  /** The set at `index` when its page is loaded. */
  function at(index: number): DuplicateSet | undefined {
    return pages.value.get(Math.floor(index / QUEUE_PAGE))?.[index % QUEUE_PAGE];
  }

  /** Loads the queue again from the API and shows the set at `index` (clamped). */
  async function reload(index = cursor.value): Promise<void> {
    generation += 1;
    pages.value = new Map();
    await ensure(Math.max(0, index));
    const last = (total.value ?? 0) - 1;
    const to = Math.max(0, Math.min(index, last));
    if (to !== index) await ensure(to);
    await select(to);
  }

  /** Shows the set at `index`: asks what a cleanup would do with its copies. */
  async function select(index: number): Promise<void> {
    const count = total.value ?? 0;
    if (count === 0) {
      cursor.value = 0;
      current.value = null;
      planFiles.value = [];
      choices.value = {};
      comparison.value = null;
      return;
    }
    const to = Math.max(0, Math.min(index, count - 1));
    await ensure(to);
    const set = at(to);
    cursor.value = to;
    if (!set) return;
    current.value = set;
    comparison.value = null;
    const asked = generation;
    const plan = await guard(() =>
      client().planCleanup({ scope: { kind: 'sets', ids: [set.id] } }),
    );
    if (asked !== generation || current.value?.id !== set.id) return;
    planFiles.value = plan?.files.filter((f) => f.set === set.id) ?? [];
    const next: Record<string, CopyChoice> = {};
    for (const copy of set.copies) {
      const file = plan?.files.find((f) => f.path === copy.path);
      next[copy.path] =
        file?.action === 'trash' ? 'trash' : file?.action === 'hide' ? 'hide' : 'keep';
    }
    choices.value = next;
  }

  /** What the plan says about one copy of the set in view. */
  const fileOf = (path: string) => planFiles.value.find((f) => f.path === path);

  /** The copy the API keeps for the set in view: the recommendation's, else the plan's. */
  const keeper = computed<string | null>(() => {
    const set = current.value;
    if (!set) return null;
    return (
      set.recommendation?.keeper ?? planFiles.value.find((f) => f.reason === 'keeper')?.path ?? null
    );
  });

  /** Why the keeper is kept, as the API says it. */
  const reason = computed<string | null>(() => {
    const set = current.value;
    if (!set) return null;
    const r = set.recommendation;
    if (r?.reason) return r.reason.sentence;
    const kept = r?.keep.find((k) => k.path === keeper.value);
    if (kept) return kept.sentence;
    return planFiles.value.find((f) => f.path === keeper.value)?.sentence ?? null;
  });

  /** Whether a copy cannot leave (active, system, kept by another set, gone). */
  const fixed = (path: string) => {
    const r = fileOf(path)?.reason;
    return r != null && FIXED.has(r);
  };
  /** Whether the API protects a copy (my languages, keep both, last copy). */
  const isProtected = (path: string) => {
    const r = fileOf(path)?.reason;
    return r != null && PROTECTED.has(r);
  };

  function choose(path: string, choice: CopyChoice): void {
    if (fixed(path)) return;
    choices.value = { ...choices.value, [path]: choice };
  }

  /** Keeps `path` and sends every other copy that may leave to the trash. */
  function keepOnly(path: string): void {
    const set = current.value;
    if (!set || fixed(path)) return;
    const next: Record<string, CopyChoice> = {};
    for (const c of set.copies)
      next[c.path] = c.path === path || fixed(c.path) || isProtected(c.path) ? 'keep' : 'trash';
    choices.value = next;
  }

  async function change(update: CleanupDraftUpdate): Promise<boolean> {
    const d = await guard(() => client().updateCleanupDraft(update));
    if (!d) return false;
    draft.value = d;
    return true;
  }

  /**
   * Adds the set in view to the draft with `kind`, then shows the set that took its
   * place in the queue (the next one; the previous one when it was the last).
   */
  async function decide(kind: SetDecision['kind']): Promise<boolean> {
    const set = current.value;
    if (!set) return false;
    const decision: SetDecision =
      kind === 'copies'
        ? {
            kind,
            copies: set.copies
              .filter((c) => !fixed(c.path))
              .map((c) => ({ path: c.path, choice: choices.value[c.path] ?? 'keep' })),
          }
        : { kind };
    if (!(await change({ kind: 'decide', set: set.id, decision }))) return false;
    await reload(cursor.value);
    return true;
  }

  async function takeBack(set: string): Promise<void> {
    if (await change({ kind: 'takeBack', set })) await reload(cursor.value);
  }

  async function includeIdentical(include: boolean): Promise<void> {
    await change({ kind: 'identical', include });
  }

  async function clearDraft(): Promise<void> {
    if (await change({ kind: 'clear' })) await reload(0);
  }

  async function setFilter(patch: Partial<QueueFilter>): Promise<void> {
    filter.value = { ...filter.value, ...patch };
    await reload(0);
  }

  async function setSort(value: DuplicateSetSort): Promise<void> {
    sort.value = value;
    await reload(0);
  }

  /** Compares the keeper with `other` (or the first other copy) through the API. */
  async function compare(other?: string): Promise<void> {
    const set = current.value;
    if (!set) return;
    const keep = set.copies.find((c) => c.path === keeper.value) ?? set.copies[0];
    const with_ =
      set.copies.find((c) => c.path === other && c.path !== keep?.path) ??
      set.copies.find((c) => c.path !== keep?.path);
    if (keep?.face == null || with_?.face == null) return;
    const result = await guard(() => client().compareCopies({ a: keep.face!, b: with_.face! }));
    if (result && current.value?.id === set.id) comparison.value = result;
  }

  /** The number of sets the draft holds: the decisions and, when in, the identical sets. */
  const planned = computed(
    () =>
      (draft.value.decisions?.length ?? 0) +
      (draft.value.identical ? (overview.value?.identical.sets ?? 0) : 0),
  );

  return {
    overview,
    draft,
    pairs,
    pairTotal,
    sessions,
    presets,
    error,
    filter,
    sort,
    total,
    pages,
    cursor,
    current,
    planFiles,
    choices,
    comparison,
    keeper,
    reason,
    planned,
    load,
    ensure,
    at,
    reload,
    select,
    fileOf,
    fixed,
    isProtected,
    choose,
    keepOnly,
    decide,
    takeBack,
    includeIdentical,
    clearDraft,
    setFilter,
    setSort,
    compare,
  };
});
