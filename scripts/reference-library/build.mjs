#!/usr/bin/env node
// build.mjs — CLI of scripts/build-reference-library.sh (see lib.mjs for the sources).
//
//   node scripts/reference-library/build.mjs [--dest DIR] [--sources a,b] [--jobs N]
//                                            [--limit N] [--refresh] [--verify]
//
//   --dest      library folder; default $FM_REFERENCE_LIBRARY, else
//               ~/.cache/font-manager/reference-library
//   --sources   comma list of google-repo, google-static, fontshare (default: all)
//   --jobs      parallel downloads (default 8)
//   --limit     at most N families per source (quick trial runs)
//   --refresh   fetch the listings again instead of reading the cached copies
//   --verify    re-check every google-repo file against the pinned commit
//
// GITHUB_TOKEN, when set, is sent to the GitHub API for the single tree listing call.
// Exit code 0 when every file is in place, 1 when some failed (run again to resume).

import { homedir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, SOURCES } from './lib.mjs';

export function parseCli(argv, env = process.env) {
  const opts = { sources: SOURCES, jobs: 8, limit: 0, refresh: false, verify: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const value = () => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${a} needs a value`);
      return v;
    };
    if (a === '--dest') opts.dest = value();
    else if (a === '--sources') opts.sources = value().split(',').map((s) => s.trim()).filter(Boolean);
    else if (a === '--jobs') opts.jobs = positiveInt(a, value());
    else if (a === '--limit') opts.limit = positiveInt(a, value());
    else if (a === '--refresh') opts.refresh = true;
    else if (a === '--verify') opts.verify = true;
    else if (a === '--help' || a === '-h') opts.help = true;
    else throw new Error(`unknown argument: ${a}`);
  }
  opts.dest = resolve(
    opts.dest ?? env.FM_REFERENCE_LIBRARY ?? join(homedir(), '.cache', 'font-manager', 'reference-library'),
  );
  for (const s of opts.sources) if (!SOURCES.includes(s)) throw new Error(`unknown source: ${s}`);
  return opts;
}

function positiveInt(flag, v) {
  const n = Number(v);
  if (!Number.isInteger(n) || n < 1) throw new Error(`${flag} needs a positive integer`);
  return n;
}

async function main() {
  let opts;
  try {
    opts = parseCli(process.argv.slice(2));
  } catch (err) {
    console.error(`build-reference-library: ${err.message}`);
    process.exit(2);
  }
  if (opts.help) {
    console.log('usage: scripts/build-reference-library.sh [--dest DIR] [--sources google-repo,google-static,fontshare] [--jobs N] [--limit N] [--refresh] [--verify]');
    return;
  }
  console.log(`reference library: ${opts.dest}`);
  const started = Date.now();
  const { rows, failures, stats } = await build({
    ...opts,
    githubToken: process.env.GITHUB_TOKEN,
    log: (m) => console.log(m),
  });
  const bySource = {};
  for (const r of rows) bySource[r.source] = (bySource[r.source] ?? 0) + 1;
  const secs = ((Date.now() - started) / 1000).toFixed(1);
  console.log(`fonts: ${rows.length} ${JSON.stringify(bySource)}`);
  console.log(`downloads: ${stats.downloads}, listings fetched: ${stats.listings}, ${secs} s`);
  console.log(`manifest: ${join(opts.dest, 'manifest.tsv')}`);
  if (failures.length > 0) {
    for (const f of failures.slice(0, 20)) {
      console.error(`failed [${f.source}] ${JSON.stringify(f.item)}: ${f.error}`);
    }
    if (failures.length > 20) console.error(`... and ${failures.length - 20} more`);
    console.error(`${failures.length} item(s) failed; run the script again to resume.`);
    process.exit(1);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error(`build-reference-library: ${err.stack ?? err}`);
    process.exit(1);
  });
}
