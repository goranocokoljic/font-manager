#!/usr/bin/env node
// Builds the user docs (docs/user/*.md) into a static site. The default output is
// apps/desktop/public/help/, so Vite serves the pages in dev and bundles them into
// the app: the Help menu opens them offline (CLAUDE.md rule 7).
//
//   node scripts/docs/build-site.mjs [--out <dir>]

import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { escapeHtml, renderMarkdown } from './markdown.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** The source pages. */
export const SOURCE_DIR = join(ROOT, 'docs', 'user');

/** Where the site goes unless `--out` says otherwise. */
export const DEFAULT_OUT = join(ROOT, 'apps', 'desktop', 'public', 'help');

/**
 * The pages in reading order. The Help menu (`HELP_PAGES` in
 * apps/desktop/src-tauri/src/help.rs) and the palette's Help actions
 * (apps/desktop/src/help.ts) list the same slugs; site.test.mjs checks it.
 */
export const PAGES = [
  'getting-started',
  'sources',
  'activation',
  'keyboard-shortcuts',
  'shared-libraries',
];

const STYLE = `:root { color-scheme: light dark; --fg: #1d1d1f; --muted: #55555a; --bg: #ffffff;
  --rule: #d2d2d7; --link: #0a58ca; --code: #f2f2f5; }
@media (prefers-color-scheme: dark) { :root { --fg: #f2f2f5; --muted: #b4b4bb; --bg: #1c1c1e;
  --rule: #48484d; --link: #8ab4ff; --code: #2c2c2e; } }
* { box-sizing: border-box; }
body { margin: 0; font: 15px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif;
  color: var(--fg); background: var(--bg); }
.page { display: grid; grid-template-columns: 13rem minmax(0, 46rem); gap: 2rem; padding: 2rem; }
nav ul { list-style: none; margin: 0; padding: 0; }
nav li { margin: 0 0 0.4rem; }
nav a[aria-current="page"] { font-weight: 600; color: var(--fg); text-decoration: none; }
a { color: var(--link); }
h1 { font-size: 1.8rem; margin: 0 0 1rem; }
h2 { font-size: 1.25rem; margin: 2rem 0 0.5rem; }
h3 { font-size: 1.05rem; margin: 1.5rem 0 0.4rem; }
code { font: 0.9em ui-monospace, "Cascadia Mono", Menlo, monospace; background: var(--code);
  padding: 0.1em 0.3em; border-radius: 4px; }
pre { background: var(--code); padding: 0.8rem; border-radius: 6px; overflow-x: auto; }
pre code { padding: 0; }
table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--rule); vertical-align: top; }
.note { border-left: 3px solid var(--rule); padding: 0.1rem 1rem; color: var(--muted); }
@media (max-width: 720px) { .page { grid-template-columns: 1fr; } }
`;

function layout(title, nav, body) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} · Font Manager Help</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<div class="page">
<nav aria-label="Help pages">
${nav}
</nav>
<main>
${body}
</main>
</div>
</body>
</html>
`;
}

function navList(pages, current) {
  const items = pages.map((p) => {
    const here = p.slug === current ? ' aria-current="page"' : '';
    return `<li><a href="${p.slug}.html"${here}>${escapeHtml(p.title)}</a></li>`;
  });
  return `<ul>\n<li><a href="index.html"${current === 'index' ? ' aria-current="page"' : ''}>Help home</a></li>\n${items.join('\n')}\n</ul>`;
}

/**
 * Renders every page of `pages` from `src` into `out` (emptied first), with an
 * index page and the style sheet. Returns the pages with their titles.
 * Throws when a page is missing or has no `#` title.
 */
export function buildSite({ src = SOURCE_DIR, out = DEFAULT_OUT, pages = PAGES } = {}) {
  const rendered = pages.map((slug) => {
    const { html, title } = renderMarkdown(readFileSync(join(src, `${slug}.md`), 'utf8'));
    if (!title) throw new Error(`docs/user/${slug}.md has no "# Title" line`);
    return { slug, title, html };
  });
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, 'style.css'), STYLE);
  for (const page of rendered) {
    writeFileSync(join(out, `${page.slug}.html`), layout(page.title, navList(rendered, page.slug), page.html));
  }
  const index = `<h1>Font Manager Help</h1>\n<ul>\n${rendered
    .map((p) => `<li><a href="${p.slug}.html">${escapeHtml(p.title)}</a></li>`)
    .join('\n')}\n</ul>`;
  writeFileSync(join(out, 'index.html'), layout('Help', navList(rendered, 'index'), index));
  return rendered.map(({ slug, title }) => ({ slug, title }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const at = process.argv.indexOf('--out');
  const out = at > 0 && process.argv[at + 1] ? resolve(process.argv[at + 1]) : DEFAULT_OUT;
  const built = buildSite({ out });
  console.log(`docs: ${built.length} pages -> ${out}`);
}
