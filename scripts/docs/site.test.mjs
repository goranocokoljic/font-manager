import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { after, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { PAGES, SOURCE_DIR, buildSite } from './build-site.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const tmp = mkdtempSync(join(tmpdir(), 'fm-docs-'));
after(() => rmSync(tmp, { recursive: true, force: true }));

const out = join(tmp, 'site');
const built = buildSite({ out });
const page = (slug) => readFileSync(join(out, `${slug}.html`), 'utf8');

test('the site has the required pages, in reading order', () => {
  assert.deepEqual(built, [
    { slug: 'getting-started', title: 'Getting started' },
    { slug: 'sources', title: 'Sources' },
    { slug: 'activation', title: 'Activation' },
    { slug: 'keyboard-shortcuts', title: 'Keyboard shortcuts' },
    { slug: 'shared-libraries', title: 'Shared libraries' },
  ]);
  for (const { slug, title } of built) {
    const html = page(slug);
    assert.match(html, new RegExp(`<title>${title} · Font Manager Help</title>`));
    assert.match(html, new RegExp(`<h1 id="${slug}">${title}</h1>`));
  }
  assert.ok(existsSync(join(out, 'index.html')));
  assert.ok(existsSync(join(out, 'style.css')));
});

test('every page links to every other page, and every link resolves', () => {
  for (const slug of [...PAGES, 'index']) {
    const html = page(slug);
    for (const other of PAGES) assert.ok(html.includes(`href="${other}.html"`), `${slug} -> ${other}`);
    for (const [, href] of html.matchAll(/href="([^"]+)"/g)) {
      const [file, anchor] = href.split('#');
      assert.ok(existsSync(join(out, file)), `${slug}.html links to missing ${href}`);
      if (anchor) {
        const target = file ? readFileSync(join(out, file), 'utf8') : html;
        assert.ok(target.includes(`id="${anchor}"`), `${slug}.html links to missing ${href}`);
      }
    }
  }
});

test('the text of Sources and Getting started links to Shared libraries', () => {
  for (const slug of ['sources', 'getting-started']) {
    const main = /<main>([\s\S]*)<\/main>/.exec(page(slug))?.[1] ?? '';
    assert.ok(main.includes('href="shared-libraries.html'), slug);
  }
});

test('the current page is marked in the navigation', () => {
  assert.match(page('sources'), /<a href="sources.html" aria-current="page">Sources<\/a>/);
});

test('the site uses no script and loads nothing from the network', () => {
  for (const slug of [...PAGES, 'index']) {
    const html = page(slug);
    assert.doesNotMatch(html, /<script/i);
    assert.doesNotMatch(html, /(src|href)="(https?:)?\/\//i);
  }
});

test('the shortcuts page lists every key of the keymap', () => {
  const html = page('keyboard-shortcuts');
  for (const key of ['Ctrl+K', '/', 'Space', 'A', 'Shift+A', 'C', '1', '2', 'Ctrl+,', 'Ctrl+I']) {
    assert.ok(html.includes(`<code>${key}</code>`), key);
  }
});

test('the Help menu and the palette list the same pages as the site', () => {
  const rust = readFileSync(join(ROOT, 'apps/desktop/src-tauri/src/help.rs'), 'utf8');
  const ts = readFileSync(join(ROOT, 'apps/desktop/src/help.ts'), 'utf8');
  assert.deepEqual([...rust.matchAll(/slug: "([a-z0-9-]+)"/g)].map((m) => m[1]), PAGES);
  assert.deepEqual([...ts.matchAll(/slug: '([a-z0-9-]+)'/g)].map((m) => m[1]), PAGES);
  for (const slug of PAGES) assert.ok(existsSync(join(SOURCE_DIR, `${slug}.md`)), slug);
});

test('a missing page fails the build', () => {
  const src = join(tmp, 'none');
  assert.throws(() => buildSite({ src, out: join(tmp, 'a'), pages: ['nope'] }), /ENOENT/);
});

test('a page without a "# Title" line fails the build', () => {
  const src = mkdtempSync(join(tmp, 'untitled-'));
  writeFileSync(join(src, 'x.md'), '## no title\n');
  assert.throws(() => buildSite({ src, out: join(tmp, 'b'), pages: ['x'] }), /has no "# Title" line/);
});
