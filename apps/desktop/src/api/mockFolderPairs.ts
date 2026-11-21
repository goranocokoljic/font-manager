// The folder-pair rule of the TypeScript mock (E56.6): a copy of
// `fm_core::folder_pairs` and of the Rust mock's copy.
// `fixtures/duplicates/pair-cases.json` runs against all three, so they stay one rule.
// The mock has no files on disk, so its sides have no other files.
import { type SetFile, versionKey } from './mockDuplicates';
import type {
  DuplicateLevel,
  DuplicateScope,
  FolderPair,
  FolderPairFile,
  FolderPairSide,
  ListFolderPairFilesRequest,
  ListFolderPairsRequest,
  Paged,
  PairSide,
  ProtectedFolder,
  SharedCounts,
} from './types';

/** A folder a pair climbs no higher than and never removes as a whole. */
export interface PairRoot {
  path: string;
  kind: ProtectedFolder;
}

/** The fewest shared font files a pair needs, as `FOLDER_PAIR_MIN_SHARED`. */
export const FOLDER_PAIR_MIN_SHARED = 5;
/** The most pairs, or files, one page holds, as fm-core. */
const MAX_PAGE = 500;

/** A path by its parts, as Rust's `Path` components: `/a/b` is `['', 'a', 'b']`. */
const parts = (p: string) => p.split(/[\\/]+/).filter((x, i) => x !== '' || i === 0);
const startsWith = (p: string[], prefix: string[]) =>
  prefix.length <= p.length && prefix.every((x, i) => x === p[i]);
const same = (a: string[], b: string[]) => a.length === b.length && startsWith(a, b);
const byString = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
/** Paths compared part by part, as `PathBuf`'s order. */
const byParts = (a: string[], b: string[]) => {
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    const c = byString(a[i], b[i]);
    if (c !== 0) return c;
  }
  return a.length - b.length;
};
/** The parent folder's path, as Rust's `Path::parent` writes it. */
const parentOf = (p: string): string | null => {
  const at = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  if (at < 0) return null;
  if (at === 0) return p.length > 1 ? p.slice(0, 1) : null;
  return p.slice(0, at);
};
const nameOf = (p: string) => {
  const ps = parts(p);
  return ps[ps.length - 1];
};

interface Font {
  file: SetFile;
  path: string[];
  dir: string;
  names: { name: string; key: string | null }[];
}

type Copy = { level: DuplicateLevel; at: number } | null;

const rank = (l: DuplicateLevel) => (l === 'identical' ? 0 : l === 'sameVersion' ? 1 : 2);
const sat = (n: number) => Math.min(n, 4294967295);

class Pairs {
  fonts: Font[];
  constructor(
    files: SetFile[],
    private roots: PairRoot[],
  ) {
    this.fonts = files
      .filter((f) => f.format !== null)
      .map((file) => {
        const seen = new Set<string>();
        const names = file.faces
          .filter((f) => f.name && !seen.has(f.name) && (seen.add(f.name), true))
          .map((f) => ({ name: f.name, key: versionKey(f.version) }));
        return { file, path: parts(file.path), dir: parentOf(file.path) ?? '', names };
      })
      .sort((a, b) => byParts(a.path, b.path));
  }

  side(dir: string, other: string): number[] {
    const d = parts(dir);
    const o = parts(other);
    const skip = startsWith(o, d);
    const out: number[] = [];
    this.fonts.forEach((f, i) => {
      if (startsWith(f.path, d) && !(skip && startsWith(f.path, o))) out.push(i);
    });
    return out;
  }

  matched(xs: number[], ys: number[]): { at: number; copy: Copy }[] {
    const byHash = new Map<string, number>();
    const byName = new Map<string, { key: string | null; at: number }[]>();
    for (const y of ys) {
      const font = this.fonts[y];
      if (!byHash.has(font.file.hash)) byHash.set(font.file.hash, y);
      for (const n of font.names) {
        const list = byName.get(n.name) ?? [];
        list.push({ key: n.key, at: y });
        byName.set(n.name, list);
      }
    }
    return xs.map((x) => {
      const font = this.fonts[x];
      const same = byHash.get(font.file.hash);
      if (same !== undefined) return { at: x, copy: { level: 'identical', at: same } };
      let best: Copy = null;
      for (const n of font.names) {
        for (const other of byName.get(n.name) ?? []) {
          const level: DuplicateLevel = other.key === n.key ? 'sameVersion' : 'otherVersion';
          const better =
            best === null ||
            rank(level) < rank(best.level) ||
            (level === best.level && other.at < best.at);
          if (better) best = { level, at: other.at };
        }
      }
      return { at: x, copy: best };
    });
  }

  contained(a: string, b: string): [boolean, boolean] {
    const xs = this.side(a, b);
    const ys = this.side(b, a);
    const all = (m: { copy: Copy }[]) =>
      m.length > 0 && m.every((x) => x.copy?.level === 'identical');
    return [all(this.matched(xs, ys)), all(this.matched(ys, xs))];
  }

  inRoots(dir: string) {
    const d = parts(dir);
    return this.roots.some((r) => startsWith(d, parts(r.path)));
  }

  protectedOf(dir: string, fonts: number[]): ProtectedFolder | null {
    const d = parts(dir);
    const system =
      this.roots.some((r) => r.kind === 'systemFolder' && startsWith(d, parts(r.path))) ||
      fonts.some((i) => this.fonts[i].file.system);
    if (system) return 'systemFolder';
    return this.roots.find((r) => same(parts(r.path), d))?.kind ?? null;
  }

  climb(a: string, b: string): [string, string, boolean] {
    let [ca, cb] = this.contained(a, b);
    if (!ca && !cb) return [a, b, false];
    for (;;) {
      const pa = parentOf(a);
      const pb = parentOf(b);
      if (pa === null || pb === null) break;
      const [qa, qb] = [parts(pa), parts(pb)];
      if (
        nameOf(a) !== nameOf(b) ||
        same(qa, qb) ||
        startsWith(qa, qb) ||
        startsWith(qb, qa) ||
        !this.inRoots(pa) ||
        !this.inRoots(pb)
      )
        break;
      const [xa, xb] = this.contained(pa, pb);
      if (!((ca && xa) || (cb && xb))) break;
      [ca, cb] = [ca && xa, cb && xb];
      [a, b] = [pa, pb];
    }
    return [a, b, true];
  }

  basePairs(): [string, string][] {
    const groups = new Map<string, Set<string>>();
    const add = (key: string, dir: string) => {
      const set = groups.get(key) ?? new Set<string>();
      set.add(dir);
      groups.set(key, set);
    };
    for (const font of this.fonts) {
      add(`h:${font.file.hash}`, font.dir);
      for (const n of font.names) add(`n:${n.name}`, font.dir);
    }
    const out = new Map<string, [string, string]>();
    for (const dirs of groups.values()) {
      const list = [...dirs].sort((x, y) => byParts(parts(x), parts(y)));
      list.forEach((a, i) => {
        for (const b of list.slice(i + 1)) out.set(`${a}\u0000${b}`, [a, b]);
      });
    }
    return [...out.values()];
  }

  pairs(scope: DuplicateScope, minShared: number): FolderPair[] {
    const tops = new Map<string, [string, string, boolean]>();
    for (const [x, y] of this.basePairs()) {
      const [ca, cb, contained] = this.climb(x, y);
      const [a, b] = byParts(parts(ca), parts(cb)) <= 0 ? [ca, cb] : [cb, ca];
      tops.set(`${a}\u0000${b}\u0000${contained}`, [a, b, contained]);
    }
    const held = [...tops.values()].filter((t) => t[2]);
    const inside = (a: string, b: string) => {
      const [pa, pb] = [parts(a), parts(b)];
      return held.some(([ta, tb]) => {
        const [qa, qb] = [parts(ta), parts(tb)];
        return (
          !(same(qa, pa) && same(qb, pb)) &&
          ((startsWith(pa, qa) && startsWith(pb, qb)) || (startsWith(pa, qb) && startsWith(pb, qa)))
        );
      });
    };
    const seen = new Set<string>();
    const min = Math.max(minShared, 1);
    const shared = (s: FolderPairSide) =>
      s.shared.identical + s.shared.sameVersion + s.shared.otherVersion;
    return [...tops.values()]
      .filter(([a, b]) => {
        const key = `${a}\u0000${b}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return !inside(a, b);
      })
      .map(([a, b]) => this.pair(a, b))
      .filter((p) => Math.max(shared(p.a), shared(p.b)) >= min && this.inScope(p, scope))
      .sort(
        (x, y) => y.frees - x.frees || byString(x.a.path, y.a.path) || byString(x.b.path, y.b.path),
      );
  }

  inScope(pair: FolderPair, scope: DuplicateScope) {
    if (scope.kind === 'library') return true;
    if (scope.kind === 'source') {
      return [
        [pair.a.path, pair.b.path],
        [pair.b.path, pair.a.path],
      ].some(([p, o]) => this.side(p, o).some((i) => this.fonts[i].file.source === scope.source));
    }
    const folder = parts(scope.path);
    return [pair.a.path, pair.b.path].some((p) => startsWith(parts(p), folder));
  }

  pair(a: string, b: string): FolderPair {
    const xs = this.side(a, b);
    const ys = this.side(b, a);
    const sa = this.sideOf(a, xs, this.matched(xs, ys));
    const sb = this.sideOf(b, ys, this.matched(ys, xs));
    const bSmaller = sb.size < sa.size || (sb.size === sa.size && sb.fontFiles < sa.fontFiles);
    return { a: sa, b: sb, frees: (bSmaller ? sb : sa).identicalSize };
  }

  sideOf(dir: string, fonts: number[], m: { at: number; copy: Copy }[]): FolderPairSide {
    const shared: SharedCounts = { identical: 0, sameVersion: 0, otherVersion: 0 };
    let [total, sharedSize, identicalSize] = [0, 0, 0];
    for (const { at, copy } of m) {
      const size = this.fonts[at].file.size;
      total = sat(total + size);
      if (!copy) continue;
      sharedSize = sat(sharedSize + size);
      if (copy.level === 'identical') {
        shared.identical += 1;
        identicalSize = sat(identicalSize + size);
      } else if (copy.level === 'sameVersion') shared.sameVersion += 1;
      else shared.otherVersion += 1;
    }
    const fontFiles = fonts.length;
    return {
      path: dir,
      fontFiles,
      size: total,
      shared,
      sharedSize,
      identicalSize,
      onlyHere: fontFiles - shared.identical - shared.sameVersion - shared.otherVersion,
      otherFiles: 0,
      contained: fontFiles > 0 && shared.identical === fontFiles,
      protected: this.protectedOf(dir, fonts),
    };
  }

  files(a: string, b: string): FolderPairFile[] {
    const xs = this.side(a, b);
    const ys = this.side(b, a);
    const row =
      (side: PairSide) =>
      ({ at, copy }: { at: number; copy: Copy }): FolderPairFile => ({
        path: this.fonts[at].file.path,
        side,
        size: this.fonts[at].file.size,
        level: copy?.level ?? null,
        copy: copy ? this.fonts[copy.at].file.path : null,
      });
    return [...this.matched(xs, ys).map(row('a')), ...this.matched(ys, xs).map(row('b'))];
  }
}

const page = <T>(all: T[], offset: number, limit: number): Paged<T> => ({
  items: all.slice(offset, offset + Math.min(limit, MAX_PAGE)),
  total: all.length,
});

/** The page `req` asks for of the pairs among `files`. */
export const listFolderPairs = (
  files: SetFile[],
  roots: PairRoot[],
  req: ListFolderPairsRequest,
): Paged<FolderPair> => {
  const all = new Pairs(files, roots).pairs(
    req.scope ?? { kind: 'library' },
    req.minShared ?? FOLDER_PAIR_MIN_SHARED,
  );
  return page(all, req.page?.offset ?? 0, req.page?.limit ?? 100);
};

/** The page `req` asks for of the files of two folders; `null` when they are the same. */
export const listFolderPairFiles = (
  files: SetFile[],
  roots: PairRoot[],
  req: ListFolderPairFilesRequest,
): Paged<FolderPairFile> | null => {
  if (same(parts(req.a), parts(req.b))) return null;
  const all = new Pairs(files, roots)
    .files(req.a, req.b)
    .filter((f) => (req.side ?? null) === null || f.side === req.side)
    .filter((f) => !req.onlyHere || f.level === null);
  return page(all, req.page?.offset ?? 0, req.page?.limit ?? 100);
};
