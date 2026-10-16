// node --test scripts/reference-library/lib.test.mjs — tests for build-reference-library.sh.
// No network: a local HTTP server plays GitHub, fonts.google.com and Fontshare.

import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { crc32 } from 'node:zlib';

import {
  build, downloadFile, extractZip, familyDir, gitBlobSha, parseGoogleJson, parseManifest,
  planFontshare, planGoogleRepo, planGoogleStatic, serializeManifest, staticFileRefs,
} from './lib.mjs';
import { parseCli } from './build.mjs';

const tmp = () => mkdtempSync(join(tmpdir(), 'fm-reflib-'));

/** A minimal stored (uncompressed) zip, enough to stand in for a Fontshare archive. */
function makeZip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const [name, content] of Object.entries(entries)) {
    const data = Buffer.from(content);
    const nameBuf = Buffer.from(name);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, nameBuf, data);
    centrals.push(central, nameBuf);
    offset += 30 + nameBuf.length + data.length;
  }
  const cd = Buffer.concat(centrals);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(Object.keys(entries).length, 8);
  eocd.writeUInt16LE(Object.keys(entries).length, 10);
  eocd.writeUInt32LE(cd.length, 12);
  eocd.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, cd, eocd]);
}

describe('manifest', () => {
  test('serializes sorted by path with LF and round-trips', () => {
    const rows = [
      { sha256: 'b'.repeat(64), size: 2, license: 'OFL-1.1', source: 'fontshare', path: 'fontshare/z/Z.otf' },
      { sha256: 'a'.repeat(64), size: 1, license: 'Apache-2.0', source: 'google-repo', path: 'google-fonts/apache/a/A.ttf' },
    ];
    const text = serializeManifest(rows);
    assert.equal(text, serializeManifest([...rows].reverse()), 'order of input must not matter');
    assert.ok(!text.includes('\r'));
    const back = parseManifest(text);
    assert.deepEqual(back.map((r) => r.path), ['fontshare/z/Z.otf', 'google-fonts/apache/a/A.ttf']);
    assert.deepEqual(back[1], rows[1]);
  });

  test('rejects a file that is not a manifest', () => {
    assert.throws(() => parseManifest('hello\n'), /not a reference-library manifest/);
  });
});

describe('planning', () => {
  test('gitBlobSha matches git hash-object', () => {
    // printf 'hello\n' | git hash-object --stdin
    assert.equal(gitBlobSha(Buffer.from('hello\n')), 'ce013625030ba8dba906f756967f9e9ca394464a');
  });

  test('google repo plan keeps fonts and licence files under ofl/apache/ufl only', () => {
    const tree = {
      tree: [
        { type: 'blob', path: 'ofl/abel/Abel-Regular.ttf', sha: '1', size: 10 },
        { type: 'blob', path: 'ofl/abel/OFL.txt', sha: '2', size: 1 },
        { type: 'blob', path: 'ofl/abel/METADATA.pb', sha: '3', size: 1 },
        { type: 'blob', path: 'apache/roboto/Roboto[wdth,wght].ttf', sha: '4', size: 10 },
        { type: 'blob', path: 'ufl/ubuntu/Ubuntu-R.ttf', sha: '5', size: 10 },
        { type: 'blob', path: 'cc-by-sa/x/X.ttf', sha: '6', size: 10 },
        { type: 'blob', path: 'tools/test.ttf', sha: '7', size: 10 },
        { type: 'tree', path: 'ofl/abel', sha: '8' },
      ],
    };
    const plan = planGoogleRepo(tree);
    assert.deepEqual(
      plan.map((f) => [f.path, f.license]),
      [
        ['ofl/abel/Abel-Regular.ttf', 'OFL-1.1'],
        ['ofl/abel/OFL.txt', 'OFL-1.1'],
        ['apache/roboto/Roboto[wdth,wght].ttf', 'Apache-2.0'],
        ['ufl/ubuntu/Ubuntu-R.ttf', 'UFL-1.0'],
      ],
    );
  });

  test('google static plan takes variable families whose repo dir has an allowed licence', () => {
    assert.equal(familyDir('Roboto Condensed'), 'robotocondensed');
    const repo = [
      { path: 'ofl/robotocondensed/RobotoCondensed[wght].ttf' },
      { path: 'apache/opensans/OpenSans[wght].ttf' },
      { path: 'ofl/abel/Abel-Regular.ttf' },
    ];
    const meta = {
      familyMetadataList: [
        { family: 'Roboto Condensed', axes: [{ tag: 'wght' }] },
        { family: 'Open Sans', axes: [{ tag: 'wght' }] },
        { family: 'Abel', axes: [] },
        { family: 'Not In Repo', axes: [{ tag: 'wght' }] },
      ],
    };
    assert.deepEqual(planGoogleStatic(meta, repo), [
      { family: 'Open Sans', dir: 'opensans', license: 'Apache-2.0' },
      { family: 'Roboto Condensed', dir: 'robotocondensed', license: 'OFL-1.1' },
    ]);
    const listing = parseGoogleJson(
      ")]}'\n" +
        JSON.stringify({
          manifest: {
            fileRefs: [
              { filename: 'static/OpenSans-Bold.ttf', url: 'u1' },
              { filename: 'OpenSans[wght].ttf', url: 'u2' },
              { filename: 'static/../../evil.ttf', url: 'u3' },
              { filename: 'static/README.txt', url: 'u4' },
            ],
          },
        }),
    );
    assert.deepEqual(staticFileRefs(listing).map((r) => r.url), ['u1']);
  });

  test('fontshare plan keeps OFL families only', () => {
    const pages = [
      { fonts: [{ slug: 'satoshi', license_type: 'itf_ffl' }, { slug: 'archivo', license_type: 'sil_ofl' }] },
      { fonts: [{ slug: 'anton', license_type: 'sil_ofl' }, { slug: '../x', license_type: 'sil_ofl' }] },
    ];
    assert.deepEqual(planFontshare(pages), ['anton', 'archivo']);
  });

  test('cli: dest falls back to FM_REFERENCE_LIBRARY, bad values are refused', () => {
    const o = parseCli(['--sources', 'google-repo', '--jobs', '2'], { FM_REFERENCE_LIBRARY: '/lib' });
    assert.ok(o.dest.endsWith('lib'));
    assert.deepEqual(o.sources, ['google-repo']);
    assert.equal(o.jobs, 2);
    assert.throws(() => parseCli(['--sources', 'adobe'], {}), /unknown source/);
    assert.throws(() => parseCli(['--jobs', '0'], {}), /positive integer/);
    assert.throws(() => parseCli(['--bogus'], {}), /unknown argument/);
  });
});

describe('zip extraction', () => {
  test('extracts entries and refuses paths that leave the folder', () => {
    const dir = tmp();
    try {
      const out = extractZip(makeZip({ 'A/Fonts/A.otf': 'font', 'A/License/OFL.txt': 'ofl' }), dir);
      assert.deepEqual(out, ['A/Fonts/A.otf', 'A/License/OFL.txt']);
      assert.equal(readFileSync(join(dir, 'A', 'Fonts', 'A.otf'), 'utf8'), 'font');
      assert.throws(() => extractZip(makeZip({ '../evil.ttf': 'x' }), dir), /unsafe path/);
      assert.throws(() => extractZip(makeZip({ 'a\\..\\..\\evil.ttf': 'x' }), dir), /unsafe path/);
      assert.throws(() => extractZip(makeZip({ '/etc/evil.ttf': 'x' }), dir), /unsafe path/);
      assert.throws(() => extractZip(Buffer.from('not a zip at all, really not'), dir), /not a zip/);
      assert.ok(!existsSync(join(dir, '..', 'evil.ttf')));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

// ---------------------------------------------------------------- fake upstream

const REF = 'pinned';
const FILES = {
  'ofl/abel/Abel-Regular.ttf': Buffer.alloc(5000, 1),
  'ofl/abel/OFL.txt': Buffer.from('SIL Open Font License'),
  'ofl/opensans/OpenSans[wght].ttf': Buffer.alloc(7000, 2),
  'apache/roboto/Roboto-Regular.ttf': Buffer.alloc(3000, 3),
  'cc-by-sa/nope/Nope.ttf': Buffer.alloc(10, 4),
};
const STATIC = { 'static/OpenSans-Bold.ttf': Buffer.alloc(4000, 5), 'static/OpenSans-Light.ttf': Buffer.alloc(4100, 6) };
const ZIPS = {
  archivo: makeZip({ 'Archivo/Fonts/Archivo-Regular.otf': 'otf-bytes', 'Archivo/Fonts/Archivo.woff2': 'w2', 'Archivo/License/OFL.txt': 'ofl' }),
};

function upstream() {
  const state = { requests: [], failOnce: new Set(), cutOnce: new Set(), ignoreRange: false };
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    state.requests.push({ path: url.pathname + url.search, range: req.headers.range });
    const send = (body, type = 'application/octet-stream') => {
      const key = url.pathname;
      if (state.failOnce.has(key)) {
        state.failOnce.delete(key);
        res.writeHead(500).end();
        return;
      }
      let start = 0;
      const m = !state.ignoreRange && /^bytes=(\d+)-$/.exec(req.headers.range ?? '');
      if (m) start = Number(m[1]);
      if (start >= body.length && m) {
        res.writeHead(416, { 'Content-Range': `bytes */${body.length}` }).end();
        return;
      }
      const chunk = body.subarray(start);
      const headers = { 'Content-Type': type, 'Content-Length': chunk.length };
      if (m) headers['Content-Range'] = `bytes ${start}-${body.length - 1}/${body.length}`;
      res.writeHead(m ? 206 : 200, headers);
      if (state.cutOnce.has(key)) {
        // Send half the body, then drop the connection: an interrupted download.
        state.cutOnce.delete(key);
        res.write(chunk.subarray(0, Math.floor(chunk.length / 2)), () => setTimeout(() => res.destroy(), 50));
        return;
      }
      res.end(chunk);
    };
    const p = url.pathname;
    if (p === `/gh/repos/google/fonts/git/trees/${REF}`) {
      const tree = Object.entries(FILES).map(([path, buf]) => ({ type: 'blob', path, sha: gitBlobSha(buf), size: buf.length }));
      return send(Buffer.from(JSON.stringify({ truncated: false, tree })), 'application/json');
    }
    if (p.startsWith(`/raw/google/fonts/${REF}/`)) {
      const buf = FILES[decodeURIComponent(p.slice(`/raw/google/fonts/${REF}/`.length))];
      return buf ? send(buf) : res.writeHead(404).end();
    }
    if (p === '/gf/metadata/fonts') {
      const meta = { familyMetadataList: [{ family: 'Open Sans', axes: [{ tag: 'wght' }] }, { family: 'Abel', axes: [] }] };
      return send(Buffer.from(")]}'\n" + JSON.stringify(meta)), 'application/json');
    }
    if (p === '/gf/download/list') {
      const base = `http://127.0.0.1:${server.address().port}/static/`;
      const fileRefs = Object.keys(STATIC).map((f) => ({ filename: f, url: base + f.slice(7) }));
      return send(Buffer.from(")]}'\n" + JSON.stringify({ manifest: { fileRefs } })), 'application/json');
    }
    if (p.startsWith('/static/')) {
      const buf = STATIC[`static/${p.slice(8)}`];
      return buf ? send(buf) : res.writeHead(404).end();
    }
    if (p === '/fs/v2/fonts') {
      const offset = Number(url.searchParams.get('offset'));
      const fonts = offset === 0 ? [{ slug: 'archivo', license_type: 'sil_ofl' }, { slug: 'satoshi', license_type: 'itf_ffl' }] : [];
      return send(Buffer.from(JSON.stringify({ fonts, has_more: offset === 0 })), 'application/json');
    }
    if (p.startsWith('/fs/v2/fonts/download/')) {
      const zip = ZIPS[p.slice('/fs/v2/fonts/download/'.length)];
      return zip ? send(zip, 'application/zip') : res.writeHead(404).end();
    }
    res.writeHead(404).end();
  });
  return { server, state };
}

describe('build against a fake upstream', () => {
  let up;
  let endpoints;
  before(async () => {
    up = upstream();
    await new Promise((r) => up.server.listen(0, '127.0.0.1', r));
    const base = `http://127.0.0.1:${up.server.address().port}`;
    endpoints = { githubApi: `${base}/gh`, githubRaw: `${base}/raw`, googleFonts: `${base}/gf`, fontshareApi: `${base}/fs` };
  });
  after(() => up.server.close());

  const opts = (dest, extra = {}) => ({ dest, endpoints, googleRef: REF, jobs: 2, retries: 0, retryDelayMs: 1, ...extra });

  test('downloads all three sources, licensed files only, with a manifest of hashes', async () => {
    const dest = tmp();
    try {
      const { rows, failures } = await build(opts(dest));
      assert.deepEqual(failures, []);
      const manifest = parseManifest(readFileSync(join(dest, 'manifest.tsv'), 'utf8'));
      assert.deepEqual(manifest, [...rows].sort((a, b) => (a.path < b.path ? -1 : 1)));
      assert.deepEqual(
        manifest.map((r) => [r.path, r.source, r.license]),
        [
          ['fontshare/archivo/Archivo/Fonts/Archivo-Regular.otf', 'fontshare', 'OFL-1.1'],
          ['fontshare/archivo/Archivo/Fonts/Archivo.woff2', 'fontshare', 'OFL-1.1'],
          ['google-fonts-static/opensans/OpenSans-Bold.ttf', 'google-static', 'OFL-1.1'],
          ['google-fonts-static/opensans/OpenSans-Light.ttf', 'google-static', 'OFL-1.1'],
          ['google-fonts/apache/roboto/Roboto-Regular.ttf', 'google-repo', 'Apache-2.0'],
          ['google-fonts/ofl/abel/Abel-Regular.ttf', 'google-repo', 'OFL-1.1'],
          ['google-fonts/ofl/opensans/OpenSans[wght].ttf', 'google-repo', 'OFL-1.1'],
        ],
      );
      const abel = manifest.find((r) => r.path.endsWith('Abel-Regular.ttf'));
      assert.equal(abel.size, 5000);
      const { createHash } = await import('node:crypto');
      assert.equal(abel.sha256, createHash('sha256').update(FILES['ofl/abel/Abel-Regular.ttf']).digest('hex'));
      assert.ok(existsSync(join(dest, 'google-fonts', 'ofl', 'abel', 'OFL.txt')), 'licence file sits beside the fonts');
      assert.ok(!existsSync(join(dest, 'google-fonts', 'cc-by-sa')), 'other licences are not fetched');
      assert.ok(!existsSync(join(dest, 'fontshare', 'satoshi')), 'non-OFL Fontshare families are not fetched');
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test('is idempotent: a second run makes no request and writes the same manifest', async () => {
    const dest = tmp();
    try {
      await build(opts(dest));
      const first = readFileSync(join(dest, 'manifest.tsv'));
      up.state.requests.length = 0;
      const { failures, stats } = await build(opts(dest));
      assert.deepEqual(failures, []);
      assert.deepEqual(up.state.requests, [], 'second run must not touch the network');
      assert.equal(stats.downloads, 0);
      assert.deepEqual(readFileSync(join(dest, 'manifest.tsv')), first);
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test('is resumable: a failed run is finished by the next one, fetching only what is missing', async () => {
    const dest = tmp();
    try {
      up.state.failOnce.add(`/raw/google/fonts/${REF}/apache/roboto/Roboto-Regular.ttf`);
      up.state.failOnce.add('/fs/v2/fonts/download/archivo');
      const first = await build(opts(dest));
      assert.equal(first.failures.length, 2);
      const partial = parseManifest(readFileSync(join(dest, 'manifest.tsv'), 'utf8'));
      assert.equal(partial.length, 4, 'the manifest lists what is complete, nothing more');
      assert.ok(!partial.some((r) => r.path.includes('roboto') || r.source === 'fontshare'));

      up.state.requests.length = 0;
      const second = await build(opts(dest));
      assert.deepEqual(second.failures, []);
      assert.deepEqual(
        up.state.requests.map((r) => r.path).sort(),
        ['/fs/v2/fonts/download/archivo', `/raw/google/fonts/${REF}/apache/roboto/Roboto-Regular.ttf`],
      );
      assert.equal(parseManifest(readFileSync(join(dest, 'manifest.tsv'), 'utf8')).length, 7);
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test('re-hashes a file whose content changed and drops a file that vanished', async () => {
    const dest = tmp();
    try {
      await build(opts(dest, { sources: ['google-repo'] }));
      const file = join(dest, 'google-fonts', 'ofl', 'abel', 'Abel-Regular.ttf');
      writeFileSync(file, 'tampered and longer than before');
      const { rows } = await build(opts(dest, { sources: ['google-repo'] }));
      const abel = rows.find((r) => r.path.endsWith('Abel-Regular.ttf'));
      const { createHash } = await import('node:crypto');
      assert.equal(abel.sha256, createHash('sha256').update('tampered and longer than before').digest('hex'));
      // --verify puts the pinned content back.
      const fixed = await build(opts(dest, { sources: ['google-repo'], verify: true }));
      assert.equal(fixed.rows.find((r) => r.path.endsWith('Abel-Regular.ttf')).size, 5000);
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test('download resumes an interrupted body with a Range request', async () => {
    const dest = tmp();
    try {
      const url = `${endpoints.githubRaw}/google/fonts/${REF}/ofl/opensans/OpenSans%5Bwght%5D.ttf`;
      const file = join(dest, 'OpenSans.ttf');
      up.state.cutOnce.add(`/raw/google/fonts/${REF}/ofl/opensans/OpenSans%5Bwght%5D.ttf`);
      await assert.rejects(downloadFile(url, file, { retries: 0 }));
      assert.ok(!existsSync(file), 'no final file after an interrupted download');
      const half = readFileSync(`${file}.part`).length;
      assert.ok(half > 0 && half < 7000, `part file holds the first bytes (${half})`);

      up.state.requests.length = 0;
      await downloadFile(url, file, { retries: 0 });
      assert.equal(up.state.requests[0].range, `bytes=${half}-`);
      assert.deepEqual(readFileSync(file), FILES['ofl/opensans/OpenSans[wght].ttf']);
      assert.ok(!existsSync(`${file}.part`));
    } finally {
      rmSync(dest, { recursive: true, force: true });
    }
  });

  test('download starts over when the server ignores the Range header', async () => {
    const dest = tmp();
    try {
      const url = `${endpoints.githubRaw}/google/fonts/${REF}/ofl/abel/Abel-Regular.ttf`;
      const file = join(dest, 'Abel.ttf');
      writeFileSync(`${file}.part`, 'stale bytes');
      up.state.ignoreRange = true;
      await downloadFile(url, file, { retries: 0 });
      assert.deepEqual(readFileSync(file), FILES['ofl/abel/Abel-Regular.ttf']);
    } finally {
      up.state.ignoreRange = false;
      rmSync(dest, { recursive: true, force: true });
    }
  });
});
