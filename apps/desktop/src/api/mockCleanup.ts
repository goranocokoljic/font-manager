// The cleanup plan of the TypeScript mock (E56.7): a copy of the Rust mock's
// `mock/cleanup.rs`. With no index, no disk and no facts, each set's keeper is the
// user's pick or the E8.1 keeper rule's (`DuplicateGroup::choose_keeper`); a system
// copy and an active copy stay (an active identical copy can move its activation),
// a file another set keeps stays, and a folder pair plans the identical copies of
// the removed side only.
import type {
  ActivationMode,
  CleanupAction,
  CleanupFile,
  DuplicateSet,
  DuplicateSetCopy,
  PlanCleanupRequest,
  StayReason,
} from './types';

const byString = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/** Version parts: digit runs as numbers, other text lower-cased. */
const parts = (v: string | null): (bigint | string)[] | null => {
  const t = v?.trim();
  if (!t) return null;
  return (t.match(/\d+|\D+/g) ?? []).map((r) => (/^\d/.test(r) ? BigInt(r) : r.toLowerCase()));
};

/** Newer first; no version last. */
const byVersion = (a: string | null, b: string | null): number => {
  const [x, y] = [parts(a), parts(b)];
  if (!x || !y) return x ? -1 : y ? 1 : 0;
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const [p, q] = [x[i], y[i]];
    if (p === undefined) return 1;
    if (q === undefined) return -1;
    if (p === q) continue;
    // A number ranks before text, as Rust's enum order (Number, Text).
    if (typeof p !== typeof q) return typeof p === 'bigint' ? 1 : -1;
    return p > q ? -1 : 1;
  }
  return 0;
};

/** The default format order by the file's extension, as `format_from_extension`. */
const formatRank = (path: string): number => {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  const order = [['otf'], ['ttf', 'ttc', 'otc'], ['woff2'], ['woff'], ['pfb', 'pfa', 'pfm']];
  const i = order.findIndex((exts) => exts.includes(ext));
  return i < 0 ? order.length : i;
};

/** The keeper of `set` among its copies outside the system folders. */
export const mockKeeper = (set: DuplicateSet): string | null => {
  const copies = set.copies
    .filter((c) => !c.system)
    .sort(
      (a, b) =>
        byVersion(a.version, b.version) ||
        formatRank(a.path) - formatRank(b.path) ||
        Number(b.inLibraryFolder) - Number(a.inLibraryFolder) ||
        byString(a.path, b.path),
    );
  return copies[0]?.path ?? null;
};

const name = (path: string) => path.split(/[\\/]/).pop() ?? path;

const SENTENCES: Partial<Record<StayReason, string>> = {
  keeper: 'Stays: it is the copy that is kept.',
  active: 'Stays: this copy is active. Deactivate it first, then move it.',
  system: 'Stays: in a system font folder. Font Fu never moves system fonts.',
  keptByOtherSet: "Stays: this file also holds another font, and that font's copy here is kept.",
  outsideScope: 'Stays: it is outside the folder or source you clean up.',
  onlyHere: 'Stays: it has no copy in the folder that is kept.',
};

const under = (path: string, dir: string) => {
  const split = (p: string) => p.split(/[\\/]+/).filter((x, i) => x !== '' || i === 0);
  const [d, p] = [split(dir), split(path)];
  return d.length <= p.length && d.every((x, i) => x === p[i]);
};

/** The planned files and the number of sets, or the refusal's message. */
export const mockPlanCleanup = (
  all: DuplicateSet[],
  req: PlanCleanupRequest,
  active: Map<string, ActivationMode>,
  inScope: (path: string) => boolean,
): { sets: number; files: CleanupFile[] } | string => {
  for (const k of req.keepers ?? []) {
    const set = all.find((s) => s.id === k.set);
    if (!set) return `${k.set} is not a duplicate set any more.`;
    if (!set.copies.some((c) => c.path === k.keep)) return `${k.keep} is not a copy of ${k.set}.`;
  }
  const keeperOf = (set: DuplicateSet) =>
    req.keepers?.find((k) => k.set === set.id)?.keep ?? mockKeeper(set);
  const [action, verb]: [CleanupAction, string] =
    req.removeAs === 'hide' ? ['hide', 'Is hidden'] : ['trash', 'Goes to the trash'];
  const row = (set: DuplicateSet, c: DuplicateSetCopy, kept: string | null): CleanupFile => ({
    path: c.path,
    size: c.size,
    hash: c.hash,
    faces: c.face === null ? [] : [c.face],
    set: set.id,
    level: set.level,
    action: 'stays',
    reason: null,
    kept,
    alsoKept: [],
    sentence: '',
    activation: null,
  });
  const scope = req.scope ?? { kind: 'library' };
  const seen = new Set<string>();
  const files: CleanupFile[] = [];
  if (scope.kind === 'folderPair') {
    if (scope.keep === scope.remove) return 'Choose two different folders.';
    const sets = new Set<string>();
    for (const set of all)
      for (const c of set.copies.filter((c) => under(c.path, scope.remove))) {
        if (seen.has(c.path)) continue;
        seen.add(c.path);
        const twin = set.copies.find((o) => o.hash === c.hash && under(o.path, scope.keep));
        const r = row(set, c, twin?.path ?? null);
        if (twin && !c.system && !active.has(c.path)) {
          r.action = action;
          r.sentence = `${verb}; ${name(twin.path)} stays in ${scope.keep}.`;
          sets.add(set.id);
        } else {
          const why: StayReason = !twin ? 'onlyHere' : c.system ? 'system' : 'active';
          r.reason = why;
          r.sentence = SENTENCES[why] ?? 'Stays.';
        }
        files.push(r);
      }
    files.sort((a, b) => byString(a.set, b.set) || byString(a.path, b.path));
    return { sets: sets.size, files };
  }
  let selected: DuplicateSet[];
  if (scope.kind === 'sets') {
    selected = [];
    for (const id of scope.ids) {
      const s = all.find((s) => s.id === id);
      if (!s) return `${id} is not a duplicate set any more.`;
      if (!selected.includes(s)) selected.push(s);
    }
  } else selected = all.filter((s) => s.copies.some((c) => inScope(c.path)));
  for (const set of selected) {
    const kept = keeperOf(set);
    for (const c of set.copies) {
      if (seen.has(c.path)) continue;
      seen.add(c.path);
      const r = row(set, c, kept);
      const keptElsewhere = c.otherSets.some((o) => {
        const s = all.find((x) => x.id === o);
        return !!s && keeperOf(s) === c.path;
      });
      const mode = active.get(c.path);
      const identical = set.level === 'identical';
      let why: StayReason | null = null;
      if (kept === c.path) why = 'keeper';
      else if (c.system || kept === null) why = 'system';
      else if (!inScope(c.path)) why = 'outsideScope';
      else if (keptElsewhere) why = 'keptByOtherSet';
      else if (mode) {
        const planned = req.moveActivations ?? false;
        if (identical) r.activation = { to: kept, mode, planned };
        if (!(planned && identical)) why = 'active';
      }
      if (why) {
        r.reason = why;
        r.sentence = SENTENCES[why] ?? 'Stays.';
      } else {
        r.action = action;
        r.sentence = `${verb}; ${kept ? name(kept) : ''} stays.`;
      }
      files.push(r);
    }
  }
  return { sets: selected.length, files };
};
