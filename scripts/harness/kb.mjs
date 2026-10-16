#!/usr/bin/env node
// kb.mjs — the review knowledge base: lessons distilled from /fm-review findings.
//
//   node scripts/harness/kb.mjs catalog [--json]             # existing lessons, so a distiller merges instead of duplicating
//   node scripts/harness/kb.mjs apply --in distilled.json    # fold new findings in (stdin works too; --dry writes nothing)
//   node scripts/harness/kb.mjs retrieve --paths "crates/fm-core/**,crates/fm-api/**" --top 8   # HOT PATH digest
//   node scripts/harness/kb.mjs graduate --list | --id <id> [--id <id>] | --demote <id> | --retire <id>
//   node scripts/harness/kb.mjs regenerate                   # rebuild dev-docs/harness/review-rules.md
//
// Store: one JSON file per lesson under dev-docs/harness/kb/lessons/<id>.json.
// One file per lesson (not one JSONL) because two lanes distil into their own PRs
// at the same time; separate files only conflict when both touch the same lesson.
//
// Lifecycle: candidate (seen in 1 story) -> active (seen in >= 2 distinct stories;
// injected into the implementer for matching files) -> graduated (human decision;
// compiled into review-rules.md, which CLAUDE.md @-imports into EVERY agent) ->
// retired. Graduation is the only step that grows every agent's context, so it is
// never done by an agent — only by `go-harness.ps1 -GraduateOnly` or by hand.
//
// Distilled item shape (apply input is an array of these):
//   { "match_id": "<existing id or omit>", "title": "...", "category": "error-handling",
//     "rule": "<terse imperative rule>", "rationale": "E3.2 (#61): <what went wrong>",
//     "severity": "must|should", "source_issue": 61, "file_globs": ["crates/fm-core/src/index/**"] }

import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { REPO_ROOT, parseArgs, isMain } from './lib.mjs';

export const KB_DIR = resolve(REPO_ROOT, 'dev-docs', 'harness', 'kb', 'lessons');
export const RULES_MD = resolve(REPO_ROOT, 'dev-docs', 'harness', 'review-rules.md');
export const ACTIVE_THRESHOLD = 2;
export const CATEGORIES = [
  'architecture', 'error-handling', 'unsafe-ffi', 'platform-parity', 'api-contract', 'user-files',
  'correctness', 'data-integrity', 'performance', 'testing', 'determinism', 'accessibility', 'security',
];
const SEVERITIES = ['must', 'should'];
const SEV_RANK = { must: 0, should: 1 };

const nowIso = () => new Date().toISOString();
export const slug = (t) => String(t).toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 60);
export const bySeverity = (a, b) => (SEV_RANK[a.severity] ?? 9) - (SEV_RANK[b.severity] ?? 9) || (b.occurrences ?? 0) - (a.occurrences ?? 0) || a.id.localeCompare(b.id);

export function readStore(dir = KB_DIR) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter((f) => f.endsWith('.json')).map((f) => {
    try { return JSON.parse(readFileSync(join(dir, f), 'utf8')); }
    catch (e) { throw new Error(`${f} is not valid JSON: ${e.message}`, { cause: e }); }
  });
}

export function normalize(l) {
  const issues = [...new Set((l.source_issues ?? []).map(Number).filter((n) => !Number.isNaN(n)))].sort((a, b) => a - b);
  return {
    id: l.id, title: l.title ?? '', category: l.category ?? 'correctness', rule: l.rule ?? '',
    rationale: l.rationale ?? '', severity: SEVERITIES.includes(l.severity) ? l.severity : 'should',
    status: l.status ?? 'candidate', occurrences: issues.length || (l.occurrences ?? 1), source_issues: issues,
    file_globs: [...new Set(l.file_globs ?? [])], first_seen: l.first_seen ?? nowIso(), last_seen: l.last_seen ?? nowIso(),
  };
}

function writeLesson(l, dir = KB_DIR) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${l.id}.json`), JSON.stringify(normalize(l), null, 2) + '\n', 'utf8');
}

/** Pure: fold distilled items into the lessons; returns { lessons, changed:Set<id>, summary }. */
export function applyItems(lessons, items, now = nowIso()) {
  const byId = new Map(lessons.map((l) => [l.id, l]));
  const changed = new Set();
  const summary = { created: 0, merged: 0, promoted: 0 };
  for (const item of items) {
    if (item.severity && !SEVERITIES.includes(item.severity)) throw new Error(`"${item.title}": severity must be must|should`);
    const id = item.match_id || item.id || slug(item.title);
    const title = (item.title ?? '').toLowerCase().trim();
    const hit = byId.get(id) ?? lessons.find((l) => title && l.title.toLowerCase().trim() === title);
    if (hit) {
      const before = hit.status;
      Object.assign(hit, normalize({
        ...hit,
        severity: (SEV_RANK[item.severity] ?? 9) < (SEV_RANK[hit.severity] ?? 9) ? item.severity : hit.severity,
        source_issues: [...hit.source_issues, item.source_issue].filter((n) => n != null),
        file_globs: [...hit.file_globs, ...(item.file_globs ?? [])],
        last_seen: now,
      }));
      if (hit.status === 'candidate' && hit.occurrences >= ACTIVE_THRESHOLD) hit.status = 'active';
      if (before !== hit.status) summary.promoted++;
      summary.merged++;
      changed.add(hit.id);
    } else {
      const l = normalize({ ...item, id, status: 'candidate', source_issues: [item.source_issue], first_seen: now, last_seen: now });
      lessons.push(l);
      byId.set(id, l);
      summary.created++;
      changed.add(id);
    }
  }
  return { lessons, changed, summary };
}

export function globToRegExp(glob) {
  let re = '';
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i];
    if (c === '*') {
      if (glob[i + 1] === '*') { re += '.*'; i++; if (glob[i + 1] === '/') i++; }
      else re += '[^/]*';
    } else if (c === '?') re += '[^/]';
    else if ('.+^${}()|[]\\'.includes(c)) re += '\\' + c;
    else re += c;
  }
  return new RegExp('^' + re + '$');
}

/** Does a lesson's file_globs overlap any caller path glob? Shared concrete dir prefix, or a glob match either way. */
export function matchesPaths(lesson, paths) {
  if (!paths.length || !(lesson.file_globs ?? []).length) return false;
  const key = (g) => { const out = []; for (const s of g.split('/')) { if (/[*?]/.test(s)) break; out.push(s); } return out.slice(0, 3).join('/'); };
  const keys = new Set(paths.map(key).filter(Boolean));
  return lesson.file_globs.some((lg) => {
    const k = key(lg);
    if (k && [...keys].some((p) => p === k || p.startsWith(k + '/') || k.startsWith(p + '/'))) return true;
    const lre = globToRegExp(lg);
    return paths.some((p) => lre.test(p) || globToRegExp(p).test(lg));
  });
}

export function retrieveDigest(lessons, paths, top = 8) {
  const active = lessons.filter((l) => l.status === 'active');
  const pool = (paths.length ? active.filter((l) => matchesPaths(l, paths)) : active).sort(bySeverity).slice(0, top);
  if (!pool.length) return '<!-- review-KB: no relevant active lessons -->';
  return ['### Review-KB pitfalls (avoid these — distilled from past reviews in this repo)', '',
    ...pool.map((l) => `- **[${l.category}/${l.severity}]** ${l.rule}  _(${l.title}; seen in #${l.source_issues.join(', #')})_`)].join('\n');
}

export function rulesMarkdown(lessons) {
  const grad = lessons.filter((l) => l.status === 'graduated');
  const out = ['# Review rules — graduated lessons (auto-generated)', '',
    '> Generated by `node scripts/harness/kb.mjs regenerate` from `dev-docs/harness/kb/lessons/`.',
    '> Do not edit by hand. These review findings recurred often enough that a human',
    '> graduated them into always-loaded project rules. Honor them when writing code.', ''];
  if (!grad.length) {
    out.push('_No lessons have graduated yet. Active lessons are injected into the implementer', 'for matching files instead (go-cycle, build stage)._');
    return out.join('\n') + '\n';
  }
  const cats = [...CATEGORIES, ...new Set(grad.map((l) => l.category))].filter((c, i, a) => a.indexOf(c) === i);
  for (const c of cats) {
    const g = grad.filter((l) => l.category === c).sort(bySeverity);
    if (!g.length) continue;
    out.push(`## ${c}`, '');
    for (const l of g) { out.push(`- **${l.title}.** ${l.rule}`); if (l.rationale) out.push(`  - _Why:_ ${l.rationale}`); }
    out.push('');
  }
  return out.join('\n').trimEnd() + '\n';
}

function repeated(flag) {
  const a = process.argv.slice(2); const out = [];
  for (let i = 0; i < a.length; i++) if (a[i] === `--${flag}`) out.push(a[++i]);
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const cmd = args._[0];
  const lessons = readStore();
  switch (cmd) {
    case 'catalog':
      if (args.json) { process.stdout.write(JSON.stringify(lessons.sort(bySeverity)) + '\n'); return; }
      if (!lessons.length) { console.log('(KB empty — no lessons yet)'); return; }
      for (const l of lessons.sort(bySeverity)) console.log(`${l.id}  [${l.category}/${l.severity}/${l.status}]  ${l.title}\n    ${l.rule}`);
      return;
    case 'apply': {
      const raw = args.in ? readFileSync(String(args.in), 'utf8') : readFileSync(0, 'utf8');
      const items = JSON.parse(raw);
      if (!Array.isArray(items)) throw new Error('input must be a JSON array');
      const { changed, summary } = applyItems(lessons, items);
      if (!args.dry) for (const l of lessons) if (changed.has(l.id)) writeLesson(l);
      console.log(`${args.dry ? '[dry] ' : ''}applied ${items.length}: +${summary.created} new, ${summary.merged} merged, ${summary.promoted} promoted to active.`);
      return;
    }
    case 'retrieve': {
      const paths = args.paths ? String(args.paths).split(',').map((s) => s.trim()).filter(Boolean) : [];
      console.log(retrieveDigest(lessons, paths, Number(args.top ?? 8)));
      return;
    }
    case 'regenerate':
      mkdirSync(resolve(RULES_MD, '..'), { recursive: true });
      writeFileSync(RULES_MD, rulesMarkdown(lessons), 'utf8');
      console.log(`regenerated ${RULES_MD}: ${lessons.filter((l) => l.status === 'graduated').length} graduated lesson(s).`);
      return;
    case 'graduate': {
      if (args.list) {
        const act = lessons.filter((l) => l.status === 'active').sort(bySeverity);
        if (!act.length) { console.log('No active lessons awaiting graduation.'); return; }
        for (const l of act) console.log(`  ${l.id}\n    [${l.category}/${l.severity}] seen ${l.occurrences}x (#${l.source_issues.join(', #')})\n    ${l.rule}\n`);
        return;
      }
      const byId = new Map(lessons.map((l) => [l.id, l]));
      const moves = [['id', 'graduated', null], ['demote', 'active', 'graduated'], ['retire', 'retired', null]];
      let n = 0;
      for (const [flag, to, from] of moves) {
        for (const id of repeated(flag)) {
          const l = byId.get(id);
          if (!l) { console.error(`! no lesson "${id}"`); continue; }
          if (from && l.status !== from) { console.error(`! ${id} is ${l.status}, not ${from}`); continue; }
          l.status = to; l.last_seen = nowIso(); writeLesson(l); n++;
          console.log(`${to}: ${id}`);
        }
      }
      if (!n) { console.log('Nothing to do. Use --list, --id, --demote or --retire.'); return; }
      writeFileSync(RULES_MD, rulesMarkdown(readStore()), 'utf8');
      console.log('regenerated dev-docs/harness/review-rules.md');
      return;
    }
    default:
      throw new Error('usage: kb.mjs <catalog|apply|retrieve|graduate|regenerate> [...]');
  }
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(2); }
}
