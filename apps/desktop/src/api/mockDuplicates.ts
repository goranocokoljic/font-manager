// The duplicate-set rule of the TypeScript mock (E56.1): a copy of
// `fm_core::duplicate_sets` and of the Rust mock's copy.
// `fixtures/duplicates/set-cases.json` runs against all three, so they stay one rule.
import type {
  DuplicateLevel,
  DuplicateScope,
  DuplicateSet,
  DuplicateSetCopy,
  FaceId,
  FontFormat,
  ListDuplicateSetsRequest,
  Paged,
  SourceId,
} from './types';

/** One file the library shows, with its faces in collection order. */
export interface SetFile {
  path: string;
  size: number;
  hash: string;
  format: FontFormat | null;
  source: SourceId;
  inLibraryFolder: boolean;
  system: boolean;
  faces: { face: FaceId; name: string; version: string | null }[];
}

/** The most sets one page holds, as fm-core. */
const MAX_PAGE = 500;

/** A digit run by value; past u64 it is u64::MAX, as fm-core parses it. */
const U64_MAX = 18446744073709551615n;
const number = (run: string) => {
  const n = BigInt(run);
  return (n > U64_MAX ? U64_MAX : n).toString();
};

/**
 * The value two versions are compared by: trimmed, digit runs as numbers, other
 * text lower-cased; `null` for no version (its own value). As a string, so equal
 * keys are equal strings.
 */
export const versionKey = (version: string | null): string | null => {
  const v = version?.trim();
  if (!v) return null;
  return (v.match(/\d+|\D+/g) ?? [])
    .map((run) => (/^\d/.test(run) ? `#${number(run)}` : `t${run.toLowerCase()}`))
    .join('\u0000');
};

/** Whether `file` is in `scope`; a folder takes its subfolders, compared by path parts. */
const inScope = (file: SetFile, scope: DuplicateScope): boolean => {
  if (scope.kind === 'library') return true;
  if (scope.kind === 'source') return file.source === scope.source;
  const parts = (p: string) => p.split(/[\\/]+/).filter((x, i) => x !== '' || i === 0);
  const folder = parts(scope.path);
  const path = parts(file.path);
  return folder.length <= path.length && folder.every((x, i) => x === path[i]);
};

interface Draft {
  id: string;
  level: DuplicateLevel;
  name: string | null;
  members: { file: number; face: number | null }[];
}

const byString = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/**
 * Every duplicate set among `files` with a copy in `scope`, sets by PostScript name
 * (case-insensitive, then exact, then id), copies by path.
 */
export const duplicateSets = (files: SetFile[], scope: DuplicateScope): DuplicateSet[] => {
  const byName = new Map<string, { file: number; face: number }[]>();
  files.forEach((file, fi) => {
    const seen = new Set<string>();
    file.faces.forEach((face, k) => {
      if (face.name && !seen.has(face.name)) {
        seen.add(face.name);
        byName.set(face.name, [...(byName.get(face.name) ?? []), { file: fi, face: k }]);
      }
    });
  });
  const drafts: Draft[] = [];
  for (const name of [...byName.keys()].sort(byString)) {
    const members = byName.get(name)!;
    if (new Set(members.map((m) => files[m.file]!.hash)).size < 2) continue;
    const versions = new Set(
      members.map((m) => versionKey(files[m.file]!.faces[m.face]!.version) ?? '\u0001none'),
    );
    drafts.push({
      id: `name:${name}`,
      level: versions.size === 1 ? 'sameVersion' : 'otherVersion',
      name,
      members,
    });
  }
  const named = drafts.slice();
  const byHash = new Map<string, number[]>();
  files.forEach((file, fi) => byHash.set(file.hash, [...(byHash.get(file.hash) ?? []), fi]));
  const holds = (d: Draft, members: number[]) =>
    members.every((fi) => d.members.some((m) => m.file === fi));
  for (const hash of [...byHash.keys()].sort(byString)) {
    const members = byHash.get(hash)!;
    if (members.length < 2) continue;
    const names = new Set(files[members[0]!]!.faces.map((f) => f.name).filter((n) => n));
    const covered =
      names.size > 0 &&
      [...names].every((name) => named.some((d) => d.name === name && holds(d, members)));
    if (covered) continue;
    const first = (fi: number) => {
      const k = files[fi]!.faces.findIndex((f) => f.name);
      return k < 0 ? null : k;
    };
    const withName = members.find((fi) => first(fi) != null);
    drafts.push({
      id: `identical:${hash}`,
      level: 'identical',
      name: withName == null ? null : files[withName]!.faces[first(withName)!]!.name,
      members: members.map((fi) => ({
        file: fi,
        face: first(fi) ?? (files[fi]!.faces.length ? 0 : null),
      })),
    });
  }
  const setsOf = new Map<number, string[]>();
  for (const d of drafts)
    for (const m of d.members) setsOf.set(m.file, [...(setsOf.get(m.file) ?? []), d.id]);
  const out = drafts
    .filter((d) => d.members.some((m) => inScope(files[m.file]!, scope)))
    .map((d): DuplicateSet => {
      const members = d.members
        .slice()
        .sort((a, b) => byString(files[a.file]!.path, files[b.file]!.path));
      const copies = members.map((m): DuplicateSetCopy => {
        const file = files[m.file]!;
        const face = m.face == null ? undefined : file.faces[m.face];
        return {
          path: file.path,
          size: file.size,
          hash: file.hash,
          face: face?.face ?? null,
          version: face?.version ?? null,
          format: file.format,
          source: file.source,
          inLibraryFolder: file.inLibraryFolder,
          system: file.system,
          inScope: inScope(file, scope),
          identicalWith: members
            .filter((o) => o.file !== m.file && files[o.file]!.hash === file.hash)
            .map((o) => files[o.file]!.path),
          otherSets: (setsOf.get(m.file) ?? []).filter((id) => id !== d.id).sort(byString),
          // The facts come from fm-core's index; the sample has none (E56.2).
          facts: null,
        };
      });
      return {
        id: d.id,
        level: d.level,
        postscriptName: d.name,
        formatsDiffer: new Set(copies.map((c) => c.format)).size > 1,
        copies,
        // The keeper policy needs the facts of fm-core's index (E56.4).
        recommendation: null,
      };
    });
  const key = (s: DuplicateSet) => s.postscriptName;
  return out.sort((a, b) => {
    const na = key(a);
    const nb = key(b);
    if ((na == null) !== (nb == null)) return na == null ? 1 : -1;
    return (
      byString(na?.toLowerCase() ?? '', nb?.toLowerCase() ?? '') ||
      byString(na ?? '', nb ?? '') ||
      byString(a.id, b.id)
    );
  });
};

/**
 * The bytes removing the recommended copies frees, as `DuplicateSet::frees`: the
 * recommendation's `remove` copies; without one (the mocks) every copy outside the
 * system folders but the largest.
 */
export const setFrees = (set: DuplicateSet): number => {
  const cap = (n: number) => Math.min(n, 0xffff_ffff);
  if (set.recommendation)
    return cap(
      set.recommendation.remove.reduce(
        (n, r) => n + (set.copies.find((c) => c.path === r.path)?.size ?? 0),
        0,
      ),
    );
  const sizes = set.copies.filter((c) => !c.system).map((c) => c.size);
  const total = cap(sizes.reduce((n, x) => n + x, 0));
  return Math.max(0, total - Math.max(0, ...sizes));
};

/** Whether `set` passes `req.filter`, as `DuplicateSetFilter::matches`. */
const matches = (set: DuplicateSet, req: ListDuplicateSetsRequest, decided: Set<string>) => {
  const f = req.filter ?? {};
  const r = set.recommendation;
  return (
    (!f.levels?.length || f.levels.includes(set.level)) &&
    (f.format == null || set.copies.some((c) => c.format === f.format)) &&
    (f.recommendation == null ||
      (r != null &&
        (f.recommendation === 'keepBoth'
          ? r.outcome === 'keepBoth'
          : r.outcome === 'keeper' && r.remove.length > 0))) &&
    !(f.undecided && decided.has(set.id))
  );
};

/**
 * The page `req` asks for of the sets among `files`, filtered and sorted as
 * `DuplicateSet::page`; `decided` holds the sets the cleanup draft has decided.
 */
export const listDuplicateSets = (
  files: SetFile[],
  req: ListDuplicateSetsRequest,
  decided: Set<string> = new Set(),
): Paged<DuplicateSet> => {
  const all = duplicateSets(files, req.scope ?? { kind: 'library' }).filter((s) =>
    matches(s, req, decided),
  );
  const name = (s: DuplicateSet) => s.postscriptName ?? '';
  const byName = (a: DuplicateSet, b: DuplicateSet) =>
    byString(name(a), name(b)) || byString(a.id, b.id);
  // A stable sort, as Rust's `sort_by_cached_key`; by name the listing's own order.
  if (req.sort === 'frees') all.sort((a, b) => setFrees(b) - setFrees(a) || byName(a, b));
  else if (req.sort === 'copies')
    all.sort((a, b) => b.copies.length - a.copies.length || byName(a, b));
  const offset = req.page?.offset ?? 0;
  const limit = Math.min(req.page?.limit ?? 100, MAX_PAGE);
  return { items: all.slice(offset, offset + limit), total: all.length };
};
