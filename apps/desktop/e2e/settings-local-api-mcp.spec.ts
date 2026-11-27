// E57.6: Settings › Local API and AI assistants as on board 15 pass axe (contrast included)
// in both themes, in the states the board draws: the token hidden and shown, the server on
// with the snippet block and the tools.
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

for (const scheme of ['light', 'dark'] as const) {
  test(`Local API with the token hidden and shown passes axe (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/settings?section=localApi');
    await expect(page.locator('#local-api-token-value')).toHaveText(/^•{64}$/);
    let results = await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze();
    expect(results.violations.map((v) => v.id)).toEqual([]);
    await page.getByRole('button', { name: 'Show' }).click();
    await expect(page.locator('#local-api-token-value')).toHaveText(/^[0-9a-f]{64}$/);
    results = await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze();
    expect(results.violations.map((v) => v.id)).toEqual([]);
  });

  test(`AI assistants off and on, with the snippet and the tools, passes axe (${scheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto('/#/settings?section=mcp');
    const server = page.getByRole('switch', { name: 'Allow AI assistants (MCP server)' });
    await expect(server).not.toBeChecked();
    let results = await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze();
    expect(results.violations.map((v) => v.id)).toEqual([]);
    await server.click();
    await expect(page.getByTestId('mcp-connect')).toBeVisible();
    await page.getByRole('switch', { name: 'Tag fonts' }).click();
    await expect(page.getByRole('switch', { name: 'Tag fonts' })).toBeChecked();
    results = await new AxeBuilder({ page }).include('[role="tabpanel"]').analyze();
    expect(results.violations.map((v) => v.id)).toEqual([]);
  });
}
