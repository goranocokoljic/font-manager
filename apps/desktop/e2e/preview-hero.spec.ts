import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test, type Locator, type Page } from '@playwright/test';
import { optionLabels, selectOption, showPreviewOptions } from './select';

// The hero row above the wall (E41.2). The wall library has 60 families.
const fixture = readFileSync(
  fileURLToPath(
    new URL(
      '../../../fixtures/fonts/google-repo/asapsharp/AsapSharp[wdth,wght].ttf',
      import.meta.url,
    ),
  ),
);

async function openWall(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.route('**/fmfont/*', (route) =>
    route.fulfill({ status: 200, contentType: 'font/ttf', body: fixture }),
  );
  await page.goto('/?mockLibrary=wall#/');
  await expect(page.getByRole('tree', { name: 'Font list' })).toBeVisible();
  await page
    .getByRole('group', { name: 'View mode' })
    .getByRole('button', { name: 'Grid' })
    .click();
  const wall = page.getByRole('listbox', { name: 'Font grid' });
  await expect(wall).toBeVisible();
  return wall;
}

const row = (page: Page) => page.getByRole('group', { name: 'Preview settings' });
const field = (page: Page) => row(page).getByRole('textbox', { name: 'Type to preview' });
const chip = (page: Page, name: string) =>
  row(page).getByRole('group', { name: 'Preset' }).getByRole('button', { name });
const firstCard = (wall: Locator) => wall.getByRole('option').first();

const setSize = (page: Page, value: number) =>
  page.getByRole('slider', { name: 'Size slider' }).evaluate((el: HTMLInputElement, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);

test.describe('the preview row', () => {
  test('a 44 px field with a hint, then chips, a divider and the language; nothing else', async ({
    page,
  }) => {
    await openWall(page);
    const box = await field(page).locator('xpath=..').boundingBox();
    expect(box!.height).toBe(44);
    await expect(field(page)).toHaveAttribute('placeholder', 'Type to preview');
    await expect(row(page).getByTestId('preview-hint')).toBeVisible();
    await expect(row(page).getByRole('group', { name: 'Preset' }).getByRole('button')).toHaveText([
      'Word',
      'Pangram',
      'Alphabet',
      'Numerals',
      'Paragraph',
    ]);
    await expect(row(page).getByLabel('Language')).toBeVisible();
    for (const name of ['Size in pixels', 'Line height', 'Surface', 'Auto size', 'Sample']) {
      await expect(row(page).getByLabel(name)).toBeHidden();
    }
    // The options button ends the row.
    const toggle = await row(page).getByRole('button', { name: 'Preview options' }).boundingBox();
    const language = await row(page).getByLabel('Language').boundingBox();
    expect(toggle!.x).toBeGreaterThan(language!.x + language!.width - 1);
  });

  test('at the smallest window the divider stays between the chips and the language', async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1000, height: 700 });
    await page.goto('/?mockLibrary=wall#/');
    await expect(field(page)).toBeVisible();
    const middle = async (target: Locator) => {
      const box = await target.boundingBox();
      return box!.y + box!.height / 2;
    };
    const chips = row(page).getByRole('group', { name: 'Preset' });
    const divider = row(page).locator('.divider');
    const language = row(page).getByLabel('Language');
    // One row: the field above it, and all of it on the screen.
    expect(await middle(divider)).toBeCloseTo(await middle(chips), 0);
    expect(await middle(language)).toBeCloseTo(await middle(chips), 0);
    expect(await middle(field(page))).toBeLessThan((await chips.boundingBox())!.y);
    const toggle = await row(page).getByRole('button', { name: 'Preview options' }).boundingBox();
    expect(toggle!.x + toggle!.width).toBeLessThanOrEqual(1000);
  });

  test('each preset is its own wall layout, as the boards draw it', async ({ page }) => {
    const wall = await openWall(page);
    await setSize(page, 60);
    const looks = [
      ['Word', '4', false, 229],
      ['Pangram', '2', false, 183],
      ['Alphabet', '2', true, 204],
      ['Numerals', '4', false, 184],
      ['Paragraph', '2', true, 184],
    ] as const;
    for (const [name, columns, live, height] of looks) {
      await chip(page, name).click();
      await expect(chip(page, name)).toHaveAttribute('aria-pressed', 'true');
      await expect(wall).toHaveAttribute('data-columns', columns);
      await expect
        .poll(async () => (await firstCard(wall).boundingBox())!.height)
        .toBeCloseTo(height, -1);
      await expect(firstCard(wall).getByTestId('card-live-text'), name).toHaveCount(live ? 1 : 0, {
        timeout: 5000,
      });
    }
  });

  test('Alphabet and Paragraph wrap whole lines inside the card, in the face', async ({ page }) => {
    const wall = await openWall(page);
    await setSize(page, 60);
    for (const name of ['Alphabet', 'Paragraph']) {
      await chip(page, name).click();
      const live = firstCard(wall).getByTestId('card-live-text');
      await expect(live).toHaveAttribute('data-state', 'ready');
      const fit = await live.evaluate((el) => {
        const line = el.querySelector<HTMLElement>('.line')!;
        return {
          lines: Math.round(line.offsetHeight / parseFloat(getComputedStyle(el).lineHeight)),
          inside: line.offsetHeight <= el.clientHeight + 0.5,
          wraps: getComputedStyle(el).whiteSpace,
        };
      });
      expect(fit.lines).toBeGreaterThan(1);
      expect(fit.inside).toBe(true);
      expect(fit.wraps).toBe('normal');
    }
    await expect(firstCard(wall).getByTestId('card-live-text')).toHaveCSS('font-size', '17px');
  });

  test('wrapped text is whole at the biggest size, and a long typed word stays inside', async ({
    page,
  }) => {
    const wall = await openWall(page);
    const whole = () =>
      firstCard(wall)
        .getByTestId('card-live-text')
        .evaluate((el) => {
          const line = el.querySelector<HTMLElement>('.line')!;
          return {
            wide: line.scrollWidth - line.clientWidth,
            tall: line.scrollHeight - line.clientHeight,
          };
        });
    await chip(page, 'Alphabet').click();
    await expect(firstCard(wall).getByTestId('card-live-text')).toHaveAttribute(
      'data-state',
      'ready',
    );
    for (const size of [80, 100, 120]) {
      await setSize(page, size);
      await expect(firstCard(wall).getByTestId('card-live-text')).toHaveCSS(
        'font-size',
        `${size * 0.42}px`,
      );
      await expect.poll(whole).toEqual({ wide: 0, tall: 0 });
    }
    await field(page).fill('W'.repeat(30));
    await expect(firstCard(wall).getByTestId('card-live-text')).toContainText('WWW');
    expect((await whole()).wide).toBe(0);
  });

  test('the slider scales by the preset; Paragraph stays at the body size', async ({ page }) => {
    const wall = await openWall(page);
    const fontSize = () =>
      firstCard(wall)
        .getByTestId('card-live-text')
        .evaluate((el) => parseFloat(getComputedStyle(el).fontSize));
    await chip(page, 'Alphabet').click();
    await setSize(page, 100);
    await expect.poll(fontSize).toBeCloseTo(42, 1);
    await chip(page, 'Paragraph').click();
    await setSize(page, 120);
    await expect.poll(fontSize).toBe(17);
    await setSize(page, 24);
    await expect.poll(fontSize).toBe(17);
  });

  test('typing repaints the cards on the next frame, with no transition', async ({ page }) => {
    const wall = await openWall(page);
    await chip(page, 'Word').click();
    await expect(firstCard(wall).getByRole('option')).toHaveCount(0);
    // In one task with the input event; the next animation frame reads what is painted.
    const painted = await field(page).evaluate(
      (el: HTMLInputElement) =>
        new Promise<{ text: string | null; transition: string | undefined }>((resolve) => {
          el.value = 'Hamburg';
          el.dispatchEvent(new Event('input', { bubbles: true }));
          requestAnimationFrame(() => {
            const live = document.querySelector('[role="listbox"] [data-testid="card-live-text"]');
            resolve({
              text: live?.textContent?.trim() ?? null,
              transition: live ? getComputedStyle(live).transitionDuration : undefined,
            });
          });
        }),
    );
    expect(painted).toEqual({ text: 'Hamburg', transition: '0s' });
    // The rendered thumbnail then takes over.
    await expect(firstCard(wall).getByTestId('thumbnail')).toHaveAttribute('data-state', 'ready');
  });

  test('the text is shared by the views; preset and size come back after a reload', async ({
    page,
  }) => {
    const wall = await openWall(page);
    await field(page).fill('Hamburgefonstiv');
    await chip(page, 'Numerals').click();
    await field(page).fill('Hamburgefonstiv');
    await setSize(page, 80);
    const modes = page.getByRole('group', { name: 'View mode' });
    await modes.getByRole('button', { name: 'List' }).click();
    await expect(page.locator('[role="treeitem"] .sample').first()).toHaveText('Hamburgefonstiv');
    await modes.getByRole('button', { name: 'Waterfall' }).click();
    await expect(page.locator('[role="tree"]').getByText('Hamburgefonstiv').first()).toBeVisible();
    await modes.getByRole('button', { name: 'Grid' }).click();
    await wall.getByRole('option').first().click();
    await page.keyboard.press('Space');
    const look = page.getByRole('dialog').first();
    await expect(look.getByText('Hamburgefonstiv').first()).toBeVisible();
    await page.keyboard.press('Escape');

    await page.reload();
    await expect(wall).toBeVisible();
    await expect(wall).toHaveAttribute('data-preset', 'numerals');
    await expect(field(page)).toHaveValue('Hamburgefonstiv');
    await expect(chip(page, 'Numerals')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('slider', { name: 'Size slider' })).toHaveValue('80');
  });

  test('the Language picker puts its pangram up and lists recent languages first', async ({
    page,
  }) => {
    const wall = await openWall(page);
    const language = row(page).getByLabel('Language');
    expect((await optionLabels(language))[0]).toBe('English');
    await selectOption(language, { label: 'Serbian (Cyrillic)' });
    await expect(field(page)).toHaveValue(/фењерџија/);
    await expect(wall).toHaveAttribute('data-preset', 'pangram');
    await expect(chip(page, 'Pangram')).toHaveAttribute('aria-pressed', 'true');
    expect((await optionLabels(language))[0]).toBe('Serbian (Cyrillic)');
  });

  test('the language it already shows puts its pangram up too', async ({ page }) => {
    const wall = await openWall(page);
    const language = row(page).getByLabel('Language');
    await field(page).fill('Hamburg');
    await selectOption(language, { label: 'English' });
    await expect(field(page)).toHaveValue('The quick brown fox jumps over the lazy dog');
    await expect(wall).toHaveAttribute('data-preset', 'pangram');

    await chip(page, 'Alphabet').click();
    await expect(wall).toHaveAttribute('data-preset', 'alphabet');
    await selectOption(language, { label: 'English' });
    await expect(wall).toHaveAttribute('data-preset', 'pangram');
    await expect(chip(page, 'Pangram')).toHaveAttribute('aria-pressed', 'true');
  });

  test('the options popover opens from the row, closes on a press outside and on Escape', async ({
    page,
  }) => {
    const wall = await openWall(page);
    const options = page.getByRole('group', { name: 'Preview options' });
    await expect(options).toBeHidden();
    await showPreviewOptions(page);
    await expect(options).toBeVisible();
    for (const name of [
      'Size in pixels',
      'Auto size',
      'Line height',
      'Tracking in em',
      'Surface',
    ]) {
      await expect(options.getByLabel(name)).toBeVisible();
    }
    // A Select's menu in it lives in <body>: choosing keeps the popover open.
    await selectOption(options.getByLabel('Surface'), 'dark');
    await expect(options).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(options).toBeHidden();
    await expect(row(page).getByRole('button', { name: 'Preview options' })).toBeFocused();

    await showPreviewOptions(page);
    await wall.click({ position: { x: 4, y: 4 } });
    await expect(options).toBeHidden();
  });

  test('keyboard: / focuses search, T the field, Escape returns to the wall', async ({ page }) => {
    const wall = await openWall(page);
    await wall.focus();
    await page.keyboard.press('t');
    await expect(field(page)).toBeFocused();
    // Typed in the field, T is a letter.
    await page.keyboard.type('Tt');
    await expect(field(page)).toHaveValue('Tt');
    await page.keyboard.press('Escape');
    await expect(wall).toBeFocused();

    await page.keyboard.press('/');
    await expect(page.getByRole('searchbox')).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(wall).toBeFocused();

    // With a row under the cursor T stays the tagging key.
    await firstCard(wall).click();
    await page.keyboard.press('t');
    await expect(field(page)).not.toBeFocused();
    await expect(page.getByRole('dialog', { name: /tag/i })).toBeVisible();
  });
});

// Screenshots of the five boards' layouts, for the review against dev-docs/design/redesign.
const dir = process.env.FM_SCREENS_DIR;
test.describe('@screens the wall presets', () => {
  test.skip(!dir, 'needs FM_SCREENS_DIR');
  for (const name of ['Word', 'Pangram', 'Alphabet', 'Numerals', 'Paragraph']) {
    test(`wall-${name.toLowerCase()}-1440x900`, async ({ page }) => {
      await page.emulateMedia({ reducedMotion: 'reduce' });
      const wall = await openWall(page);
      await setSize(page, 60);
      await chip(page, name).click();
      if (name === 'Word') await field(page).fill('Hamburg');
      await expect(firstCard(wall).locator('[data-state="ready"]')).toBeVisible();
      await page.screenshot({ path: `${dir}/wall-${name.toLowerCase()}-1440x900.png` });
    });
  }
});
