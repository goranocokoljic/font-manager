// node --test scripts/reference-library/fixtures.test.mjs — the committed corpus under
// fixtures/fonts/ is what its manifest says, and the sniffing helpers that check it work.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ALLOWED_LICENSES, isFontFile, relPath } from './lib.mjs';
import { FIXTURE_COUNT, declaresRfn, parseFixtures } from './fixtures.mjs';
import { classify, packTtc, sfntTables, sniffFormat, ttcFaces } from './sfnt.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fixtures', 'fonts');
const MAX_BYTES = 30_000_000;

function walk(dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

/** A tiny but well-formed sfnt with the given tables (4 bytes each). */
function sfnt(tags, version = 0x00010000) {
  const head = Buffer.alloc(12 + tags.length * 16);
  head.writeUInt32BE(version, 0);
  head.writeUInt16BE(tags.length, 4);
  const body = Buffer.alloc(tags.length * 4, 7);
  tags.forEach((t, i) => {
    const rec = 12 + i * 16;
    head.write(t.padEnd(4, ' '), rec, 'latin1');
    head.writeUInt32BE(head.length + i * 4, rec + 8);
    head.writeUInt32BE(4, rec + 12);
  });
  return Buffer.concat([head, body]);
}

describe('sfnt sniffing', () => {
  test('classifies containers, tables and broken files', () => {
    assert.equal(classify(sfnt(['cmap', 'glyf', 'head'])), 'ttf');
    assert.equal(classify(sfnt(['CFF ', 'cmap'], 0x4f54544f)), 'otf');
    assert.equal(classify(sfnt(['cmap', 'fvar', 'glyf'])), 'variable');
    assert.equal(classify(sfnt(['COLR', 'CPAL', 'cmap'])), 'color');
    assert.equal(classify(Buffer.from('wOF2\0\0\0\0')), 'woff2');
    assert.equal(classify(Buffer.from('wOFF\0\0\0\0')), 'woff');
    const pfb = Buffer.concat([Buffer.from([0x80, 0x01, 0, 0, 0, 0]), Buffer.from('%!PS-AdobeFont-1.0: X 001')]);
    assert.equal(sniffFormat(pfb), 'type1');
    assert.equal(classify(Buffer.from('%!FontType1-1.0: X')), 'type1');
    const font = sfnt(['cmap', 'glyf', 'head']);
    assert.equal(classify(font.subarray(0, 20)), 'broken', 'truncated table directory');
    assert.equal(classify(font.subarray(0, font.length - 2)), 'broken', 'table past the end');
    assert.equal(classify(Buffer.from('<html>not a font</html>')), 'broken');
    assert.equal(classify(Buffer.alloc(0)), 'broken');
  });

  test('packTtc keeps every face and its tables', () => {
    const a = sfnt(['cmap', 'glyf', 'head']);
    const b = sfnt(['cmap', 'glyf', 'head', 'name']);
    const ttc = packTtc([a, b]);
    assert.equal(classify(ttc), 'ttc');
    assert.deepEqual(ttcFaces(ttc), [sfntTables(a), sfntTables(b)]);
    assert.equal(ttcFaces(ttc.subarray(0, 30)), null);
  });
});

test('a Reserved Font Name counts only when the copyright header declares it', () => {
  const body =
    'This Font Software is licensed under the SIL Open Font License.\n' +
    '"Reserved Font Name" refers to any names specified as such';
  assert.equal(declaresRfn(`Copyright 2023 The Agdasima Project Authors\n\n${body}`), false);
  assert.equal(declaresRfn(`Copyright 2010 Foo, with Reserved Font Name "Bar".\n\n${body}`), true);
});

describe('fixtures/fonts', () => {
  const rows = parseFixtures(readFileSync(join(ROOT, 'manifest.tsv'), 'utf8'));

  test(`holds exactly ${FIXTURE_COUNT} fonts, all listed in the manifest`, () => {
    assert.equal(rows.length, FIXTURE_COUNT);
    const onDisk = walk(ROOT).filter((f) => isFontFile(f)).map((f) => relPath(ROOT, f)).sort();
    assert.deepEqual(onDisk, rows.map((r) => r.path).sort());
  });

  test('every file matches its hash, size and kind', () => {
    for (const r of rows) {
      const buf = readFileSync(join(ROOT, ...r.path.split('/')));
      assert.equal(buf.length, r.size, r.path);
      assert.equal(createHash('sha256').update(buf).digest('hex'), r.sha256, r.path);
      assert.equal(classify(buf), r.kind, r.path);
    }
  });

  test('includes variable, color, TTC, WOFF2, broken and Type 1 fonts', () => {
    const kinds = new Set(rows.map((r) => r.kind));
    for (const k of ['variable', 'color', 'ttc', 'woff2', 'broken', 'type1', 'ttf', 'otf', 'woff']) {
      assert.ok(kinds.has(k), `no ${k} font in the corpus`);
    }
  });

  test('is OFL, Apache or UFL only, with the licence file beside every font', () => {
    for (const r of rows) {
      assert.ok(ALLOWED_LICENSES.has(r.license), `${r.path}: ${r.license}`);
      const dir = join(ROOT, ...r.path.split('/').slice(0, -1));
      const lic = ['OFL.txt', 'LICENSE.txt', 'UFL.txt'].find((n) => existsSync(join(dir, n)));
      assert.ok(lic, `${r.path}: no licence file`);
    }
  });

  test(`stays under ${MAX_BYTES / 1e6} MB`, () => {
    const total = rows.reduce((s, r) => s + r.size, 0);
    assert.ok(total < MAX_BYTES, `${total} bytes`);
  });
});
