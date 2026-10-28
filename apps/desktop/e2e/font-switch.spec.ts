import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E34.1: dragging a font out of the window to a design app, and the keyboard's
// "Copy for Font Switch". MockApi face 1 is Inter Regular, the first face of
// the Inter family; Legacy Grotesk is a Type 1 font, which cannot be activated.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

const INTER = {
  postscriptName: 'Inter-Regular',
  family: 'Inter',
  style: 'Regular',
  fileUrl: 'file:///Users/demo/Fonts/Inter/Inter%5Bwght%5D.ttf',
};

async function open(page: Page, hash = '#/') {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto(`/${hash}`);
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

/**
 * Presses the pointer on `row` and starts a drag from it, again until the drag
 * carries the Font Switch type (the payload is read when the pointer goes
 * down); returns what the drag carries, type by type.
 */
async function dragOut(page: Page, row: Locator): Promise<Record<string, string>> {
  await row.dispatchEvent('pointerdown');
  let carried: Record<string, string> = {};
  await expect
    .poll(async () => {
      const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
      await row.dispatchEvent('dragstart', { dataTransfer });
      carried = await dataTransfer.evaluate((dt) =>
        Object.fromEntries(dt.types.map((type) => [type, dt.getData(type)])),
      );
      return Object.keys(carried);
    })
    .toEqual([
      'application/x-fm-fonts',
      'application/x-fm-font-switch',
      'text/plain',
      'text/uri-list',
    ]);
  return carried;
}

/** The drag leaves the window over its left edge. */
async function leaveWindow(page: Page) {
  await page.evaluate(() =>
    document.dispatchEvent(new DragEvent('dragleave', { clientX: -1, clientY: 40 })),
  );
}

const dot = (page: Page, family: string) =>
  page.getByRole('treeitem', { name: new RegExp(`^${family},`) }).locator('[data-activation]');

test('a row dragged out of the window carries the payload and activates its face', async ({
  page,
}) => {
  const tree = await open(page);
  const inter = tree.getByRole('treeitem', { name: /^Inter,/ });
  await expect(dot(page, 'Inter')).toHaveAttribute('data-activation', 'inactive');

  const carried = await dragOut(page, inter);
  expect(JSON.parse(carried['application/x-fm-font-switch']!)).toEqual(INTER);
  expect(carried['text/plain']).toBe('Inter-Regular');
  expect(carried['text/uri-list']).toBe(INTER.fileUrl);
  expect(JSON.parse(carried['application/x-fm-fonts']!)).toEqual({ faces: [], families: [1] });
  await expect(dot(page, 'Inter')).toHaveAttribute('data-activation', 'inactive');

  await leaveWindow(page);
  await expect(page.getByText(/fonts? active$/)).toHaveText('1 font active');
  await expect(dot(page, 'Inter')).not.toHaveAttribute('data-activation', 'inactive');

  // The drag ends; the face stays active.
  await page.evaluate(() => document.dispatchEvent(new DragEvent('dragend')));
  await expect(page.getByText(/fonts? active$/)).toHaveText('1 font active');
  await expect(page.getByTestId('toast')).toHaveCount(0);
});

test('a face that cannot be activated for the drop shows a toast', async ({ page }) => {
  const tree = await open(page);
  await dragOut(page, tree.getByRole('treeitem', { name: /^Legacy Grotesk,/ }));
  await leaveWindow(page);
  await expect(page.getByTestId('toast')).toContainText(
    'Legacy Grotesk Regular could not be activated for the drop',
  );
  await expect(page.getByText(/fonts? active$/)).toHaveText('0 fonts active');
});

test('"Copy for Font Switch" is reached by the keyboard and copies the payload', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, '#/?face=1');
  await page.getByRole('tab', { name: 'Developer' }).click();
  const button = page.getByRole('button', { name: 'Copy Inter Regular for Font Switch' });
  await expect(button).toHaveText('Copy for Font Switch');

  // Tab from the tab: the panel, its Copy button, then this one.
  await page.getByRole('tab', { name: 'Developer' }).focus();
  for (let i = 0; i < 5; i++) {
    if (await button.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('Tab');
  }
  await expect(button).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('font-switch-status')).toHaveText('Copied');
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(JSON.parse(copied)).toEqual(INTER);
});
