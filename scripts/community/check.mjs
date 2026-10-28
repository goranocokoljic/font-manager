#!/usr/bin/env node
// Checks the community files (E36.5), from `pnpm lint`:
//
// - every issue form in .github/ISSUE_TEMPLATE/ has the top-level keys GitHub requires
//   (`name`, `description`, `body`), and none carries an `fm-meta` block or a harness
//   label: the harness picks up issues by `type:story` and `fm-meta`, and a public
//   issue made from a form must never become a story;
// - every relative link in CONTRIBUTING.md, CODE_OF_CONDUCT.md and the PR template
//   points to a file or folder that exists.
//
//   node scripts/community/check.mjs
//
// No YAML or Markdown dependency: top-level keys are the lines at column 0, and links are
// `[text](target)` and `[label]: target` outside code.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

export const TEMPLATE_DIR = '.github/ISSUE_TEMPLATE';

/** The Markdown files whose relative links must resolve, relative to the root. */
export const LINKED_FILES = [
  'CONTRIBUTING.md',
  'CODE_OF_CONDUCT.md',
  '.github/pull_request_template.md',
];

/** Keys GitHub requires at the top of an issue form. */
export const REQUIRED_KEYS = ['name', 'description', 'body'];

/** Labels that make the harness treat an issue as its own. */
const HARNESS_LABEL = /^(type:story|type:epic|harness:)/;

/** The top-level keys of a YAML document, in order: the `key:` lines at column 0. */
export function topLevelKeys(yaml) {
  const keys = [];
  for (const line of yaml.split(/\r?\n/)) {
    const m = /^([A-Za-z_][\w-]*)\s*:/.exec(line);
    if (m) keys.push(m[1]);
  }
  return keys;
}

/** The labels of an issue form: `labels: [a, 'b']` or a `- item` list below `labels:`. */
export function formLabels(yaml) {
  const lines = yaml.split(/\r?\n/);
  const at = lines.findIndex((l) => /^labels\s*:/.test(l));
  if (at < 0) return [];
  const unquote = (s) => s.trim().replace(/^['"]|['"]$/g, '');
  const inline = lines[at].replace(/^labels\s*:/, '').trim();
  if (inline.startsWith('[')) {
    return inline
      .replace(/^\[|\]$/g, '')
      .split(',')
      .map(unquote)
      .filter(Boolean);
  }
  if (inline) return inline.split(',').map(unquote).filter(Boolean);
  const labels = [];
  for (const line of lines.slice(at + 1)) {
    const m = /^\s+-\s+(.+)$/.exec(line);
    if (!m) break;
    labels.push(unquote(m[1]));
  }
  return labels;
}

/** Problems with one issue form, as messages; empty when it is fine. */
export function checkIssueForm(yaml) {
  const problems = [];
  const keys = topLevelKeys(yaml);
  for (const key of REQUIRED_KEYS) {
    if (!keys.includes(key)) problems.push(`missing top-level key \`${key}\``);
  }
  if (/fm-meta/.test(yaml)) problems.push('contains an `fm-meta` block (the harness would pick it up)');
  for (const label of formLabels(yaml)) {
    if (HARNESS_LABEL.test(label)) problems.push(`carries the harness label \`${label}\``);
  }
  return problems;
}

/** Markdown with fenced code blocks and code spans blanked out. */
function withoutCode(markdown) {
  return markdown
    .replace(/^ {0,3}(```|~~~)[^\n]*\n[\s\S]*?^ {0,3}\1[^\n]*$/gm, '')
    .replace(/`+[^`\n]*`+/g, '');
}

/** Link targets in Markdown: inline `[text](target)` and reference `[label]: target`. */
export function linkTargets(markdown) {
  const text = withoutCode(markdown);
  const targets = [];
  for (const m of text.matchAll(/\]\(\s*(?:<([^>\n]+)>|([^)\s]+))(?:\s+["'][^"']*["'])?\s*\)/g)) {
    targets.push(m[1] ?? m[2]);
  }
  for (const m of text.matchAll(/^ {0,3}\[[^\]]+\]:\s*<?(\S+?)>?(?:\s|$)/gm)) targets.push(m[1]);
  return targets;
}

/** Whether a link target is relative to the file (not a URL, not a same-page anchor). */
export function isRelative(target) {
  return !/^[a-z][a-z0-9+.-]*:/i.test(target) && !target.startsWith('#') && !target.startsWith('//');
}

/**
 * The relative links of a Markdown file that point to nothing. `file` is relative to
 * `root`; a target starting with `/` is relative to `root`, any other to the file's folder.
 */
export function brokenLinks(root, file) {
  const markdown = readFileSync(join(root, file), 'utf8');
  const broken = [];
  for (const target of linkTargets(markdown).filter(isRelative)) {
    let path = target.replace(/[#?].*$/, '');
    try {
      path = decodeURIComponent(path);
    } catch {
      // keep the raw target: a stray `%` is a broken link anyway
    }
    const full = path.startsWith('/') ? join(root, path) : resolve(root, dirname(file), path);
    if (!existsSync(full)) broken.push(target);
  }
  return broken;
}

/** Every problem in the community files under `root`, as `path: message` lines (`/` paths). */
export function checkCommunity(root) {
  const problems = [];
  const dir = join(root, TEMPLATE_DIR);
  const forms = existsSync(dir)
    ? readdirSync(dir).filter((f) => /\.ya?ml$/.test(f) && !/^config\.ya?ml$/.test(f))
    : [];
  if (forms.length === 0) problems.push(`${TEMPLATE_DIR}: no issue forms`);
  for (const form of forms.sort()) {
    const path = `${TEMPLATE_DIR}/${form}`;
    for (const p of checkIssueForm(readFileSync(join(root, path), 'utf8'))) problems.push(`${path}: ${p}`);
  }
  for (const file of LINKED_FILES) {
    if (!existsSync(join(root, file))) {
      problems.push(`${file}: missing`);
      continue;
    }
    for (const target of brokenLinks(root, file)) problems.push(`${file}: broken link \`${target}\``);
  }
  return problems;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const problems = checkCommunity(ROOT);
  for (const p of problems) console.error(p);
  if (problems.length > 0) {
    console.error(`community files: ${problems.length} problem(s)`);
    process.exit(1);
  }
  console.log('community files: ok');
}
