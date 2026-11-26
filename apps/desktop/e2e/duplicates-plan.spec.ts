import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { sectionPanel, showSection } from './family-page';

// E56.15: the plan before anything moves, the apply with its progress and Cancel, the
// sessions in History with their undo, the hidden copies and remembered answers, and the
// ways into the module. The browser mock with `?mockDuplicates=1` has JetBrains Mono twice
// (identical), Inter in three versions and Legacy Grotesk twice; `?mockConflict=1` rates
// the kept Inter 5 stars and its old copy 3; `?mockCleanupStep=<ms>` applies file by file.

const STEPPED = '?mockDuplicates=1&mockConflict=1&mockCleanupStep=400';
const open = async (page: Page, section = '', query = STEPPED) => {
  await page.goto(`/${query}#/duplicates${section ? `?section=${section}` : ''}`);
  await expect(nav(page)).toBeVisible();
};
const nav = (page: Page) => page.getByRole('navigation', { name: 'Duplicates' });
const setPanel = (page: Page) => page.getByTestId('duplicate-set');
const review = (page: Page) => page.getByTestId('plan-review');
const history = (page: Page) => page.getByTestId('duplicates-history');

/** Puts the Inter set into the plan as the mock recommends it, and opens the plan. */
async function planInter(page: Page): Promise<void> {
  await nav(page).getByRole('button', { name: /^Sets/ }).click();
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  await setPanel(page)
    .getByRole('button', { name: /^Add to plan and go on/ })
    .click();
  await nav(page).getByRole('button', { name: /^Plan/ }).click();
  await expect(review(page)).toBeVisible();
}

test('the plan shows what goes, what stays and the conflict before anything moves', async ({
  page,
}) => {
  await open(page);
  await planInter(page);
  const trash = review(page).locator('[data-group="trash"]');
  await expect(trash).toContainText(/^2 files go to the (Trash|Recycle Bin)/);
  await trash.getByRole('button').click();
  await expect(trash.locator('code')).toHaveText([
    '/Users/demo/Fonts/Old/Inter-Regular.ttf',
    '/Users/demo/Fonts/Older/Inter-Regular.ttf',
  ]);
  const stays = review(page).locator('[data-group="stays-keeper"]');
  await expect(stays).toContainText('1 file stays');
  await stays.getByRole('button').click();
  await expect(stays).toContainText('/Users/demo/Fonts/Inter/Inter[wght].ttf');

  // The old copy has 3 stars, the kept one 5: both values and a choice, 5 by default.
  const conflict = page.getByTestId('plan-conflicts').getByRole('group');
  await expect(conflict).toHaveCount(1);
  await expect(conflict.getByRole('radio', { name: 'Keep 5 stars' })).toBeChecked();
  await conflict.getByRole('radio', { name: /^Take 3 stars from/ }).check();
  await expect(conflict.getByRole('radio', { name: 'Keep 5 stars' })).not.toBeChecked();

  // The button names the count and this system's trash; nothing moved yet.
  await expect(page.getByTestId('plan-apply')).toHaveText(
    /^Move 2 files to the (Trash|Recycle Bin)$/,
  );
  await nav(page).getByRole('button', { name: 'Overview' }).click();
  await expect(
    page.getByTestId('duplicates-overview').getByRole('heading', { level: 2 }),
  ).toHaveText('3 fonts have more than one copy');
});

test('apply shows the progress and ends with a summary that undoes', async ({ page }) => {
  await open(page);
  await planInter(page);
  await page.getByTestId('plan-apply').click();
  const progress = page.getByTestId('plan-progress');
  await expect(progress).toBeVisible();
  await expect(progress.getByRole('status')).toHaveText(/of 2 files$/);

  const result = page.getByTestId('plan-result');
  await expect(result).toContainText(/Done\.\s*2 files moved to the (Trash|Recycle Bin)/);
  await expect(progress).toBeHidden();
  await expect(nav(page).getByTestId('dup-count-sets')).toHaveText('1');

  await result.getByRole('button', { name: /^Undo/ }).click();
  await expect(result.getByRole('button', { name: /^Undo/ })).toHaveCount(0);
  await expect(nav(page).getByTestId('dup-count-sets')).toHaveText('2');
});

test('cancel stops between files and the files after it stay', async ({ page }) => {
  await open(page, '', '?mockDuplicates=1&mockCleanupStep=1500');
  await planInter(page);
  await page.getByTestId('plan-apply').click();
  const progress = page.getByTestId('plan-progress');
  await expect(progress.getByRole('status')).toHaveText('1 of 2 files');
  await progress.getByRole('button', { name: 'Cancel' }).click();

  const result = page.getByTestId('plan-result');
  await expect(result).toContainText('Cancelled.');
  await expect(result).toContainText('The files after the last one done stay where they are.');
  await expect(result).toContainText(/1 file moved to the (Trash|Recycle Bin)/);
});

test('History lists the session and undoes it whole or file by file', async ({ page }) => {
  await open(page, '', '?mockDuplicates=1&mockCleanupStep=50');
  await planInter(page);
  await page.getByTestId('plan-apply').click();
  await expect(page.getByTestId('plan-result')).toContainText('Done.');
  await page.getByTestId('plan-result').getByRole('link', { name: 'See it in History' }).click();

  const session = history(page).locator('[data-session]');
  await expect(session).toHaveCount(1);
  await expect(session).toContainText(/1 font · 2 in the (Trash|Recycle Bin)/);
  await session.getByRole('button', { expanded: false }).click();
  const files = session.locator('[data-status]');
  await expect(files).toHaveCount(2);

  // One file back, then the rest of the session.
  await files
    .first()
    .getByRole('button', { name: /^Put back/ })
    .click();
  await expect(files.first()).toHaveAttribute('data-status', 'restored');
  await expect(files.first()).toContainText('Put back');
  await session.getByRole('button', { name: /^Undo all/ }).click();
  await expect(files.nth(1)).toHaveAttribute('data-status', 'restored');
  await expect(session).toContainText('Undone');
  await expect(session.getByRole('button', { name: /^Undo all/ })).toHaveCount(0);
});

test('History says when a file cannot come back', async ({ page }) => {
  // `?mockUndoFail=Older`: the trash no longer holds the Older copy, as when it was emptied.
  await open(page, '', '?mockDuplicates=1&mockUndoFail=Older');
  await planInter(page);
  await page.getByTestId('plan-apply').click();
  await expect(page.getByTestId('plan-result')).toContainText('Done.');
  await nav(page)
    .getByRole('button', { name: /^History/ })
    .click();
  const session = history(page).locator('[data-session]');
  await session.getByRole('button', { name: /^Undo all/ }).click();
  await session.getByRole('button', { expanded: false }).click();
  const lost = session.locator('[data-status="failed"]');
  await expect(lost).toHaveCount(1);
  await expect(lost).toContainText('/Users/demo/Fonts/Older/Inter-Regular.ttf');
  await expect(lost).toContainText(
    'Cannot come back — The trash was emptied, so this file cannot come back.',
  );
  await expect(session.locator('[data-status="restored"]')).toHaveCount(1);
});

test('a hidden copy and a remembered answer can each be taken back', async ({ page }) => {
  await open(page, 'sets', '?mockDuplicates=1');
  // Hide the old Inter copy, trash none: Keep the others.
  const copies = setPanel(page).getByTestId('duplicate-copy');
  await copies.nth(1).getByRole('button', { name: 'Hide' }).click();
  await copies.nth(2).getByRole('button', { name: 'Keep' }).click();
  await setPanel(page)
    .getByRole('button', { name: /^Add to plan and go on/ })
    .click();
  // Legacy Grotesk: "Not duplicates".
  await page.keyboard.press('n');
  await nav(page).getByRole('button', { name: /^Plan/ }).click();
  await expect(page.getByTestId('plan-apply')).toBeEnabled();
  await page.getByTestId('plan-apply').click();
  await expect(page.getByTestId('plan-result')).toContainText('Done.');

  await nav(page)
    .getByRole('button', { name: /^Hidden copies/ })
    .click();
  const hidden = page.getByTestId('duplicates-hidden');
  await expect(hidden.getByRole('listitem')).toHaveCount(1);
  await hidden.getByRole('button', { name: /^Show .* again$/ }).click();
  await expect(hidden.getByRole('listitem')).toHaveCount(0);
  await expect(hidden).toContainText('No hidden copies.');

  await nav(page)
    .getByRole('button', { name: /^Remembered decisions/ })
    .click();
  const decisions = page.getByTestId('duplicates-decisions');
  await expect(decisions.getByRole('listitem')).toHaveCount(1);
  await expect(decisions).toContainText('Not duplicates');
  await decisions.getByRole('button', { name: /^Take back the answer about/ }).click();
  await expect(decisions).toContainText('No remembered decisions.');
});

test('the ways in: the family mark and Info, the palette', async ({ page }) => {
  // The native File menu opens the module (E57.10); the sidebar has no row for it,
  // even while fonts have copies.
  await page.goto('/?mockDuplicates=1#/');
  await expect(page.getByRole('navigation', { name: 'Library' })).toBeVisible();
  // The marks on the cards say the copies are known before the sidebar is checked.
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  await expect(page.getByTestId('card-copies').first()).toHaveText('3 copies');
  await expect(page.getByTestId('sidebar-duplicates')).toHaveCount(0);
  await expect(
    page.getByRole('navigation', { name: 'Library' }).getByText('Duplicates', { exact: true }),
  ).toHaveCount(0);

  // The family page: the "N copies" mark and the line under Info › Files.
  await page.goto('/?mockDuplicates=1#/family/1?face=1');
  const mark = page.getByTestId('family-copies');
  await expect(mark).toHaveText('3 copies');
  await showSection(page, 'Info');
  const info = sectionPanel(page, 'Info').getByTestId('info-copies');
  await expect(info).toContainText('3 copies');
  await info.getByRole('link', { name: 'Decide which to keep' }).click();
  await expect(setPanel(page).getByRole('heading', { level: 2 })).toHaveText('Inter-Regular');
  await expect(page.getByTestId('queue-family')).toBeVisible();

  // The palette: "Show the copies of Inter" on the family page, "Find duplicates" anywhere.
  await page.goto('/?mockDuplicates=1#/family/1?face=1');
  await expect(mark).toBeVisible();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('Show the copies of Inter');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('queue-family')).toBeVisible();
  await page.getByTestId('queue-family').getByRole('button', { name: 'Show every set' }).click();
  await expect(page.getByTestId('queue-family')).toHaveCount(0);

  await page.goto('/?mockDuplicates=1#/');
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('Find duplicates');
  await page.keyboard.press('Enter');
  await expect(nav(page)).toBeVisible();
});

test('the wall card marks a family with copies and opens its sets', async ({ page }) => {
  await page.goto('/?mockDuplicates=1#/');
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const mark = page.getByTestId('card-copies').first();
  await expect(mark).toHaveText('3 copies');
  await mark.click();
  await expect(page.getByTestId('queue-family')).toBeVisible();
});

test('without copies the palette still opens the module, on its empty overview', async ({
  page,
}) => {
  await page.goto('/#/');
  await expect(page.getByRole('navigation', { name: 'Library' })).toBeVisible();
  await expect(page.getByTestId('sidebar-duplicates')).toHaveCount(0);
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.type('Find duplicates');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/duplicates/);
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`the plan, History and the kept lists pass a WCAG 2.2 AA scan (${colorScheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme, reducedMotion: 'reduce' });
    const scan = async () => {
      // A section fades in when it opens: scan the colors it ends with.
      await page.waitForFunction(() =>
        document.getAnimations().every((a) => a.playState !== 'running'),
      );
      const result = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
    };
    await open(page, '', '?mockDuplicates=1&mockConflict=1&mockCleanupStep=50');
    await planInter(page);
    for (const group of await review(page).locator('[data-group] > button').all())
      await group.click();
    await expect(page.getByTestId('plan-conflicts')).toBeVisible();
    await scan();
    await page.getByTestId('plan-apply').click();
    await expect(page.getByTestId('plan-result')).toContainText('Done.');
    await scan();
    await nav(page)
      .getByRole('button', { name: /^History/ })
      .click();
    await history(page).locator('[data-session]').getByRole('button', { expanded: false }).click();
    await expect(history(page).locator('[data-status]')).toHaveCount(2);
    await scan();
    await nav(page)
      .getByRole('button', { name: /^Hidden copies/ })
      .click();
    await expect(page.getByTestId('duplicates-hidden')).toBeVisible();
    await scan();
    await nav(page)
      .getByRole('button', { name: /^Remembered decisions/ })
      .click();
    await expect(page.getByTestId('duplicates-decisions')).toBeVisible();
    await scan();
  });
}
