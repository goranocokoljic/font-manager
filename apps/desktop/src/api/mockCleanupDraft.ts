// The Duplicates module's overview and draft plan in the TypeScript mock (E56.11): a
// copy of `fm_api::cleanup_draft`. The overview counts a plan as
// `DuplicatesOverview::from_plan`; a draft change follows `CleanupDraft::apply`.
import type {
  ApiError,
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
