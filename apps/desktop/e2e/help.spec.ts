import { expect, test } from '@playwright/test';

// The palette's Help actions open the built docs (scripts/docs/build-site.mjs, run
// by `pnpm dev`) in a new window; the native Help menu opens the same pages.
const PAGES = [
  ['Getting started', 'getting-started'],
  ['Sources', 'sources'],
  ['Activation', 'activation'],
  ['Keyboard shortcuts', 'keyboard-shortcuts'],
  ['Shared libraries', 'shared-libraries'],
] as const;

test('the palette lists a Help action per docs page and opens the page', async ({
  page,
  context,
}) => {
  await page.goto('/');
  await page.keyboard.press('ControlOrMeta+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await page.keyboard.type('help');
  for (const [title] of PAGES)
    await expect(palette.getByRole('option').filter({ hasText: `Help: ${title}` })).toHaveCount(1);

  await page.keyboard.type(' sources');
  await expect(palette.getByRole('option').first()).toContainText('Help: Sources');
  const [help] = await Promise.all([context.waitForEvent('page'), page.keyboard.press('Enter')]);
  await help.waitForLoadState();
  expect(new URL(help.url()).pathname).toBe('/help/sources.html');
  await expect(help.getByRole('heading', { level: 1 })).toHaveText('Sources');
});

test('every help page is served, titled and links to the others', async ({ page }) => {
  for (const [title, slug] of PAGES) {
    const res = await page.goto(`/help/${slug}.html`);
    expect(res?.status()).toBe(200);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText(title);
    const nav = page.getByRole('navigation', { name: 'Help pages' });
    for (const [other] of PAGES) await expect(nav.getByRole('link', { name: other })).toBeVisible();
    await expect(nav.getByRole('link', { name: title })).toHaveAttribute('aria-current', 'page');
  }
});
