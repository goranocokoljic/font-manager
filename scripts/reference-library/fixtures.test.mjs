// node --test scripts/reference-library/fixtures.test.mjs — the committed corpus under
// fixtures/fonts/ is what its manifest says, and the sniffing helpers that check it work.

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ALLOWED_LICENSES, isFontFile, relPath } from './lib.mjs';
import { FIXTURE_COUNT, declaresRfn, parseFixtures, pickPlain, serializeFixtures } from './fixtures.mjs';
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

test('fixture manifest round-trips, sorted by path', () => {
  const rows = [
    { sha256: 'b'.repeat(64), size: 2, license: 'OFL-1.1', kind: 'ttc', path: 'z/Z.ttc', origin: 'packTtc(a, b)' },
    { sha256: 'a'.repeat(64), size: 1, license: 'UFL-1.0', kind: 'ttf', path: 'a/A.ttf', origin: 'google-fonts/ufl/a/A.ttf' },
  ];
  const text = serializeFixtures(rows);
  assert.deepEqual(parseFixtures(text), [rows[1], rows[0]]);
  assert.throws(() => parseFixtures('nope\n'), /not a fixture manifest/);
});

test('pickPlain fills each quota, a few per family, and refuses a short library', () => {
  const rows = [];
  const add = (source, license, family, kind, n, size = 1000) => {
    for (let i = 0; i < n; i++) {
      const sha256 = createHash('sha256').update(`${source}${family}${kind}${i}`).digest('hex');
      const path =
        source === 'google-repo'
          ? `google-fonts/${license === 'OFL-1.1' ? 'ofl' : license === 'UFL-1.0' ? 'ufl' : 'apache'}/${family}/${kind}${i}.ttf`
          : `${source === 'fontshare' ? 'fontshare' : 'google-fonts-static'}/${family}/${kind}${i}.x`;
      rows.push({ sha256, size, license, source, path, kind });
    }
  };
  for (let f = 0; f < 10; f++) {
    add('google-repo', 'OFL-1.1', `color${f}`, 'color', 1);
    add('google-repo', 'Apache-2.0', `apache${f}`, 'ttf', 1);
    add('google-repo', 'UFL-1.0', `ufl${f}`, 'ttf', 1);
    add('google-repo', 'OFL-1.1', `var${f}`, 'variable', 4);
    add('google-static', 'OFL-1.1', `static${f}`, 'ttf', 5);
    add('google-static', 'OFL-1.1', `staticb${f}`, 'ttf', 5);
    add('fontshare', 'OFL-1.1', `fs${f}`, 'woff2', 3);
    add('fontshare', 'OFL-1.1', `fs${f}`, 'woff', 3);
    add('fontshare', 'OFL-1.1', `fs${f}`, 'otf', 3);
    add('fontshare', 'OFL-1.1', `fs${f}`, 'ttf', 3);
  }
  for (let f = 0; f < 200; f++) add('google-repo', 'OFL-1.1', `fill${f}`, 'ttf', 2);
  add('google-repo', 'OFL-1.1', 'huge', 'ttf', 5, 10_000_000);
  const kindOf = (r) => r.kind;

  // 35 variable fonts are wanted but only 10 families x 1 per family exist.
  assert.throws(() => pickPlain(rows, kindOf), /quota variable: only 10 of 35/);
  for (let f = 10; f < 40; f++) add('google-repo', 'OFL-1.1', `var${f}`, 'variable', 1);

  const picked = pickPlain(rows, kindOf);
  assert.equal(picked.length, FIXTURE_COUNT - 3);
  assert.equal(new Set(picked.map((p) => p.row.sha256)).size, picked.length, 'no file twice');
  assert.ok(!picked.some((p) => p.row.path.includes('/huge/')), 'size cap holds');
  const count = (q) => picked.filter((p) => p.quota === q).length;
  assert.equal(count('color'), 4);
  assert.equal(count('variable'), 35);
  const fillFamilies = picked.filter((p) => p.quota === 'fill').map((p) => p.row.path.split('/')[2]);
  assert.equal(new Set(fillFamilies).size, fillFamilies.length, 'one filler per family');
  assert.deepEqual(pickPlain(rows, kindOf), picked, 'deterministic');
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
