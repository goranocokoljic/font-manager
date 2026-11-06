import { expect, test, type Page } from '@playwright/test';
import { openSection, sectionPanel } from './family-page';

const info = (page: Page) => sectionPanel(page, 'Info');

test('the Info tab shows the selected face with every field', async ({ page }) => {
  const panel = await openSection(page, 1, 'Info');
  await expect(panel.getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
  await panel.getByRole('button', { name: 'Technical details' }).click();
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
  const panel = await openSection(page, 1, 'Info');
  await panel.getByRole('button', { name: 'Technical details' }).click();
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
  const panel = await openSection(page, 1, 'Info');
  await expect(panel.getByTestId('health-badge')).toHaveText('OK');
  await panel.getByRole('button', { name: 'Activate', exact: true }).click();
  await expect(panel.getByTestId('activation-state')).toHaveText('Active for this session');
  await panel.getByRole('button', { name: 'Deactivate' }).click();
  await expect(panel.getByTestId('activation-state')).toHaveText('Not active');
});

test('an unsupported face shows its health and cannot be activated', async ({ page }) => {
  const panel = await openSection(page, 7, 'Info');
  await expect(panel.getByTestId('health-badge')).toContainText('Unsupported');
  await expect(panel.getByTestId('health-badge')).toContainText('Type 1 fonts cannot be activated');
  await expect(panel.getByRole('button', { name: 'Activate', exact: true })).toBeDisabled();
});

test('a family page without a face in the URL describes its first style', async ({ page }) => {
  await page.goto('/#/family/1');
  await page.getByRole('group', { name: 'Section' }).getByRole('button', { name: 'Info' }).click();
  await expect(info(page).getByRole('heading', { name: 'Inter Regular' })).toBeVisible();
});

test('an activation from outside the inspector shows there without a click (E6.9)', async ({
  page,
}) => {
  const panel = await openSection(page, 1, 'Info');
  await expect(panel.getByTestId('activation-state')).toHaveText('Not active');
  await expect(panel.getByRole('button', { name: 'Activate', exact: true })).toBeVisible();

  // The top bar activates the family through the activation store, not the Info section;
  // only the activationChanged event tells the panel.
  await page.getByRole('button', { name: 'Activate family' }).click();

  await expect(panel.getByTestId('activation-state')).toHaveText('Active for this session');
  await expect(panel.getByRole('button', { name: 'Deactivate' })).toBeVisible();
});
