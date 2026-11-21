import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

// The Folders view of the Duplicates module (E56.12) on the browser mock with
// `?mockFolderPairs=1`: 18 sample families in /Users/demo/Fonts/Samples, and three folders
// that hold copies of them — "Backup 2014/Samples" holds Sample 1–6 unchanged (a contained
// pair), "fonts-old" holds Sample 7–11 unchanged and an older Sample 12 (a partial pair),
// and the system folder "Supplemental" holds Sample 13–18 (a side that can only be kept).

const SAMPLES = '/Users/demo/Fonts/Samples';
const BACKUP = '/Users/demo/Backup 2014/Samples';
const OLD = '/Users/demo/Downloads/fonts-old';
const SYSTEM = '/System/Library/Fonts/Supplemental';

const open = async (page: Page, query = '?mockFolderPairs=1') => {
  await page.goto(`/${query}#/duplicates?section=folders`);
  await expect(page.getByTestId('duplicates-folders')).toBeVisible();
};
const cards = (page: Page) => page.getByTestId('folder-pair');
const card = (page: Page, title: string) =>
  cards(page).filter({ has: page.getByRole('heading', { name: title, exact: true }) });
const side = (page: Page, title: string, path: string) =>
  card(page, title).locator('[data-side]').filter({ hasText: path });

test('the folder pairs are listed with both paths, contained or the share, and the size', async ({
  page,
}) => {
  await open(page);
  await expect(
    page.getByRole('heading', { level: 2, name: 'Folders that hold the same fonts' }),
  ).toBeVisible();
  await expect(cards(page)).toHaveCount(3);
  await expect(
    page.getByRole('navigation', { name: 'Duplicates' }).getByTestId('dup-count-folders'),
  ).toHaveText('3');

  // Largest saving first: the API's order (frees), each card with its size.
  const titles = await cards(page).getByRole('heading', { level: 3 }).allTextContents();
  expect(titles).toHaveLength(3);
  const frees = await cards(page).locator('.size').allTextContents();
  expect(frees.every((f) => /^Up to .+ to free$/.test(f.trim()))).toBe(true);

  const contained = card(page, 'Samples is contained in Samples');
  await expect(contained.getByTestId('pair-share')).toHaveText('Contained');
  await expect(contained.getByTestId('pair-path')).toHaveText([BACKUP, SAMPLES]);
  await expect(side(page, 'Samples is contained in Samples', BACKUP)).toContainText(
    '6 of 6 files are in the other folder',
  );
  await expect(side(page, 'Samples is contained in Samples', SAMPLES)).toContainText(
    '6 of 18 files are in the other folder',
  );
  await expect(side(page, 'Samples is contained in Samples', SAMPLES)).toContainText(
    '12 only here',
  );

  // The partial pair shows its share (6 of 6 in fonts-old have a copy, one of them in
  // another version) and the fonts that stay in the queue of sets.
  const partial = card(page, 'fonts-old and Samples share fonts');
  await expect(partial.getByTestId('pair-share')).toHaveText('100% shared');
  await expect(partial).toContainText(
    '1 more font is in both folders in another file. It stays in the queue of sets.',
  );
});

test("'Keep this side' adds a pair to the plan and says what it covers, nothing else", async ({
  page,
}) => {
  await open(page);
  const title = 'fonts-old and Samples share fonts';
  const keep = side(page, title, SAMPLES).getByRole('button', { name: /^Keep this side/ });
  await keep.focus();
  await page.keyboard.press('Enter');
  await expect(keep).toHaveAttribute('aria-pressed', 'true');
  await expect(keep).toBeFocused();
  await expect(card(page, title).getByTestId('pair-covers')).toHaveText(
    new RegExp(
      `^In the plan: the 5 identical copies in ${OLD} \\(.+\\) go to the (Trash|Recycle Bin)\\. Nothing else\\.`,
    ),
  );
  const nav = page.getByRole('navigation', { name: 'Duplicates' });
  await expect(nav.getByTestId('dup-count-plan')).toHaveText('1');

  // Keeping the other side instead replaces the decision.
  await side(page, title, OLD)
    .getByRole('button', { name: /^Keep this side/ })
    .click();
  await expect(card(page, title).getByTestId('pair-covers')).toContainText(
    `the 5 identical copies in ${SAMPLES}`,
  );
  await expect(nav.getByTestId('dup-count-plan')).toHaveText('1');

  // The plan lists it, and Take back takes it out.
  await nav.getByRole('button', { name: /^Plan/ }).click();
  await expect(page.getByTestId('plan-pairs')).toContainText(
    `Keep ${OLD}: 5 identical copies in ${SAMPLES} leave`,
  );
  await nav.getByRole('button', { name: /^Folders/ }).click();
  await card(page, title)
    .getByRole('button', { name: `Take back ${title}` })
    .click();
  await expect(card(page, title).getByTestId('pair-covers')).toHaveCount(0);
  await expect(nav.getByTestId('dup-count-plan')).toHaveText('0');
  await expect(
    side(page, title, OLD).getByRole('button', { name: /^Keep this side/ }),
  ).toBeFocused();
});

test('a system font folder says why and offers only to be kept', async ({ page }) => {
  await open(page);
  const title = 'Supplemental is contained in Samples';
  const system = side(page, title, SYSTEM);
  await expect(system).toHaveAttribute('data-protected', 'systemFolder');
  await expect(system.getByTestId('pair-protected')).toHaveText(
    'A system font folder is never removed from',
  );
  await expect(system.getByRole('button', { name: /^Keep this side/ })).toBeVisible();
  // Keeping the other side would remove from the system folder: not offered.
  const other = side(page, title, SAMPLES);
  await expect(other.getByRole('button', { name: /^Keep this side/ })).toHaveCount(0);
  await expect(other.getByTestId('pair-protected')).toHaveCount(0);

  await system.getByRole('button', { name: /^Keep this side/ }).click();
  await expect(card(page, title).getByTestId('pair-covers')).toContainText(
    `the 6 identical copies in ${SAMPLES}`,
  );
});

test('a pair opens to what both have, what one side has and the copies that differ', async ({
  page,
}) => {
  await open(page);
  const title = 'fonts-old and Samples share fonts';
  await card(page, title)
    .getByRole('button', { name: `Show files of ${title}` })
    .click();
  const detail = page.getByTestId('folder-pair-detail');
  await expect(detail.getByRole('heading', { level: 2 })).toBeFocused();
  await expect(detail.locator('[data-group="both"] h3')).toHaveText('In both folders: 5 files');
  await expect(detail.locator('[data-group="differ"] h3')).toHaveText(
    'In both, not identical: 1 file',
  );
  await expect(detail.locator('[data-group="differ"]')).toContainText(
    `${OLD}/Sample12-Regular.ttf`,
  );
  await expect(detail.locator('[data-group="differ"]')).toContainText('Another version');
  await expect(detail.locator('[data-group="onlyA"] h3')).toHaveText('Only in fonts-old: 0 files');
  await expect(detail.locator('[data-group="onlyB"] h3')).toHaveText('Only in Samples: 12 files');

  // Esc goes back to the list, to the card it opened, not out of the module.
  await page.keyboard.press('Escape');
  await expect(detail).toHaveCount(0);
  await expect(card(page, title)).toBeFocused();
  await expect(page).toHaveURL(/section=folders/);
});

test('a path is shown in full on demand and opens in the file manager', async ({ page }) => {
  await open(page);
  const title = 'Samples is contained in Samples';
  const backup = side(page, title, BACKUP);
  const path = backup.getByTestId('pair-path');
  await expect(path).toHaveCSS('white-space', 'nowrap');
  await backup.getByRole('button', { name: 'Show the full path' }).click();
  await expect(path).toHaveCSS('white-space', 'normal');
  await expect(backup.getByRole('button', { name: 'Shorten the path' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.evaluate(() => {
    (globalThis as { __fmMockShownFolders?: string[] }).__fmMockShownFolders = [];
  });
  await backup.getByRole('button', { name: 'Show in file manager' }).click();
  expect(
    await page.evaluate(
      () => (globalThis as { __fmMockShownFolders?: string[] }).__fmMockShownFolders,
    ),
  ).toEqual([BACKUP]);
});

test('the list is virtualized and every card is reached with the keys', async ({ page }) => {
  // `=2` adds 20 archive folders that hold Sample 1–5: a few hundred pairs.
  await open(page, '?mockFolderPairs=2');
  const total = Number(
    await page
      .getByRole('navigation', { name: 'Duplicates' })
      .getByTestId('dup-count-folders')
      .textContent(),
  );
  expect(total).toBeGreaterThan(100);
  // Only the cards in the window (and a few beyond) exist.
  expect(await page.locator('.pairs > li').count()).toBeLessThan(15);

  await cards(page)
    .first()
    .getByRole('button', { name: /^Show files of/ })
    .focus();
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-index="1"]')).toBeFocused();
  await page.keyboard.press('End');
  const last = page.locator(`[data-index="${total - 1}"]`);
  await expect(last).toBeFocused();
  await expect(last).toBeInViewport();
  expect(await page.locator('.pairs > li').count()).toBeLessThan(15);
  // Tab reaches the buttons of the card in view.
  await page.keyboard.press('Tab');
  await expect(last.getByRole('button').first()).toBeFocused();
  await page.keyboard.press('Home');
  await expect(page.locator('[data-index="0"]')).toBeFocused();
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`the Folders view passes a WCAG 2.2 AA scan (${colorScheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme, reducedMotion: 'reduce' });
    await open(page);
    const title = 'fonts-old and Samples share fonts';
    await side(page, title, SAMPLES)
      .getByRole('button', { name: /^Keep this side/ })
      .click();
    await expect(card(page, title).getByTestId('pair-covers')).toBeVisible();
    const scan = async () => {
      const result = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(result.violations.map((v) => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
    };
    await scan();
    await card(page, title)
      .getByRole('button', { name: `Show files of ${title}` })
      .click();
    await expect(page.getByTestId('folder-pair-detail')).toBeVisible();
    await scan();
  });
}

test.describe('in German', () => {
  test.use({ locale: 'de-DE' });

  test('the Folders view speaks German', async ({ page }) => {
    await open(page);
    await expect(
      page.getByRole('heading', { level: 2, name: 'Ordner mit denselben Schriften' }),
    ).toBeVisible();
    const title = 'Supplemental ist ganz in Samples enthalten';
    await expect(card(page, title).getByTestId('pair-share')).toHaveText('Enthalten');
    await expect(
      card(page, title).getByRole('button', { name: /^Diese Seite behalten/ }),
    ).toHaveCount(1);
    await expect(card(page, title).getByTestId('pair-protected')).toHaveText(
      'Aus einem Systemschriftenordner wird nie etwas entfernt',
    );
    await expect(card(page, 'fonts-old und Samples teilen Schriften')).toContainText(
      '6 von 6 Dateien liegen im anderen Ordner',
    );
  });
});
