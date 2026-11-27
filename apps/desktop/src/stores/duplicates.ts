// The Duplicates module (E56.11): the overview, the queue of sets the user works
// through, the set in view with a choice per copy, and the draft plan the decisions
// collect into. Everything about the fonts comes from the API: the overview's counts
// (`duplicatesOverview`), the queue's filter, order and pages (`listDuplicateSets`),
// what a cleanup would do with each copy and why (`planCleanup` on the one set), and
// the stored draft (`getCleanupDraft` / `updateCleanupDraft`). The store keeps which
// set is in view and which outcome the user picked for each copy, and nothing else.
// The Folders view (E56.12) pages through `listFolderPairs`, opens a pair to the files
// `listFolderPairFiles` names, and puts "Keep this side" into the same draft.
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
  DraftPair,
  FolderPair,
  FolderPairFile,
  FolderPairSide,
  FontFormat,
  LanguagePreset,
  PairSide,
  RecommendationFilter,
  SetDecision,
} from '../api/types';
import { useSearchStore } from './search';

/** The folder pairs one request asks for. */
export const PAIR_PAGE = 100;

/** The files of an open pair one request asks for. */
export const PAIR_FILES_PAGE = 100;

/**
 * The lists an open pair shows: what both folders have (identical copies), the copies
 * that are not identical, and what only one side has. `both` and `differ` list side
 * `a`'s files, each with its copy on side `b`, so no file is listed twice.
 */
export type PairGroup = 'both' | 'differ' | 'onlyA' | 'onlyB';
export const PAIR_GROUPS: PairGroup[] = ['both', 'differ', 'onlyA', 'onlyB'];

const GROUP_REQUEST: Record<
  PairGroup,
  { side: PairSide; onlyHere?: boolean; levels?: DuplicateLevel[] }
> = {
  both: { side: 'a', levels: ['identical'] },
  differ: { side: 'a', levels: ['sameVersion', 'otherVersion'] },
  onlyA: { side: 'a', onlyHere: true },
  onlyB: { side: 'b', onlyHere: true },
};

/** The font files of a side that have a copy on the other side, at any level. */
export const sharedFiles = (side: FolderPairSide) =>
  side.shared.identical + side.shared.sameVersion + side.shared.otherVersion;

/**
 * The share of a pair, 0–1: the larger of both sides' shares of their font files that
 * have a copy on the other side (the API's counts).
 */
export const pairShare = (pair: FolderPair) =>
  Math.max(
    pair.a.fontFiles ? sharedFiles(pair.a) / pair.a.fontFiles : 0,
    pair.b.fontFiles ? sharedFiles(pair.b) / pair.b.fontFiles : 0,
  );

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
  /**
   * Only these sets, at every level (E56.15: the copies of one family, from its "N copies"
   * mark); null for the whole queue.
   */
  ids?: string[] | null;
}

/** The level chips of the queue. */
export type QueueChip = 'all' | DuplicateLevel | 'keepBoth';

const emptyDraft = (): CleanupDraft => ({ identical: false, decisions: [], pairs: [] });

export const useDuplicatesStore = defineStore('duplicates', () => {
  const overview = ref<DuplicatesOverview | null>(null);
  const draft = ref<CleanupDraft>(emptyDraft());
  const pairs = ref<FolderPair[]>([]);
  const pairTotal = ref<number | null>(null);
  const sessions = ref<number | null>(null);
  const search = useSearchStore();
  /** Every language preset, in the API's order: the search store's list, not a copy (E58.3). */
  const presets = computed<LanguagePreset[]>(() => search.languages);
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

  // The Folders view (E56.12).
  /** Loaded pages of folder pairs by page number. */
  const pairPages = shallowRef(new Map<number, FolderPair[]>());
  /** The pair that is open, and its files by list. */
  const openPair = ref<FolderPair | null>(null);
  const pairFiles = ref<Partial<Record<PairGroup, { items: FolderPairFile[]; total: number }>>>({});
  let pairGeneration = 0;

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
      const [o, d, p, s] = await Promise.all([
        c.duplicatesOverview({}),
        c.getCleanupDraft(),
        c.listFolderPairs({ page: { offset: 0, limit: 2 } }),
        c.listCleanupSessions(),
        search.loadLanguages(),
      ]);
      overview.value = o;
      draft.value = d;
      pairs.value = p.items;
      pairTotal.value = p.total;
      sessions.value = s.length;
    });
  }

  function request(offset: number) {
    const f = filter.value;
    return {
      scope: f.scope,
      page: { offset, limit: QUEUE_PAGE },
      filter: {
        // A family's sets are listed at every level, identical ones too.
        levels: f.level ? [f.level] : f.ids ? [] : QUEUE_LEVELS,
        ids: f.ids ?? [],
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

  /**
   * The chip in view: "All", one queue level, or "Keep both" (the sets whose
   * recommendation keeps several copies, at either level).
   */
  const chip = computed<QueueChip>(() =>
    filter.value.recommendation === 'keepBoth' ? 'keepBoth' : (filter.value.level ?? 'all'),
  );

  /** Shows the queue of one chip; leaving "Keep both" drops its recommendation filter. */
  async function showChip(id: QueueChip): Promise<void> {
    await setFilter(
      id === 'keepBoth'
        ? { level: null, recommendation: 'keepBoth' }
        : {
            level: id === 'all' ? null : id,
            recommendation:
              filter.value.recommendation === 'keepBoth' ? null : filter.value.recommendation,
          },
    );
  }

  async function setSort(value: DuplicateSetSort): Promise<void> {
    sort.value = value;
    await reload(0);
  }

  /**
   * Compares `first` (else the keeper) with `other` (else the first other copy) through
   * the API (E56.13: the user picks both copies of the set).
   */
  async function compare(other?: string, first?: string): Promise<void> {
    const set = current.value;
    if (!set) return;
    const keep = set.copies.find((c) => c.path === (first ?? keeper.value)) ?? set.copies[0];
    const with_ =
      set.copies.find((c) => c.path === other && c.path !== keep?.path) ??
      set.copies.find((c) => c.path !== keep?.path);
    if (keep?.face == null || with_?.face == null) return;
    const result = await guard(() => client().compareCopies({ a: keep.face!, b: with_.face! }));
    if (result && current.value?.id === set.id) comparison.value = result;
  }

  /**
   * What the draft holds: the decisions, the identical sets when in, and the folder
   * pairs.
   */
  const planned = computed(
    () =>
      (draft.value.decisions?.length ?? 0) +
      (draft.value.identical ? (overview.value?.identical.sets ?? 0) : 0) +
      (draft.value.pairs?.length ?? 0),
  );

  /** The page of pairs holding position `index`, loaded when it is not yet. */
  async function ensurePair(index: number): Promise<void> {
    const n = Math.floor(index / PAIR_PAGE);
    if (pairPages.value.has(n)) return;
    const asked = pairGeneration;
    const page = await guard(() =>
      client().listFolderPairs({ page: { offset: n * PAIR_PAGE, limit: PAIR_PAGE } }),
    );
    if (!page || asked !== pairGeneration) return;
    pairTotal.value = page.total;
    const next = new Map(pairPages.value);
    next.set(n, page.items);
    pairPages.value = next;
  }

  /** The pair at `index` when its page is loaded. */
  function pairAt(index: number): FolderPair | undefined {
    return pairPages.value.get(Math.floor(index / PAIR_PAGE))?.[index % PAIR_PAGE];
  }

  /** Loads the pairs again from the API, the first page first. */
  async function reloadPairs(): Promise<void> {
    pairGeneration += 1;
    pairPages.value = new Map();
    await ensurePair(0);
  }

  async function loadPairFiles(group: PairGroup, offset: number): Promise<void> {
    const pair = openPair.value;
    if (!pair) return;
    const page = await guard(() =>
      client().listFolderPairFiles({
        a: pair.a.path,
        b: pair.b.path,
        ...GROUP_REQUEST[group],
        page: { offset, limit: PAIR_FILES_PAGE },
      }),
    );
    if (!page || openPair.value !== pair) return;
    const before = offset > 0 ? (pairFiles.value[group]?.items ?? []) : [];
    pairFiles.value = {
      ...pairFiles.value,
      [group]: { items: [...before, ...page.items], total: page.total },
    };
  }

  /** Opens `pair` (or closes the open one): its files, list by list. */
  async function showPair(pair: FolderPair | null): Promise<void> {
    openPair.value = pair;
    pairFiles.value = {};
    if (pair) await Promise.all(PAIR_GROUPS.map((g) => loadPairFiles(g, 0)));
  }

  /** The next page of one list of the open pair. */
  async function morePairFiles(group: PairGroup): Promise<void> {
    await loadPairFiles(group, pairFiles.value[group]?.items.length ?? 0);
  }

  /** The draft's decision about `pair`, when it has one. */
  const draftPair = (pair: FolderPair): DraftPair | undefined =>
    draft.value.pairs?.find(
      (p) =>
        (p.keep === pair.a.path && p.remove === pair.b.path) ||
        (p.keep === pair.b.path && p.remove === pair.a.path),
    );

  /**
   * "Keep this side": keeps `side`'s copies and plans the removal of the other side's
   * identical copies, nothing else. The API refuses a protected side to remove.
   */
  async function keepSide(pair: FolderPair, side: PairSide): Promise<boolean> {
    const [keep, remove] = side === 'a' ? [pair.a, pair.b] : [pair.b, pair.a];
    return change({ kind: 'keepSide', keep: keep.path, remove: remove.path });
  }

  /** Takes back the draft's decision about the folders `a` and `b`. */
  async function takeBackPair(a: string, b: string): Promise<boolean> {
    return change({ kind: 'takeBackPair', a, b });
  }

  /** Shows one side of `pair` in the file manager. */
  async function showPairFolder(pair: FolderPair, side: PairSide): Promise<void> {
    await guard(() => client().showPairFolder(pair.a.path, pair.b.path, side));
  }

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
    pairPages,
    openPair,
    pairFiles,
    ensurePair,
    pairAt,
    reloadPairs,
    showPair,
    morePairFiles,
    draftPair,
    keepSide,
    takeBackPair,
    showPairFolder,
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
    chip,
    showChip,
    setSort,
    compare,
  };
});
