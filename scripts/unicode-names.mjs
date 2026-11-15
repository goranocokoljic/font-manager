#!/usr/bin/env node
// Writes data/unicode-names/names.json from UnicodeData.txt of the Unicode Character
// Database: the Name property of every named code point (E50.9). Ranges whose names are
// derived from the code point (CJK and Tangut ideographs, Hangul syllables) become
// `ranges`; controls (`<control>`), surrogates and private use have no name and are left
// out.
//
// Usage: node scripts/unicode-names.mjs <UnicodeData.txt> <version>
// The file comes from https://www.unicode.org/Public/<version>/ucd/UnicodeData.txt.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const [source, version] = process.argv.slice(2);
if (!source || !/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('usage: node scripts/unicode-names.mjs <UnicodeData.txt> <version like 17.0.0>');
  process.exit(2);
}

/** Range labels of UnicodeData.txt and how their names are derived. */
const DERIVED = [
  [/^CJK Ideograph/, { prefix: 'CJK UNIFIED IDEOGRAPH-' }],
  [/^Tangut Ideograph/, { prefix: 'TANGUT IDEOGRAPH-' }],
  [/^Hangul Syllable$/, { rule: 'hangul' }],
];
/** Range labels with no name. */
const NAMELESS = /Surrogate|Private Use/;

const names = [];
const ranges = [];
let open = null;
for (const line of readFileSync(source, 'utf8').split('\n')) {
  if (!line.trim()) continue;
  const [hex, name] = line.split(';');
  const range = /^<(.+), (First|Last)>$/.exec(name);
  if (range) {
    if (range[2] === 'First') {
      open = { first: hex, label: range[1] };
      continue;
    }
    if (!open || open.label !== range[1]) throw new Error(`unpaired range at ${hex}`);
    const derived = DERIVED.find(([re]) => re.test(range[1]));
    if (derived) ranges.push({ first: open.first, last: hex, ...derived[1] });
    else if (!NAMELESS.test(range[1])) throw new Error(`unknown range ${range[1]}`);
    open = null;
    continue;
  }
  if (name.startsWith('<')) continue; // <control>
  names.push([hex, name]);
}

const out = fileURLToPath(new URL('../data/unicode-names/names.json', import.meta.url));
const body = [
  '{',
  '  "$schema": "./names.schema.json",',
  `  "unicodeVersion": "${version}",`,
  `  "source": "https://www.unicode.org/Public/${version}/ucd/UnicodeData.txt",`,
  '  "ranges": [',
  ranges.map((r) => `    ${JSON.stringify(r).replace(/,/g, ', ').replace(/:/g, ': ')}`).join(',\n'),
  '  ],',
  '  "names": {',
  names.map(([hex, n]) => `    "${hex}": ${JSON.stringify(n)}`).join(',\n'),
  '  }',
  '}',
  '',
].join('\n');
writeFileSync(out, body);
console.log(`${names.length} names, ${ranges.length} ranges → ${out}`);
