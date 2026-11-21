// The Duplicates module's overview and draft plan in the TypeScript mock (E56.11): a
// copy of `fm_api::cleanup_draft`. The overview counts a plan as
// `DuplicatesOverview::from_plan`; a draft change follows `CleanupDraft::apply`. The
// draft's plan (E56.15) follows `plan_draft`, the summary `DuplicateSummary::from_sets`.
import type {
  ApiError,
  CleanupKeeper,
  CopyChoice,
  DuplicateSummary,
  FaceId,
  FamilyCopies,
  FamilyId,
  PlanCleanupRequest,
  PlanDraftRequest,
  PlannedAnswer,
  PlannedDraft,
  CleanupDraft,
  CleanupDraftUpdate,
  CleanupPlan,
  DuplicateLevel,
  DuplicateSet,
  DuplicatesOverview,
  DraftPair,
  FolderPair,
  LevelSummary,
  ProtectedFolder,
  SetDecision,
} from './types';

const invalid = (message: string): ApiError => ({ code: 'invalidRequest', message, detail: null });

const cap = (n: number) => Math.min(n, 0xffff_ffff);

/** What `plan` does, counted, as `DuplicatesOverview::from_plan`. */
export const overviewFromPlan = (plan: CleanupPlan): DuplicatesOverview => {
  const levels = new Map<
    DuplicateLevel,
    { sets: Set<string>; files: number; frees: number; folders: Set<string> }
  >();
  const keepBoth = new Set<string>();
  const folders = new Set<string>();
  let extra = 0;
  for (const f of plan.files) {
    let entry = levels.get(f.level);
    if (!entry)
      levels.set(f.level, (entry = { sets: new Set(), files: 0, frees: 0, folders: new Set() }));
    entry.sets.add(f.set);
    if (f.reason === 'keepBoth') keepBoth.add(f.set);
    if (f.action === 'stays') continue;
    extra += 1;
    entry.files += 1;
    if (f.action === 'trash') entry.frees = cap(entry.frees + f.size);
    const cut = Math.max(f.path.lastIndexOf('/'), f.path.lastIndexOf('\\'));
    if (cut >= 0) {
      const dir = f.path.slice(0, cut) || f.path.slice(0, 1);
      folders.add(dir);
      entry.folders.add(dir);
    }
  }
  const level = (l: DuplicateLevel): LevelSummary => {
    const e = levels.get(l);
    return e
      ? { sets: e.sets.size, files: e.files, frees: e.frees, folders: e.folders.size }
      : { sets: 0, files: 0, frees: 0, folders: 0 };
  };
  return {
    fonts: plan.sets,
    extraFiles: extra,
    folders: folders.size,
    frees: plan.frees,
    identical: level('identical'),
    sameVersion: level('sameVersion'),
    otherVersion: level('otherVersion'),
    keepBoth: keepBoth.size,
    myLanguages: plan.myLanguages,
  };
};

/** Whether `decision` fits `set`, as `SetDecision::check`; the error when not. */
const check = (decision: SetDecision, set: DuplicateSet): ApiError | null => {
  if (decision.kind !== 'copies') return null;
  const seen = new Set<string>();
  for (const d of decision.copies) {
    const copy = set.copies.find((c) => c.path === d.path);
    if (!copy) return invalid(`${d.path} is not a copy of this font.`);
    if (seen.has(d.path)) return invalid(`${d.path} is named twice.`);
    seen.add(d.path);
    if (copy.system && d.choice !== 'keep')
      return invalid(
        'A copy in a system font folder always stays. Font Fu never moves system fonts.',
      );
  }
  const kept = set.copies.some((c) => {
    const d = decision.copies.find((x) => x.path === c.path);
    return !d || d.choice === 'keep';
  });
  return kept ? null : invalid('Keep at least one copy of the font.');
};

/** What a protected folder is, as `ProtectedFolder::describe`. */
const DESCRIBE: Record<ProtectedFolder, string> = {
  sourceRoot: 'a folder you added to the library',
  systemFolder: 'a system font folder',
  libraryFolder: 'the library folder',
};

/** Whether `p` is about the folders `a` and `b`, either way round, as `DraftPair::is_pair`. */
const isPair = (p: DraftPair, a: string, b: string) =>
  (p.keep === a && p.remove === b) || (p.keep === b && p.remove === a);

/**
 * `draft` changed by `update`, as `CleanupDraft::apply`; `setOf` finds a set of the
 * library by id, `pairOf` the folder pair of two folders. Throws the API's error when
 * the change is refused.
 */
export const applyDraftUpdate = (
  draft: CleanupDraft,
  update: CleanupDraftUpdate,
  now: string,
  setOf: (id: string) => DuplicateSet | undefined,
  pairOf: (a: string, b: string) => FolderPair | undefined = () => undefined,
): CleanupDraft => {
  const decisions = draft.decisions ?? [];
  const pairs = draft.pairs ?? [];
  const identical = draft.identical ?? false;
  switch (update.kind) {
    case 'decide': {
      const set = setOf(update.set);
      if (!set)
        throw {
          code: 'notFound',
          message: `${update.set} is not a duplicate set any more.`,
          detail: null,
        } satisfies ApiError;
      const error = check(update.decision, set);
      if (error) throw error;
      return {
        identical,
        pairs,
        decisions: [
          ...decisions.filter((d) => d.set !== set.id),
          {
            set: set.id,
            level: set.level,
            postscriptName: set.postscriptName,
            decision: update.decision,
            at: now,
          },
        ],
      };
    }
    case 'takeBack':
      return { identical, pairs, decisions: decisions.filter((d) => d.set !== update.set) };
    case 'identical':
      return { identical: update.include, decisions, pairs };
    case 'keepSide': {
      const { keep, remove } = update;
      if (keep === remove) throw invalid('Choose two different folders.');
      const pair = pairOf(keep, remove);
      if (!pair)
        throw {
          code: 'notFound',
          message: `${keep} and ${remove} do not hold the same fonts any more.`,
          detail: null,
        } satisfies ApiError;
      const removed = pair.a.path === remove ? pair.a : pair.b.path === remove ? pair.b : undefined;
      if (!removed)
        throw {
          code: 'notFound',
          message: `${remove} is not a side of this folder pair.`,
          detail: null,
        } satisfies ApiError;
      if (removed.protected)
        throw invalid(
          `${remove} is ${DESCRIBE[removed.protected]}: it can be the side that is kept, never the side that is removed.`,
        );
      return {
        identical,
        decisions,
        pairs: [
          ...pairs.filter((p) => !isPair(p, keep, remove)),
          {
            keep,
            remove,
            identical: removed.shared.identical,
            identicalSize: removed.identicalSize,
            at: now,
          },
        ],
      };
    }
    case 'takeBackPair':
      return { identical, decisions, pairs: pairs.filter((p) => !isPair(p, update.a, update.b)) };
    case 'clear':
      return { identical: false, decisions: [], pairs: [] };
  }
};

const KEPT_BY_CHOICE = 'Stays: you chose to keep this copy.';
const KEPT_FOR_ANOTHER = 'Stays: another file of this plan leaves for it, so it is kept.';

/**
 * The draft as one plan (E56.15), as `fm_api::cleanup_draft::plan_draft`: one plan of the
 * identical sets and the sets with a choice per copy, then each folder pair, then no part
 * removes a copy another part keeps; "keep both" and "not duplicates" are its answers.
 */
export const planDraft = async (
  draft: CleanupDraft,
  sets: DuplicateSet[],
  req: PlanDraftRequest,
  plan: (req: PlanCleanupRequest) => Promise<CleanupPlan>,
): Promise<CleanupPlan> => {
  const byId = new Map(sets.map((s) => [s.id, s]));
  const ids: string[] = [];
  const keepers: CleanupKeeper[] = [];
  const chosen = new Map<string, CopyChoice>();
  const byChoice = new Set<string>();
  const planned: PlannedDraft = { identical: false, sets: [], pairs: [] };
  const answers: PlannedAnswer[] = [];
  const key = (set: string, path: string) => `${set}\u0000${path}`;
  if (draft.identical) {
    planned.identical = true;
    ids.push(...sets.filter((s) => s.level === 'identical').map((s) => s.id));
  }
  for (const d of draft.decisions ?? []) {
    const set = byId.get(d.set);
    if (!set || check(d.decision, set)) continue;
    const decision = d.decision;
    if (decision.kind === 'copies') {
      const choice = (path: string) =>
        decision.copies.find((c) => c.path === path)?.choice ?? 'keep';
      const keep = set.copies.find((c) => choice(c.path) === 'keep');
      if (!keep) continue;
      keepers.push({ set: set.id, keep: keep.path });
      for (const c of set.copies) chosen.set(key(set.id, c.path), choice(c.path));
      byChoice.add(set.id);
      if (!ids.includes(set.id)) ids.push(set.id);
    } else
      answers.push({
        set: set.id,
        postscriptName: set.postscriptName,
        answer: decision.kind === 'keepAll' ? 'keepBoth' : 'notDuplicates',
      });
    planned.sets!.push(set.id);
  }
  const out = await plan({
    scope: { kind: 'sets', ids },
    keepers,
    removeAs: 'trash',
    moveActivations: req.moveActivations ?? false,
  });
  for (const f of out.files) {
    if (!byChoice.has(f.set)) continue;
    const c = chosen.get(key(f.set, f.path)) ?? 'keep';
    if (c === 'keep' && f.action !== 'stays') {
      f.action = 'stays';
      f.reason = 'keeper';
      f.sentence = KEPT_BY_CHOICE;
      if (f.activation) f.activation.planned = false;
    } else if (c === 'hide' && f.action === 'trash') {
      f.action = 'hide';
      if (f.sentence.startsWith('Goes to the trash'))
        f.sentence = `Is hidden${f.sentence.slice('Goes to the trash'.length)}`;
    }
  }
  for (const p of draft.pairs ?? []) {
    let part: CleanupPlan;
    try {
      part = await plan({
        scope: { kind: 'folderPair', keep: p.keep, remove: p.remove, otherLevels: false },
        moveActivations: req.moveActivations ?? false,
      });
    } catch {
      continue;
    }
    const have = new Set(out.files.map((f) => f.path));
    out.files.push(...part.files.filter((f) => !have.has(f.path)));
    for (const c of part.conflicts)
      if (!out.conflicts.some((x) => JSON.stringify(x) === JSON.stringify(c)))
        out.conflicts.push(c);
    out.sets = cap(out.sets + part.sets);
    planned.pairs!.push(structuredClone(p));
  }
  for (;;) {
    const kept = new Set(
      out.files
        .filter((f) => f.action !== 'stays')
        .flatMap((f) => [...(f.kept ? [f.kept] : []), ...(f.alsoKept ?? [])]),
    );
    let changed = false;
    for (const f of out.files)
      if (f.action !== 'stays' && kept.has(f.path)) {
        f.action = 'stays';
        f.reason = 'keptByOtherSet';
        f.sentence = KEPT_FOR_ANOTHER;
        changed = true;
      }
    if (!changed) break;
  }
  const leaving = new Set(out.files.filter((f) => f.action !== 'stays').flatMap((f) => f.faces));
  out.conflicts = out.conflicts.filter(
    (c) => leaving.has(c.otherFace) || leaving.has(c.defaultFrom),
  );
  out.frees = cap(out.files.filter((f) => f.action === 'trash').reduce((n, f) => n + f.size, 0));
  out.answers = answers;
  out.draft = planned;
  return out;
};

/** Takes out of `draft` what `planned` holds, as `CleanupDraft::take_planned`. */
export const takePlanned = (draft: CleanupDraft, planned: PlannedDraft): CleanupDraft => ({
  identical: planned.identical ? false : (draft.identical ?? false),
  decisions: (draft.decisions ?? []).filter((d) => !(planned.sets ?? []).includes(d.set)),
  pairs: (draft.pairs ?? []).filter(
    (p) => !(planned.pairs ?? []).some((q) => q.keep === p.keep && q.remove === p.remove),
  ),
});

/** The duplicates of a scope, counted, as `DuplicateSummary::from_sets`. */
export const summaryFromSets = (
  sets: DuplicateSet[],
  inScope: (path: string) => boolean,
  familyOf: (face: FaceId) => FamilyId | undefined,
): DuplicateSummary => {
  const paths = new Set<string>();
  const families = new Map<FamilyId, FamilyCopies>();
  for (const s of sets) {
    const seen = new Set<FamilyId>();
    for (const c of s.copies) {
      if (inScope(c.path)) paths.add(c.path);
      const family = c.face == null ? undefined : familyOf(c.face);
      if (family === undefined || seen.has(family)) continue;
      seen.add(family);
      const entry = families.get(family) ?? { family, copies: 0, sets: [] };
      entry.copies = Math.max(entry.copies, s.copies.length);
      entry.sets.push(s.id);
      families.set(family, entry);
    }
  }
  return {
    fonts: sets.length,
    copies: paths.size,
    families: [...families.values()].sort((a, b) => a.family - b.family),
  };
};
