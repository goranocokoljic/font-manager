import { expect, test, type Page } from '@playwright/test';

const info = (page: Page) => page.getByRole('tabpanel', { name: 'Info' });

test('the Info tab shows the selected face with every field', async ({ page }) => {
  await page.goto('/#/?face=1');
  await expect(page.getByRole('tab', { name: 'Info' })).toHaveAttribute('aria-selected', 'true');
  const panel = info(page);
  await expect(panel.getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  for (const section of [
    'Names',
    'File',
    'Style',
    'Metrics',
    'Characters and features',
    'License',
  ]) {
    await expect(panel.getByRole('region', { name: section })).toBeVisible();
  }
  for (const text of [
    'Inter-Regular',
    '/Users/demo/Fonts/Inter/Inter[wght].ttf',
    'Version 1.000',
    'rsms',
    'Rasmus Andersson',
    'wght Weight: 100–900 (default 400)',
    'SIL Open Font License 1.1',
    'Installable',
  ]) {
    await expect(panel).toContainText(text);
  }
});

test('names and paths copy on click', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/#/?face=1');
  const panel = info(page);
  for (const [label, value] of [
    ['PostScript name', 'Inter-Regular'],
    ['path', '/Users/demo/Fonts/Inter/Inter[wght].ttf'],
  ]) {
    const button = panel.getByRole('button', { name: `Copy ${label}: ${value}` });
    await button.click();
    // The status next to this button: an earlier "Copied" may still show for 1.5 s.
    await expect(button.locator('xpath=following-sibling::*[@role="status"][1]')).toHaveText(
      'Copied',
    );
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(value);
  }
});

test('the activation button activates and deactivates the face', async ({ page }) => {
  await page.goto('/#/?face=1');
  const panel = info(page);
  await expect(panel.getByTestId('health-badge')).toHaveText('OK');
  await panel.getByRole('button', { name: 'Activate' }).click();
  await expect(panel.getByTestId('activation-state')).toHaveText('Active for this session');
  await panel.getByRole('button', { name: 'Deactivate' }).click();
  await expect(panel.getByTestId('activation-state')).toHaveText('Not active');
});

test('an unsupported face shows its health and cannot be activated', async ({ page }) => {
  await page.goto('/#/?face=7');
  const panel = info(page);
  await expect(panel.getByTestId('health-badge')).toContainText('Unsupported');
  await expect(panel.getByTestId('health-badge')).toContainText('Type 1 fonts cannot be activated');
  await expect(panel.getByRole('button', { name: 'Activate' })).toBeDisabled();
});

test('nothing selected without a face in the URL', async ({ page }) => {
  await page.goto('/');
  await expect(info(page)).toContainText('Nothing selected');
});
