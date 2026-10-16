// lib.mjs — the logic behind scripts/build-reference-library.sh.
//
// The reference library is ~10,000 freely licensed fonts that the performance
// budgets are measured against (FM_REFERENCE_LIBRARY). It is built from three
// sources, all limited to the OFL, Apache 2.0 and Ubuntu Font Licence:
//
//   google-repo    every font file under ofl/, apache/ and ufl/ of the google/fonts
//                  repository at a pinned commit, with the licence file of each family
//   google-static  the static instances fonts.google.com serves for variable families
//                  whose repository directory is under one of those licences
//   fontshare      every Fontshare family with license_type "sil_ofl", all formats
//
// Idempotent: a file that is already complete is never downloaded again, listings
// are cached in <dest>/.state, so a second run makes no network request and writes
// a byte-identical manifest. Resumable: downloads go to "<file>.part" and continue
// with an HTTP Range request after an interruption.

import { createHash } from 'node:crypto';
import {
  createReadStream, existsSync, mkdirSync, readFileSync, readdirSync,
  renameSync, rmSync, statSync, writeFileSync,
} from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';
import { open } from 'node:fs/promises';
import { pipeline } from 'node:stream/promises';
import { crc32, inflateRawSync } from 'node:zlib';

/** google/fonts commit the library is pinned to (2026-10-16). Bump deliberately. */
export const GOOGLE_FONTS_REF = '47d54ca1ebb14fd864695f1d39dc734f39017c33';

export const DEFAULT_ENDPOINTS = {
  githubApi: 'https://api.github.com',
  githubRaw: 'https://raw.githubusercontent.com',
  googleFonts: 'https://fonts.google.com',
  fontshareApi: 'https://api.fontshare.com',
};

export const SOURCES = ['google-repo', 'google-static', 'fontshare'];

/** The licence directories of google/fonts and the licence id each one stands for. */
export const LICENSE_DIRS = { ofl: 'OFL-1.1', apache: 'Apache-2.0', ufl: 'UFL-1.0' };
export const ALLOWED_LICENSES = new Set(Object.values(LICENSE_DIRS));

const FONT_EXT = /\.(ttf|otf|ttc|otc|woff2?|pfb|pfa)$/i;
const LICENSE_FILE = /^(OFL|LICENSE|UFL)\.txt$/i;
const MANIFEST_HEADER = 'sha256\tsize\tlicense\tsource\tpath';

export function isFontFile(path) {
  return FONT_EXT.test(path);
}

/** Path relative to `root` with forward slashes, the form every manifest uses. */
export function relPath(root, file) {
  return relative(root, file).split(sep).join('/');
}

// ---------------------------------------------------------------- hashing

export async function sha256File(file) {
  const hash = createHash('sha256');
  await pipeline(createReadStream(file), hash);
  return hash.digest('hex');
}

/** The git blob id of a buffer: how the google/fonts tree listing names file content. */
export function gitBlobSha(buf) {
  return createHash('sha1').update(`blob ${buf.length}\0`).update(buf).digest('hex');
}

// ---------------------------------------------------------------- manifest

/** Manifest rows → TSV text: header, rows sorted by path, LF line ends. Deterministic. */
export function serializeManifest(rows) {
  const sorted = [...rows].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  const lines = sorted.map((r) => [r.sha256, r.size, r.license, r.source, r.path].join('\t'));
  return [MANIFEST_HEADER, ...lines].join('\n') + '\n';
}

export function parseManifest(text) {
  const lines = text.split(/\r?\n/).filter((l) => l.trim() !== '');
  if (lines[0] !== MANIFEST_HEADER) throw new Error('not a reference-library manifest');
  return lines.slice(1).map((line) => {
    const [sha256, size, license, source, path] = line.split('\t');
    return { sha256, size: Number(size), license, source, path };
  });
}

/** Write a file so that a reader never sees half of it (temp file + rename). */
export function writeAtomic(file, text) {
  mkdirSync(dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  writeFileSync(tmp, text);
  renameSync(tmp, file);
}

function readJson(file, fallback) {
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    return fallback;
  }
}

// ---------------------------------------------------------------- network

export class HttpError extends Error {
  constructor(url, status) {
    super(`HTTP ${status} for ${url}`);
    this.status = status;
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function withRetry(fn, { retries = 3, delayMs = 500 } = {}) {
  let last;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      return await fn();
    } catch (err) {
      last = err;
      // A 4xx other than 408/429 will not get better by asking again.
      if (err instanceof HttpError && err.status < 500 && ![408, 429].includes(err.status)) break;
      if (attempt < retries) await sleep(delayMs * 2 ** attempt);
    }
  }
  throw last;
}

/**
 * Download `url` to `dest`, resumably. Bytes land in `dest.part`; when that file
 * exists from an interrupted run the request asks for the rest with a Range header
 * and appends. `dest` appears only when the body is complete.
 */
export async function downloadFile(url, dest, { headers = {}, retries = 3, delayMs = 500 } = {}) {
  const part = `${dest}.part`;
  mkdirSync(dirname(dest), { recursive: true });
  await withRetry(
    async () => {
      const have = existsSync(part) ? statSync(part).size : 0;
      const res = await fetch(url, {
        headers: have > 0 ? { ...headers, Range: `bytes=${have}-` } : headers,
        redirect: 'follow',
      });
      if (res.status === 416) {
        // The part file already holds the whole body.
        await res.body?.cancel();
      } else if (res.ok) {
        const append = res.status === 206 && have > 0;
        // Each chunk is written before the next is read, so the bytes of a body cut
        // off halfway are on disk for the next attempt to continue from.
        const fh = await open(part, append ? 'a' : 'w');
        try {
          for await (const chunk of res.body) await fh.write(chunk);
        } finally {
          await fh.close();
        }
        const total = expectedTotal(res, append ? have : 0);
        const got = statSync(part).size;
        if (total !== null && got !== total) throw new Error(`short body for ${url}: ${got}/${total}`);
      } else {
        await res.body?.cancel();
        throw new HttpError(url, res.status);
      }
    },
    { retries, delayMs },
  );
  renameSync(part, dest);
}

function expectedTotal(res, offset) {
  const range = res.headers.get('content-range');
  const m = range && /\/(\d+)$/.exec(range);
  if (m) return Number(m[1]);
  const len = res.headers.get('content-length');
  return len === null ? null : offset + Number(len);
}

async function fetchText(url, { headers = {}, retries = 3, delayMs = 500 } = {}) {
  return withRetry(
    async () => {
      const res = await fetch(url, { headers, redirect: 'follow' });
      if (!res.ok) {
        await res.body?.cancel();
        throw new HttpError(url, res.status);
      }
      return res.text();
    },
    { retries, delayMs },
  );
}

/** fonts.google.com prefixes its JSON with ")]}'" against JSON hijacking. */
export function parseGoogleJson(text) {
  return JSON.parse(text.replace(/^\)\]\}'\s*/, ''));
}

/** Fetch a listing once and keep it in the state folder; later runs read the copy. */
async function cachedListing(ctx, name, url, opts = {}) {
  const file = join(ctx.stateDir, name);
  if (!ctx.refresh && existsSync(file)) return readFileSync(file, 'utf8');
  const text = await fetchText(url, { ...ctx.net, ...opts });
  writeAtomic(file, text);
  ctx.stats.listings++;
  return text;
}

/** Run `worker` over `items` with at most `jobs` in flight; collect failures. */
export async function runPool(items, jobs, worker) {
  const failures = [];
  let next = 0;
  async function lane() {
    while (next < items.length) {
      const item = items[next++];
      try {
        await worker(item);
      } catch (err) {
        failures.push({ item, error: err.message ?? String(err) });
      }
    }
  }
  await Promise.all(Array.from({ length: Math.max(1, Math.min(jobs, items.length)) }, lane));
  return failures;
}

// ---------------------------------------------------------------- sources

/** google/fonts tree listing → the files to fetch: fonts plus each family's licence file. */
export function planGoogleRepo(tree) {
  const files = [];
  for (const e of tree.tree ?? []) {
    if (e.type !== 'blob') continue;
    const parts = e.path.split('/');
    if (parts.length < 3 || !(parts[0] in LICENSE_DIRS)) continue;
    const name = parts[parts.length - 1];
    if (!isFontFile(name) && !LICENSE_FILE.test(name)) continue;
    files.push({ path: e.path, sha: e.sha, size: e.size, license: LICENSE_DIRS[parts[0]] });
  }
  return files;
}

/** "Roboto Condensed" → "robotocondensed": the google/fonts directory naming rule. */
export function familyDir(family) {
  return family.toLowerCase().replace(/[^a-z0-9]/g, '');
}

/**
 * Variable families of fonts.google.com whose repository directory sits under an
 * allowed licence → [{ family, dir, license }]. A family with no matching directory
 * is skipped: its licence cannot be shown, so it does not enter the library.
 */
export function planGoogleStatic(metadata, repoFiles) {
  const dirs = new Map();
  for (const f of repoFiles) {
    const [lic, dir] = f.path.split('/');
    dirs.set(dir, LICENSE_DIRS[lic]);
  }
  const out = [];
  for (const fam of metadata.familyMetadataList ?? []) {
    if (!fam.axes || fam.axes.length === 0) continue;
    const dir = familyDir(fam.family);
    const license = dirs.get(dir);
    if (license) out.push({ family: fam.family, dir, license });
  }
  return out.sort((a, b) => (a.dir < b.dir ? -1 : a.dir > b.dir ? 1 : 0));
}

/** The static font files of one fonts.google.com download listing. */
export function staticFileRefs(listing) {
  return (listing.manifest?.fileRefs ?? []).filter(
    (r) => r.filename.startsWith('static/') && isFontFile(r.filename) && !r.filename.includes('..'),
  );
}

/** Fontshare listing pages → the OFL families only. */
export function planFontshare(pages) {
  const out = [];
  for (const page of pages) {
    for (const f of page.fonts ?? []) {
      if (f.license_type === 'sil_ofl' && /^[A-Za-z0-9-]+$/.test(f.slug)) out.push(f.slug);
    }
  }
  return [...new Set(out)].sort();
}

async function buildGoogleRepo(ctx) {
  const { githubApi, githubRaw } = ctx.endpoints;
  const ref = ctx.googleRef;
  const headers = ctx.githubToken ? { Authorization: `Bearer ${ctx.githubToken}` } : {};
  const text = await cachedListing(
    ctx,
    `google-tree-${ref}.json`,
    `${githubApi}/repos/google/fonts/git/trees/${ref}?recursive=1`,
    { headers: { ...headers, Accept: 'application/vnd.github+json' } },
  );
  const tree = JSON.parse(text);
  if (tree.truncated) throw new Error('google/fonts tree listing is truncated');
  let files = planGoogleRepo(tree);
  if (ctx.limit) {
    const keep = new Set([...new Set(files.map((f) => f.path.split('/').slice(0, 2).join('/')))].slice(0, ctx.limit));
    files = files.filter((f) => keep.has(f.path.split('/').slice(0, 2).join('/')));
  }
  ctx.repoFiles = files;
  const root = join(ctx.dest, 'google-fonts');
  const failures = await runPool(files, ctx.jobs, async (f) => {
    const dest = join(root, ...f.path.split('/'));
    if (existsSync(dest)) {
      if (!ctx.verify || gitBlobSha(readFileSync(dest)) === f.sha) return;
      rmSync(dest);
    }
    await downloadFile(`${githubRaw}/google/fonts/${ref}/${f.path}`, dest, ctx.net);
    if (gitBlobSha(readFileSync(dest)) !== f.sha) {
      rmSync(dest);
      throw new Error(`content of ${f.path} does not match the pinned commit`);
    }
    ctx.stats.downloads++;
  });
  for (const f of files) {
    if (isFontFile(f.path)) ctx.licenseOf.set(`google-fonts/${f.path}`, ['google-repo', f.license]);
  }
  return failures;
}

async function buildGoogleStatic(ctx) {
  const { googleFonts } = ctx.endpoints;
  if (!ctx.repoFiles) throw new Error('google-static needs the google-repo source (licence lookup)');
  const metadata = parseGoogleJson(
    await cachedListing(ctx, 'google-metadata.json', `${googleFonts}/metadata/fonts`),
  );
  let families = planGoogleStatic(metadata, ctx.repoFiles);
  if (ctx.limit) families = families.slice(0, ctx.limit);
  const jobs = [];
  const listFailures = await runPool(families, ctx.jobs, async (fam) => {
    const listing = parseGoogleJson(
      await cachedListing(
        ctx,
        join('google-static', `${fam.dir}.json`),
        `${googleFonts}/download/list?family=${encodeURIComponent(fam.family)}`,
      ),
    );
    for (const ref of staticFileRefs(listing)) {
      const name = ref.filename.slice('static/'.length).split('/').join('_');
      jobs.push({ url: ref.url, rel: `google-fonts-static/${fam.dir}/${name}`, license: fam.license });
    }
  });
  const failures = await runPool(jobs, ctx.jobs, async (j) => {
    const dest = join(ctx.dest, ...j.rel.split('/'));
    if (existsSync(dest)) return;
    await downloadFile(j.url, dest, ctx.net);
    ctx.stats.downloads++;
  });
  for (const j of jobs) ctx.licenseOf.set(j.rel, ['google-static', j.license]);
  return [...listFailures, ...failures];
}

/**
 * Extract a zip archive into `dir` (stored and deflated entries, no zip64: Fontshare
 * archives are a few MB). In-process, so the script needs no unzip tool on any OS.
 */
export function extractZip(buf, dir) {
  let eocd = -1;
  for (let i = buf.length - 22; i >= Math.max(0, buf.length - 22 - 0xffff); i--) {
    if (buf.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error('not a zip archive');
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  const written = [];
  for (let n = 0; n < count; n++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error('corrupt zip central directory');
    const method = buf.readUInt16LE(p + 10);
    const crc = buf.readUInt32LE(p + 16);
    const csize = buf.readUInt32LE(p + 20);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const local = buf.readUInt32LE(p + 42);
    const name = buf.toString('utf8', p + 46, p + 46 + nameLen);
    p += 46 + nameLen + extraLen + commentLen;
    if (name.endsWith('/')) continue;
    const parts = name.split(/[\\/]/);
    if (name.startsWith('/') || /^[A-Za-z]:/.test(name) || parts.includes('..')) {
      throw new Error(`unsafe path in zip: ${name}`);
    }
    if (buf.readUInt32LE(local) !== 0x04034b50) throw new Error('corrupt zip local header');
    const start = local + 30 + buf.readUInt16LE(local + 26) + buf.readUInt16LE(local + 28);
    const raw = buf.subarray(start, start + csize);
    let data;
    if (method === 0) data = raw;
    else if (method === 8) data = inflateRawSync(raw);
    else throw new Error(`unsupported zip method ${method} for ${name}`);
    if (crc32(data) !== crc) throw new Error(`crc mismatch in zip entry ${name}`);
    const out = join(dir, ...parts);
    mkdirSync(dirname(out), { recursive: true });
    writeFileSync(out, data);
    written.push(parts.join('/'));
  }
  return written;
}

async function buildFontshare(ctx) {
  const { fontshareApi } = ctx.endpoints;
  const pages = [];
  for (let offset = 0, more = true; more; offset += 100) {
    const page = JSON.parse(
      await cachedListing(
        ctx,
        join('fontshare', `fonts-${offset}.json`),
        `${fontshareApi}/v2/fonts?offset=${offset}&limit=100`,
      ),
    );
    pages.push(page);
    more = page.has_more === true && (page.fonts ?? []).length > 0;
  }
  let slugs = planFontshare(pages);
  if (ctx.limit) slugs = slugs.slice(0, ctx.limit);
  const failures = await runPool(slugs, ctx.jobs, async (slug) => {
    const dir = join(ctx.dest, 'fontshare', slug);
    const done = join(dir, '.complete');
    if (existsSync(done)) return;
    const zip = join(ctx.stateDir, 'fontshare', `${slug}.zip`);
    if (!existsSync(zip)) {
      await downloadFile(`${fontshareApi}/v2/fonts/download/${slug}`, zip, ctx.net);
      ctx.stats.downloads++;
    }
    rmSync(dir, { recursive: true, force: true });
    extractZip(readFileSync(zip), dir);
    writeFileSync(done, '');
  });
  for (const slug of slugs) ctx.fontshareDirs.add(`fontshare/${slug}/`);
  return failures;
}

// ---------------------------------------------------------------- manifest pass

function walk(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (e.isFile() && isFontFile(e.name)) out.push(p);
  }
  return out;
}

/**
 * Hash every font file the selected sources own and return manifest rows. A hash is
 * reused from the cache when the file's size and mtime have not changed, so a re-run
 * over 10,000 files takes seconds.
 */
async function manifestRows(ctx) {
  const cacheFile = join(ctx.stateDir, 'hash-cache.json');
  const cache = readJson(cacheFile, {});
  const next = {};
  const rows = [];
  const files = [
    ...walk(join(ctx.dest, 'google-fonts')),
    ...walk(join(ctx.dest, 'google-fonts-static')),
    ...walk(join(ctx.dest, 'fontshare')),
  ];
  await runPool(files, ctx.jobs, async (file) => {
    const path = relPath(ctx.dest, file);
    let owner = ctx.licenseOf.get(path);
    if (!owner && [...ctx.fontshareDirs].some((d) => path.startsWith(d))) {
      owner = ['fontshare', 'OFL-1.1'];
    }
    if (!owner) return; // left over from another pin or a source not selected this run
    const st = statSync(file);
    const hit = cache[path];
    const sha256 =
      hit && hit.size === st.size && hit.mtimeMs === st.mtimeMs ? hit.sha256 : await sha256File(file);
    next[path] = { size: st.size, mtimeMs: st.mtimeMs, sha256 };
    rows.push({ sha256, size: st.size, license: owner[1], source: owner[0], path });
  });
  writeAtomic(cacheFile, JSON.stringify(next));
  return rows;
}

// ---------------------------------------------------------------- entry point

/**
 * Build or update the library at `opts.dest`. Returns { rows, failures, stats }.
 * The manifest (<dest>/manifest.tsv) lists every complete font file, also after a
 * partial run, so a failed run still leaves a usable, truthful manifest.
 */
export async function build(opts) {
  const ctx = {
    dest: opts.dest,
    stateDir: join(opts.dest, '.state'),
    endpoints: { ...DEFAULT_ENDPOINTS, ...(opts.endpoints ?? {}) },
    googleRef: opts.googleRef ?? GOOGLE_FONTS_REF,
    githubToken: opts.githubToken,
    sources: opts.sources ?? SOURCES,
    jobs: opts.jobs ?? 8,
    limit: opts.limit ?? 0,
    refresh: opts.refresh ?? false,
    verify: opts.verify ?? false,
    net: { retries: opts.retries ?? 3, delayMs: opts.retryDelayMs ?? 500 },
    log: opts.log ?? (() => {}),
    stats: { downloads: 0, listings: 0 },
    licenseOf: new Map(),
    fontshareDirs: new Set(),
    repoFiles: null,
  };
  for (const s of ctx.sources) if (!SOURCES.includes(s)) throw new Error(`unknown source: ${s}`);
  mkdirSync(ctx.stateDir, { recursive: true });

  const failures = [];
  const steps = { 'google-repo': buildGoogleRepo, 'google-static': buildGoogleStatic, fontshare: buildFontshare };
  // google-static needs the repo listing for its licence lookup; the listing is cached.
  if (ctx.sources.includes('google-static') && !ctx.sources.includes('google-repo')) {
    throw new Error('google-static needs the google-repo source');
  }
  for (const source of SOURCES.filter((s) => ctx.sources.includes(s))) {
    ctx.log(`${source}: start`);
    const f = await steps[source](ctx);
    for (const x of f) failures.push({ source, ...x });
    ctx.log(`${source}: done, ${f.length} failure(s)`);
  }

  const rows = await manifestRows(ctx);
  writeAtomic(join(ctx.dest, 'manifest.tsv'), serializeManifest(rows));
  return { rows, failures, stats: ctx.stats };
}
