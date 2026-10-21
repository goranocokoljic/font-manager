#!/usr/bin/env node
// Writes the `assigned` count of every block in data/unicode-blocks/blocks.json:
// the code points of the block whose General_Category is not Cn (unassigned),
// as Unicode defines "assigned" (surrogates and private use count).
//
// Usage: node scripts/unicode-blocks-assigned.mjs <DerivedGeneralCategory.txt>
// The file comes from https://www.unicode.org/Public/<version>/ucd/extracted/
// of the version in blocks.json's `unicodeVersion`.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const source = process.argv[2];
if (!source) {
  console.error('usage: node scripts/unicode-blocks-assigned.mjs <DerivedGeneralCategory.txt>');
  process.exit(2);
}

const blocksPath = fileURLToPath(new URL('../data/unicode-blocks/blocks.json', import.meta.url));
const text = readFileSync(source, 'utf8');
const json = readFileSync(blocksPath, 'utf8');
const version = JSON.parse(json).unicodeVersion;
if (!text.startsWith(`# DerivedGeneralCategory-${version}.txt`)) {
  console.error(`${source} is not DerivedGeneralCategory-${version}.txt`);
  process.exit(1);
}

// Assigned ranges: every line whose category is not Cn.
const ranges = [];
for (const line of text.split('\n')) {
  const m = /^([0-9A-F]+)(?:\.\.([0-9A-F]+))?\s*;\s*(\w+)/.exec(line);
  if (m && m[3] !== 'Cn') {
    ranges.push([parseInt(m[1], 16), parseInt(m[2] ?? m[1], 16)]);
  }
}

const assigned = (first, last) =>
  ranges.reduce((n, [a, b]) => n + Math.max(0, Math.min(b, last) - Math.max(a, first) + 1), 0);

const out = json.replace(
  /\{ "first": "([0-9A-F]+)", "last": "([0-9A-F]+)",(?: "assigned": \d+,)? "name"/g,
  (_, first, last) =>
    `{ "first": "${first}", "last": "${last}", "assigned": ${assigned(parseInt(first, 16), parseInt(last, 16))}, "name"`,
);
writeFileSync(blocksPath, out);
