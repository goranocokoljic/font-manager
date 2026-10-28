import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
  checkCommunity,
  checkIssueForm,
  formLabels,
  isRelative,
  linkTargets,
  topLevelKeys,
} from './check.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

const FORM = "name: Bug\ndescription: It broke.\nlabels: ['bug']\nbody:\n  - type: textarea\n    attributes:\n      label: 'name: not a key'\n";

/** A repository in a temp folder with the given files; returns its root. */
function repo(files) {
  const root = mkdtempSync(join(tmpdir(), 'fm-community-'));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

const GOOD = {
  '.github/ISSUE_TEMPLATE/bug.yml': FORM,
  '.github/ISSUE_TEMPLATE/config.yml': 'blank_issues_enabled: false\n',
  '.github/pull_request_template.md': 'See [the guide](../CONTRIBUTING.md#rules).\n',
  'CONTRIBUTING.md': 'Read [the code](CODE_OF_CONDUCT.md), [data](data/charsets/) and [web](https://x.org).\n',
  'CODE_OF_CONDUCT.md': '[homepage]: https://www.contributor-covenant.org\n[guide]: ./CONTRIBUTING.md\n',
  'data/charsets/README.md': '# charsets\n',
};

test('the repository community files pass the check', () => {
  assert.deepEqual(checkCommunity(ROOT), []);
});

test('top-level keys are the lines at column 0, nested keys are not', () => {
  assert.deepEqual(topLevelKeys(FORM), ['name', 'description', 'labels', 'body']);
  assert.deepEqual(topLevelKeys('# name: comment\r\nbody:\r\n'), ['body']);
});

test('an issue form without name, description or body is reported', () => {
  assert.deepEqual(checkIssueForm(FORM), []);
  assert.deepEqual(checkIssueForm('name: x\nbody: []\n'), ['missing top-level key `description`']);
  assert.deepEqual(checkIssueForm('title: x\n'), [
    'missing top-level key `name`',
    'missing top-level key `description`',
    'missing top-level key `body`',
  ]);
});

test('a form with a harness label or an fm-meta block is reported', () => {
  assert.deepEqual(formLabels("labels: ['bug', \"type:story\"]\n"), ['bug', 'type:story']);
  assert.deepEqual(formLabels('labels:\n  - bug\n  - harness:ready\nbody:\n'), ['bug', 'harness:ready']);
  assert.deepEqual(formLabels('labels: bug, enhancement\n'), ['bug', 'enhancement']);
  assert.deepEqual(formLabels('name: x\n'), []);
  assert.deepEqual(checkIssueForm(FORM.replace("['bug']", '[type:story]')), [
    'carries the harness label `type:story`',
  ]);
  assert.deepEqual(checkIssueForm(`${FORM}      value: "<!-- fm-meta\nid: E1.1 -->"\n`), [
    'contains an `fm-meta` block (the harness would pick it up)',
  ]);
});

test('link targets come from inline links and reference definitions, not from code', () => {
  const md = [
    'A [link](a.md) and [titled](b.md "Title") and ![img](<c d.png>).',
    '[ref]: e.md',
    'Code `[x](not.md)` and',
    '```',
    '[y](fenced.md)',
    '```',
  ].join('\n');
  assert.deepEqual(linkTargets(md), ['a.md', 'b.md', 'c d.png', 'e.md']);
});

test('URLs and same-page anchors are not relative links', () => {
  assert.equal(isRelative('docs/a.md'), true);
  assert.equal(isRelative('../CONTRIBUTING.md#x'), true);
  assert.equal(isRelative('https://example.org'), false);
  assert.equal(isRelative('mailto:a@b.c'), false);
  assert.equal(isRelative('#rules'), false);
});

test('a temp repository with valid files passes', () => {
  const root = repo(GOOD);
  try {
    assert.deepEqual(checkCommunity(root), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('broken links, missing files, bad forms and no forms are all reported', () => {
  const root = repo({
    ...GOOD,
    '.github/ISSUE_TEMPLATE/bug.yml': 'name: Bug\nbody: []\n',
    'CONTRIBUTING.md': 'See [gone](docs/gone.md) and [ok](CODE_OF_CONDUCT.md) and [abs](/missing/).\n',
    'CODE_OF_CONDUCT.md': '[guide]: ./NOPE.md\n',
  });
  try {
    assert.deepEqual(checkCommunity(root), [
      '.github/ISSUE_TEMPLATE/bug.yml: missing top-level key `description`',
      'CONTRIBUTING.md: broken link `docs/gone.md`',
      'CONTRIBUTING.md: broken link `/missing/`',
      'CODE_OF_CONDUCT.md: broken link `./NOPE.md`',
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
  const empty = repo({ 'CONTRIBUTING.md': '', 'CODE_OF_CONDUCT.md': '' });
  try {
    assert.deepEqual(checkCommunity(empty), [
      '.github/ISSUE_TEMPLATE: no issue forms',
      '.github/pull_request_template.md: missing',
    ]);
  } finally {
    rmSync(empty, { recursive: true, force: true });
  }
});
