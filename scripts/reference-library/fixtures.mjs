#!/usr/bin/env node
// fixtures.mjs — pick the 200-font test corpus under fixtures/fonts/ from a built
// reference library. Run by hand when the corpus needs to change; the output is
// committed and tests read fonts only from there.
//
//   node scripts/reference-library/fixtures.mjs --from "$FM_REFERENCE_LIBRARY" [--out fixtures/fonts]
//
// The pick is deterministic for a given library manifest: candidates are taken in
// sha256 order, a few per family, under a size cap. Besides plain fonts it adds
//   ttc     two faces of one OFL family packed into a collection (packTtc)
//   type1   Inconsolata zi4 PFB from CTAN (OFL, see its README)
//   broken  the first 1,500 bytes of an OFL font: a truncated file
// Derived files (ttc, broken) only come from families without a Reserved Font Name.
// Every font folder gets the family's licence file. fixtures/fonts/manifest.tsv lists
// sha256, size, licence, kind and origin of each font.

import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { downloadFile, extractZip, parseManifest } from './lib.mjs';
import { classify, packTtc, sfntTables } from './sfnt.mjs';

export const FIXTURE_COUNT = 200;
export const FIXTURE_HEADER = 'sha256\tsize\tlicense\tkind\tpath\torigin';
const TYPE1_URL = 'https://mirrors.ctan.org/fonts/inconsolata.zip';
const TYPE1_ENTRY = 'inconsolata/type1/Inconsolata-zi4r.pfb';
const TYPE1_LICENSE = 'inconsolata/doc/OFL.txt';

export function serializeFixtures(rows) {
  const sorted = [...rows].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return [FIXTURE_HEADER, ...sorted.map((r) => [r.sha256, r.size, r.license, r.kind, r.path, r.origin].join('\t'))].join('\n') + '\n';
}

export function parseFixtures(text) {
  const lines = text.split(/\r?\n/).filter((l) => l.trim() !== '');
  if (lines[0] !== FIXTURE_HEADER) throw new Error('not a fixture manifest');
  return lines.slice(1).map((l) => {
    const [sha256, size, license, kind, path, origin] = l.split('\t');
    return { sha256, size: Number(size), license, kind, path, origin };
  });
}

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

/** Family key of a library path: the folder that holds the family's fonts. */
function familyOf(row) {
  const p = row.path.split('/');
  if (row.source === 'google-repo') return p[2];
  return p[1]; // google-fonts-static/<dir>/..., fontshare/<slug>/...
}

function licenseFileFor(lib, row) {
  const p = row.path.split('/');
  let dirs;
  if (row.source === 'google-repo') dirs = [join(lib, ...p.slice(0, 3))];
  else if (row.source === 'google-static') {
    dirs = ['ofl', 'apache', 'ufl'].map((l) => join(lib, 'google-fonts', l, p[1]));
  } else {
    // fontshare/<slug>/<Name>_Complete/License/OFL.txt
    const root = join(lib, 'fontshare', p[1]);
    dirs = existsSync(root) ? readdirSync(root).map((d) => join(root, d, 'License')) : [];
  }
  for (const d of dirs) {
    for (const name of ['OFL.txt', 'LICENSE.txt', 'UFL.txt']) {
      if (existsSync(join(d, name))) return join(d, name);
    }
  }
  return null;
}

/** True when the copyright header (above the licence body) declares a Reserved Font Name. */
export function declaresRfn(licenseText) {
  const header = licenseText.split(/This Font Software is licensed/i)[0];
  return /Reserved\s+Font\s+Names?/i.test(header);
}
const hasRfn = (licenseFile) => declaresRfn(readFileSync(licenseFile, 'utf8'));

/**
 * Choose the plain fonts. Each quota: a filter, a size cap, how many, and how many
 * per family. Quotas run in order; a file is chosen once.
 */
export function pickPlain(rows, kindOf) {
  const quotas = [
    { name: 'color', n: 4, cap: 700_000, perFamily: 1, ok: (r) => r.source === 'google-repo' && kindOf(r) === 'color' },
    { name: 'apache', n: 8, cap: 250_000, perFamily: 1, ok: (r) => r.license === 'Apache-2.0' },
    { name: 'ufl', n: 2, cap: 400_000, perFamily: 2, ok: (r) => r.license === 'UFL-1.0' },
    { name: 'variable', n: 35, cap: 250_000, perFamily: 1, ok: (r) => r.source === 'google-repo' && kindOf(r) === 'variable' },
    { name: 'static', n: 40, cap: 120_000, perFamily: 2, ok: (r) => r.source === 'google-static' && kindOf(r) === 'ttf' },
    { name: 'woff2', n: 12, cap: 120_000, perFamily: 2, ok: (r) => r.source === 'fontshare' && kindOf(r) === 'woff2' },
    { name: 'woff', n: 6, cap: 120_000, perFamily: 1, ok: (r) => r.source === 'fontshare' && kindOf(r) === 'woff' },
    { name: 'otf', n: 12, cap: 150_000, perFamily: 2, ok: (r) => r.source === 'fontshare' && kindOf(r) === 'otf' },
    { name: 'fontshare-ttf', n: 4, cap: 150_000, perFamily: 1, ok: (r) => r.source === 'fontshare' && kindOf(r) === 'ttf' },
    { name: 'fill', n: Infinity, cap: 120_000, perFamily: 1, ok: (r) => r.source === 'google-repo' && r.license === 'OFL-1.1' && kindOf(r) === 'ttf' },
  ];
  const ordered = [...rows].sort((a, b) => (a.sha256 < b.sha256 ? -1 : 1));
  const chosen = [];
  const seenSha = new Set();
  const target = FIXTURE_COUNT - 3; // ttc, type1, broken
  for (const q of quotas) {
    const perFam = new Map();
    let taken = 0;
    for (const r of ordered) {
      if (chosen.length >= target || taken >= q.n) break;
      if (r.size > q.cap || seenSha.has(r.sha256) || !q.ok(r)) continue;
      const fam = `${r.source}:${familyOf(r)}`;
      if ((perFam.get(fam) ?? 0) >= q.perFamily) continue;
      perFam.set(fam, (perFam.get(fam) ?? 0) + 1);
      seenSha.add(r.sha256);
      chosen.push({ row: r, quota: q.name });
      taken++;
    }
    if (q.n !== Infinity && taken < q.n) throw new Error(`quota ${q.name}: only ${taken} of ${q.n} candidates`);
  }
  if (chosen.length !== target) throw new Error(`only ${chosen.length} plain fonts found, need ${target}`);
  return chosen;
}

async function main() {
  const args = process.argv.slice(2);
  const arg = (flag, dflt) => {
    const i = args.indexOf(flag);
    return i >= 0 ? args[i + 1] : dflt;
  };
  const lib = resolve(arg('--from', process.env.FM_REFERENCE_LIBRARY ?? ''));
  const out = resolve(arg('--out', 'fixtures/fonts'));
  // A font enters the corpus only with its licence file (a few google/fonts families
  // ship none).
  const licensed = new Map();
  const rows = parseManifest(readFileSync(join(lib, 'manifest.tsv'), 'utf8')).filter((r) => {
    const key = `${r.source}:${familyOf(r)}`;
    if (!licensed.has(key)) licensed.set(key, licenseFileFor(lib, r) !== null);
    return licensed.get(key);
  });

  const kinds = new Map();
  const kindOf = (r) => {
    if (!kinds.has(r.path)) kinds.set(r.path, r.size > 700_000 ? 'big' : classify(readFileSync(join(lib, ...r.path.split('/')))));
    return kinds.get(r.path);
  };
  const plain = pickPlain(rows, kindOf);

  // Start from an empty corpus folder, keeping the README.
  for (const e of existsSync(out) ? readdirSync(out) : []) {
    if (e !== 'README.md') rmSync(join(out, e), { recursive: true, force: true });
  }
  const fixtures = [];
  const put = (rel, buf, license, licenseFile, origin) => {
    const file = join(out, ...rel.split('/'));
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, buf);
    const lic = join(dirname(file), licenseFile.split(/[\\/]/).pop());
    if (!existsSync(lic)) copyFileSync(licenseFile, lic);
    fixtures.push({ sha256: sha256(buf), size: buf.length, license, kind: classify(buf), path: rel, origin });
  };

  for (const { row } of plain) {
    const licenseFile = licenseFileFor(lib, row);
    if (!licenseFile) throw new Error(`no licence file for ${row.path}`);
    const name = row.path.split('/').pop();
    put(`${row.source}/${familyOf(row)}/${name}`, readFileSync(join(lib, ...row.path.split('/'))), row.license, licenseFile, row.path);
  }

  // Derived files come from one OFL family with two small static TTFs and no RFN.
  const byFamily = new Map();
  for (const r of [...rows].sort((a, b) => (a.path < b.path ? -1 : 1))) {
    if (r.source !== 'google-repo' || r.license !== 'OFL-1.1' || r.size > 80_000 || kindOf(r) !== 'ttf') continue;
    const fam = familyOf(r);
    if (!byFamily.has(fam)) byFamily.set(fam, []);
    byFamily.get(fam).push(r);
  }
  const usedFamilies = new Set(plain.map((p) => `${p.row.source}:${familyOf(p.row)}`));
  const donor = [...byFamily.entries()].find(([fam, list]) => {
    if (list.length < 2 || usedFamilies.has(`google-repo:${fam}`)) return false;
    const lic = licenseFileFor(lib, list[0]);
    return lic && !hasRfn(lic);
  });
  if (!donor) throw new Error('no OFL family without a Reserved Font Name for the derived fixtures');
  const [fam, list] = donor;
  const lic = licenseFileFor(lib, list[0]);
  const [a, b] = list.slice(0, 2).map((r) => readFileSync(join(lib, ...r.path.split('/'))));
  if (!sfntTables(a) || !sfntTables(b)) throw new Error('donor fonts are not usable');
  put(`derived/${fam}/${fam}.ttc`, packTtc([a, b]), 'OFL-1.1', lic, `packTtc(${list[0].path}, ${list[1].path})`);
  put(`derived/${fam}/${fam}-truncated.ttf`, a.subarray(0, 1500), 'OFL-1.1', lic, `first 1500 bytes of ${list[0].path}`);

  // Type 1 from CTAN.
  const zipFile = join(lib, '.state', 'ctan-inconsolata.zip');
  if (!existsSync(zipFile)) await downloadFile(TYPE1_URL, zipFile);
  const tmp = join(lib, '.state', 'ctan-inconsolata');
  rmSync(tmp, { recursive: true, force: true });
  extractZip(readFileSync(zipFile), tmp);
  const pfb = readFileSync(join(tmp, ...TYPE1_ENTRY.split('/')));
  put('type1/inconsolata/Inconsolata-zi4r.pfb', pfb, 'OFL-1.1', join(tmp, ...TYPE1_LICENSE.split('/')), `${TYPE1_URL}#${TYPE1_ENTRY}`);

  writeFileSync(join(out, 'manifest.tsv'), serializeFixtures(fixtures));
  const byKind = {};
  for (const f of fixtures) byKind[f.kind] = (byKind[f.kind] ?? 0) + 1;
  const total = fixtures.reduce((s, f) => s + f.size, 0);
  console.log(`fixtures: ${fixtures.length} fonts, ${(total / 1e6).toFixed(1)} MB, ${JSON.stringify(byKind)}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error(`fixtures: ${err.stack ?? err}`);
    process.exit(1);
  });
}
