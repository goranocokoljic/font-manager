#!/usr/bin/env node
// issue.mjs — the GitHub issue operations go-harness and the go-cycle skill share.
//
//   node scripts/harness/issue.mjs meta <n>                       # JSON: title, fm-meta, criteria, labels
//   node scripts/harness/issue.mjs numeric <n>                    # exit 0 if a criterion needs the bench job
//   node scripts/harness/issue.mjs tick <n> --index 2 --evidence "test index::search::filters_by_lang; CI run 123"
//   node scripts/harness/issue.mjs pending <n> --index 3 --note "verified locally, pending E9.1: 41 ms p95"
//   node scripts/harness/issue.mjs block <n> --reason "CI failed 3x on macos" --tried "fixed path case; fixed CRLF"
//
// `tick` flips exactly one `- [ ]` under "## Acceptance criteria" to `- [x]` and
// posts a one-line evidence comment. It refuses an empty evidence string: a box
// is ticked only when the verification step named its proof (test name, CI job,
// bench number, screenshot). `pending` leaves the box open, appends a marker to
// the line and comments the local numbers — the state for numeric criteria
// before the bench job (E9.1) exists. `block` swaps harness:in-progress for
// harness:blocked and comments why and what was tried.

import { writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gh, ghJson, parseMeta, parseCriteria, labelNames, parseArgs, isMain } from './lib.mjs';

const PENDING_MARK = '_(verified locally, pending E9.1)_';

function view(n) {
  return ghJson(['issue', 'view', String(n), '--json', 'number,title,body,labels,state']);
}

/** Pure: return the body with criterion `index` ticked (or marked pending). */
export function editCriterion(body, index, mode) {
  const crit = parseCriteria(body).find((c) => c.index === index);
  if (!crit) throw new Error(`no acceptance criterion #${index}`);
  const eol = body.includes('\r\n') ? '\r\n' : '\n';
  const lines = body.split(/\r?\n/);
  if (mode === 'tick') {
    lines[crit.line] = lines[crit.line].replace(/\[ \]/, '[x]').replace(` ${PENDING_MARK}`, '');
  } else if (!lines[crit.line].includes(PENDING_MARK)) {
    lines[crit.line] = `${lines[crit.line]} ${PENDING_MARK}`;
  }
  return { body: lines.join(eol), crit };
}

function setBody(n, body) {
  const dir = mkdtempSync(join(tmpdir(), 'go-harness-'));
  const f = join(dir, 'body.md');
  try {
    writeFileSync(f, body, 'utf8');
    gh(['issue', 'edit', String(n), '--body-file', f]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function comment(n, text) {
  gh(['issue', 'comment', String(n), '--body-file', '-'], { input: text });
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const [cmd, num] = args._;
  const n = Number(num);
  if (!cmd || !Number.isInteger(n)) throw new Error('usage: issue.mjs <meta|numeric|tick|pending|block> <issue> [...]');

  const issue = view(n);
  switch (cmd) {
    case 'meta': {
      const out = {
        number: issue.number, title: issue.title, state: issue.state, labels: labelNames(issue),
        meta: parseMeta(issue.body), criteria: parseCriteria(issue.body),
      };
      process.stdout.write(JSON.stringify(out, null, 2) + '\n');
      return;
    }
    case 'numeric': {
      const hits = parseCriteria(issue.body).filter((c) => c.numeric);
      for (const c of hits) console.log(`#${c.index}: ${c.text}`);
      process.exit(hits.length ? 0 : 1);
    }
    case 'tick':
    case 'pending': {
      const index = Number(args.index);
      const text = String(args[cmd === 'tick' ? 'evidence' : 'note'] ?? '').trim();
      if (!text || text === 'true') throw new Error(`${cmd} needs --${cmd === 'tick' ? 'evidence' : 'note'} "<proof>"`);
      const { body, crit } = editCriterion(issue.body, index, cmd);
      if (body !== issue.body) setBody(n, body);
      const head = cmd === 'tick' ? `✓ Criterion ${index} verified` : `◐ Criterion ${index} verified locally, pending E9.1`;
      comment(n, `${head}: ${crit.text}\n\nEvidence: ${text}`);
      console.log(`${cmd}: #${n} criterion ${index}`);
      return;
    }
    case 'block': {
      const reason = String(args.reason ?? '').trim() || 'no reason given';
      const tried = String(args.tried ?? '').trim();
      gh(['issue', 'edit', String(n), '--add-label', 'harness:blocked', '--remove-label', 'harness:in-progress'], { allowFail: true });
      comment(n, [
        '**go-harness stopped on this story — a human must look.**', '',
        `Why: ${reason}`, tried ? `\nWhat it tried: ${tried}` : '', '',
        'To retry: fix the cause, remove `harness:blocked`, and run the lane again.',
      ].join('\n'));
      console.log(`blocked: #${n}`);
      return;
    }
    default:
      throw new Error(`unknown command ${cmd}`);
  }
}

if (isMain(import.meta.url)) {
  try { main(); } catch (e) { console.error(e.message); process.exit(2); }
}
