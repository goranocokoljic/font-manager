#!/usr/bin/env node
// assets.mjs — publish UI screenshots so a PR comment can show them.
//
// Usage: node scripts/harness/assets.mjs --issue 61 --dir .harness/screens/issue-61
// Prints one markdown image line per PNG, ready to paste into a PR comment.
//
// The PNGs are committed to the orphan branch `harness-assets` under
// `issue-<n>/` with git plumbing (a private index file), so the working tree,
// the index and the checked-out story branch are never touched, and screenshots
// never land in a story PR or on main.

import { execFileSync } from 'node:child_process';
import { readdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { gh, parseArgs, isMain } from './lib.mjs';

const BRANCH = 'harness-assets';

function git(args, env = {}) {
  return execFileSync('git', args, { encoding: 'utf8', env: { ...process.env, ...env }, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const issue = Number(args.issue);
  const dir = resolve(String(args.dir ?? ''));
  if (!issue || !args.dir) throw new Error('usage: assets.mjs --issue <n> --dir <folder with .png files>');
  const files = readdirSync(dir).filter((f) => f.toLowerCase().endsWith('.png')).sort();
  if (!files.length) throw new Error(`no .png files in ${dir}`);

  try { git(['fetch', '-q', 'origin', BRANCH]); } catch { /* first publish: branch does not exist yet */ }
  let base = '';
  try { base = git(['rev-parse', '--verify', '-q', `refs/remotes/origin/${BRANCH}`]); } catch { base = ''; }

  const tmp = mkdtempSync(join(tmpdir(), 'go-assets-'));
  const env = { GIT_INDEX_FILE: join(tmp, 'index') };
  try {
    if (base) git(['read-tree', base], env); else git(['read-tree', '--empty'], env);
    for (const f of files) {
      const blob = git(['hash-object', '-w', join(dir, f)]);
      git(['update-index', '--add', '--cacheinfo', `100644,${blob},issue-${issue}/${f}`], env);
    }
    const tree = git(['write-tree'], env);
    const commit = git(['commit-tree', tree, ...(base ? ['-p', base] : []), '-m', `screens for #${issue}`]);
    git(['push', '-q', 'origin', `${commit}:refs/heads/${BRANCH}`]);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }

  const repo = gh(['repo', 'view', '--json', 'nameWithOwner', '-q', '.nameWithOwner']).trim();
  for (const f of files) console.log(`![${f}](https://github.com/${repo}/blob/${BRANCH}/issue-${issue}/${encodeURIComponent(f)}?raw=true)`);
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(2); }
}
