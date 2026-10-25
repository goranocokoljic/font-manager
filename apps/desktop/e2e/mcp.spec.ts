import { expect, test } from '@playwright/test';

// Settings → AI assistants (E27.1): the opt-in switch for the MCP server and the
// copyable client snippets, all reachable and named for the keyboard.

test.beforeEach(async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/#/settings?section=mcp');
});

test('the AI assistants section is off by default', async ({ page }) => {
  await expect(page.getByRole('heading', { name: 'AI assistants', level: 2 })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'AI assistants' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  const toggle = page.getByRole('switch', { name: 'Allow AI assistants (MCP server)' });
  await expect(toggle).not.toBeChecked();
  await expect(toggle).toHaveAccessibleDescription(/Off by default/);
  await expect(page.getByRole('button', { name: 'Copy Claude Code command' })).toHaveCount(0);
});

test('the switch and the copy buttons work from the keyboard', async ({ page }) => {
  const toggle = page.getByRole('switch', { name: 'Allow AI assistants (MCP server)' });
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).toBeChecked();
  await expect(
    page.getByRole('button', {
      name: 'Copy MCP server address: http://127.0.0.1:47613/mcp',
    }),
  ).toBeVisible();

  // Tab order: switch → address → Show snippets → Copy Claude Code → Copy HTTP config.
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: /^Copy MCP server address/ })).toBeFocused();
  await page.keyboard.press('Tab');
  const show = page.getByRole('button', { name: 'Show snippets' });
  await expect(show).toBeFocused();
  await expect(show).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Hide snippets' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('pre')).toHaveCount(2);

  await page.keyboard.press('Tab');
  const claude = page.getByRole('button', { name: 'Copy Claude Code command' });
  await expect(claude).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('status').filter({ hasText: 'Copied' }).last()).toBeVisible();
  const command = await page.evaluate(() => navigator.clipboard.readText());
  expect(command).toMatch(
    /^claude mcp add --transport http font-manager http:\/\/127\.0\.0\.1:47613\/mcp --header "Authorization: Bearer [0-9a-f]{64}"$/,
  );

  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: 'Copy HTTP client config' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).not.toBe(command);
  const config = JSON.parse(await page.evaluate(() => navigator.clipboard.readText()));
  expect(config.mcpServers['font-manager']).toMatchObject({
    type: 'http',
    url: 'http://127.0.0.1:47613/mcp',
  });

  // Off again: the snippets go away.
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).not.toBeChecked();
  await expect(page.getByRole('button', { name: 'Copy Claude Code command' })).toHaveCount(0);
});
