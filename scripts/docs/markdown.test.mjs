import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderInline, renderMarkdown, siteHref, slugify } from './markdown.mjs';

test('the first # heading is the title and every heading gets an anchor', () => {
  const { html, title } = renderMarkdown('# Getting started\n\n## Add a folder\n\ntext\n');
  assert.equal(title, 'Getting started');
  assert.match(html, /<h1 id="getting-started">Getting started<\/h1>/);
  assert.match(html, /<h2 id="add-a-folder">Add a folder<\/h2>/);
});

test('a page without a # heading has no title', () => {
  assert.equal(renderMarkdown('## Only a section\n').title, null);
  assert.equal(renderMarkdown('').title, null);
});

test('paragraph lines join and blank lines split them', () => {
  const { html } = renderMarkdown('one\ntwo\n\nthree\r\n');
  assert.equal(html, '<p>one two</p>\n<p>three</p>');
});

test('lists, numbered lists and continued items', () => {
  const { html } = renderMarkdown('- a\n- b\n  more\n\n1. x\n2. y\n');
  assert.equal(html, '<ul>\n<li>a</li>\n<li>b more</li>\n</ul>\n<ol>\n<li>x</li>\n<li>y</li>\n</ol>');
});

test('pipe tables have a header row and fill missing cells', () => {
  const { html } = renderMarkdown('| Key | Does |\n|-----|------|\n| `A` | act |\n| `B` |\n');
  assert.match(html, /<th scope="col">Key<\/th><th scope="col">Does<\/th>/);
  assert.match(html, /<tr><td><code>A<\/code><\/td><td>act<\/td><\/tr>/);
  assert.match(html, /<tr><td><code>B<\/code><\/td><td><\/td><\/tr>/);
});

test('notes and fenced code', () => {
  const { html } = renderMarkdown('> a\n> b\n\n```\n<x> & y\n```\n');
  assert.equal(html, '<aside class="note"><p>a b</p></aside>\n<pre><code>&lt;x&gt; &amp; y</code></pre>');
});

test('raw HTML in the source is shown, not run', () => {
  const { html } = renderMarkdown('<script>alert(1)</script> "q"\n');
  assert.equal(html, '<p>&lt;script&gt;alert(1)&lt;/script&gt; &quot;q&quot;</p>');
});

test('code spans stay literal', () => {
  assert.equal(renderInline('`**not bold** <b>`'), '<code>**not bold** &lt;b&gt;</code>');
  assert.equal(renderInline('`Ctrl+,` and `/`'), '<code>Ctrl+,</code> and <code>/</code>');
});

test('bold and emphasis', () => {
  assert.equal(renderInline('**Add folder** and _this_'), '<strong>Add folder</strong> and <em>this</em>');
  assert.equal(renderInline('snake_case_name'), 'snake_case_name');
});

test('links to .md pages point at the built .html pages', () => {
  assert.equal(siteHref('sources.md'), 'sources.html');
  assert.equal(siteHref('activation.md#two-kinds'), 'activation.html#two-kinds');
  assert.equal(renderInline('[Sources](sources.md)'), '<a href="sources.html">Sources</a>');
  assert.equal(renderInline('[here](#top)'), '<a href="#top">here</a>');
});

test('links that leave the bundled pages are plain text', () => {
  assert.equal(renderInline('[x](javascript:alert(1))'), 'x)');
  assert.equal(renderInline('[web](https://example.com)'), 'web');
  assert.equal(renderInline('[up](../secret.md)'), 'up');
});

test('slugify makes anchor ids', () => {
  assert.equal(slugify('Rescan, pause and remove'), 'rescan-pause-and-remove');
});
