import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';

// E48.6: the toasts sit at the top right under the toolbar, stack newest on top up to three,
// and are click-through except for their action button.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function open(page: Page) {
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockFaces=3&mockStyles=4#/');
  const tree = page.getByRole('tree', { name: 'Font list' });
  await expect(tree.getByRole('treeitem').first()).toBeVisible();
  return tree;
}

/** Every toast on screen, newest first in the page order. */
const stack = (page: Page) => page.locator('[data-testid="toast"], [data-testid="toast-stacked"]');

async function box(l: Locator) {
  const b = await l.boundingBox();
  expect(b).not.toBeNull();
  return b!;
}

/** What the pointer would hit at the middle of `l`: the element's test id, class and tag. */
const hitAt = (page: Page, b: { x: number; y: number; width: number; height: number }) =>
  page.evaluate(
    ([x, y]) => {
      const el = document.elementFromPoint(x!, y!);
      return {
        inToast: !!el?.closest('.toast'),
        tag: el?.tagName ?? '',
        text: el?.textContent ?? '',
      };
    },
    [b.x + b.width / 2, b.y + b.height / 2],
  );

const overlaps = (
  a: { x: number; y: number; width: number; height: number },
  b: { x: number; y: number; width: number; height: number },
) => a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

test('toasts stack top right under the toolbar, click-through but for Undo', async ({ page }) => {
  const tree = await open(page);
  await tree.getByRole('treeitem', { name: /^Sample 1,/ }).click();
  await page.keyboard.press('Shift+ArrowDown');
  const bar = page.getByRole('toolbar', { name: 'Selection' });
  await expect(bar).toBeVisible();

  // An Undo toast, then two more on top of it.
  await page.keyboard.press('a');
  await expect(page.getByTestId('toast')).toContainText('Activated 8 fonts for this session.');
  await page.keyboard.press('b');
  await expect(page.getByTestId('toast')).toContainText('Quick Collection');
  await page.keyboard.press('b');
  await expect(stack(page)).toHaveCount(3);
  await expect(stack(page).last()).toContainText('Activated 8 fonts');

  // Top right, under the toolbar, newest on top. Read once the stack is at rest: the newest
  // toast slides in 8 px from below, and the stack takes its place under the bar in the next
  // frame. On the macOS CI VM (15–30 fps) the boxes were read before that frame: 64 < 65.
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          document
            .getAnimations()
            .filter((a) =>
              (a.effect as KeyframeEffect | null)?.target?.closest?.('.region[role="status"]'),
            ).length,
      ),
    )
    .toBe(0);
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => done(null))));
  const viewport = page.viewportSize()!;
  const header = await box(page.locator('[data-top-bar]'));
  const boxes = await Promise.all([0, 1, 2].map((i) => box(stack(page).nth(i))));
  const regionTop = await page
    .locator('.region[role="status"]')
    .evaluate((r) => (r as HTMLElement).style.top);
  expect(
    boxes[0]!.y,
    `stack top ${regionTop}, bar ${JSON.stringify(header)}`,
  ).toBeGreaterThanOrEqual(header.y + header.height);
  expect(boxes[0]!.y).toBeLessThan(header.y + header.height + 24);
  expect(boxes[0]!.y).toBeLessThan(boxes[1]!.y);
  expect(boxes[1]!.y).toBeLessThan(boxes[2]!.y);
  for (const b of boxes) {
    expect(viewport.width - (b.x + b.width)).toBeLessThanOrEqual(24);
    // Off the middle of the window, where the Compare sheet's handle sits.
    expect(b.x).toBeGreaterThan(viewport.width / 2);
    // Never over the selection bar.
    expect(overlaps(b, await box(bar))).toBe(false);
  }

  // The message is click-through: the pointer reaches the page under it.
  const message = stack(page).first().locator('.message');
  expect((await hitAt(page, await box(message))).inToast).toBe(false);
  // The Undo button is the one exception.
  const undo = stack(page).last().getByRole('button', { name: 'Undo' });
  const onUndo = await hitAt(page, await box(undo));
  expect(onUndo).toMatchObject({ inToast: true, tag: 'BUTTON', text: 'Undo' });
  await undo.click();
  await expect(page.getByTestId('toast')).toContainText('Undone.');

  // A fourth pushes the oldest out: never more than three.
  for (let i = 0; i < 3; i++) await page.keyboard.press('b');
  await expect(stack(page)).toHaveCount(3);
  await expect(stack(page).filter({ hasText: 'Undone.' })).toHaveCount(0);
});
