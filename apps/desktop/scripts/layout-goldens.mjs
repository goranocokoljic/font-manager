// Renders the layout presets in Chromium (the WebView2 engine) on an A4 page, with the same
// CSS as LayoutBlock.vue and FontLayout.vue, into fixtures/layouts/<id>.png. fm-core's page
// renderer is compared against these images (crates/fm-core/src/preview/page/tests.rs).
//
//   cargo run -p fm-core --example layout_page -- presets > fixtures/layouts/presets.json
//   node apps/desktop/scripts/layout-goldens.mjs
//
// Hinting and LCD antialiasing are off because the core rasterizer has neither.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const root = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const FONT = 'fixtures/fonts/google-static/dmsans/DMSans_36pt-Regular.ttf';
const PAGE = { width: 794, height: 1123 };
const MARGIN_PX = 48;

const escapeHtml = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function blockHtml(b) {
  const blockStyle = [
    `font-size:${b.sizePt}pt`,
    `line-height:${b.lineHeight}`,
    `letter-spacing:${b.tracking / 1000}em`,
    `text-align:${b.align}`,
    b.columns > 1 ? `column-count:${b.columns}` : '',
  ].join(';');
  const paragraphStyle = [
    `margin-bottom:${b.paragraphSpacingPt}pt`,
    b.firstLineIndentEm ? `text-indent:${b.firstLineIndentEm}em` : '',
  ].join(';');
  const paragraphs = b.text
    .split('\n')
    .map(
      (line) => `<p class="paragraph" dir="auto" style="${paragraphStyle}">${escapeHtml(line)}</p>`,
    )
    .join('');
  const clip = b.overflow === 'clip' ? ' clip' : '';
  return `<div class="row"><div class="block${clip}" style="${blockStyle}">${paragraphs}</div></div>`;
}

function pageHtml({ widthPx, layout }, fontUrl) {
  return `<!doctype html><meta charset="utf-8"><style>
@font-face { font-family: Golden; src: url(${fontUrl}); }
html, body { margin: 0; background: #ffffff; color: #1a1a1a; }
.page { box-sizing: border-box; width: ${PAGE.width}px; height: ${PAGE.height}px;
  padding: ${MARGIN_PX}px; overflow: hidden; font-family: Golden; }
.row { max-width: ${widthPx}px; margin-inline: auto; }
.block { column-gap: 2em; column-fill: balance; overflow-wrap: break-word; }
.paragraph { margin-top: 0; }
.block.clip { overflow: hidden; }
.block.clip .paragraph { overflow: hidden; white-space: nowrap; }
</style><div class="page">${layout.blocks.map(blockHtml).join('')}</div>`;
}

const presets = JSON.parse(readFileSync(join(root, 'fixtures/layouts/presets.json'), 'utf8'));
const fontUrl = `data:font/ttf;base64,${readFileSync(join(root, FONT)).toString('base64')}`;
const browser = await chromium.launch({
  args: ['--font-render-hinting=none', '--disable-lcd-text'],
});
const page = await browser.newPage({ viewport: PAGE, deviceScaleFactor: 1 });
for (const preset of presets) {
  await page.setContent(pageHtml(preset, fontUrl));
  await page.evaluate('document.fonts.ready.then(() => true)');
  await page.screenshot({ path: join(root, `fixtures/layouts/${preset.id}.png`) });
}
await browser.close();
