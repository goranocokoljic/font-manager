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

  // Tab order: switch → address → Show snippets → Copy Claude Code → Copy HTTP config
  // → Copy Claude Desktop config.
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
  await expect(page.locator('pre')).toHaveCount(3);

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

  // Claude Desktop (E27.7): the bundled stdio shim by its absolute path, shown and copyable.
  await page.keyboard.press('Tab');
  const desktop = page.getByRole('button', { name: 'Copy Claude Desktop config' });
  await expect(desktop).toBeFocused();
  const desktopSnippet = page.getByLabel('Claude Desktop (starts the bundled fm-mcp-stdio)');
  await expect(desktopSnippet).toBeVisible();
  await expect(desktopSnippet).toContainText(
    '/Applications/Font Manager.app/Contents/MacOS/fm-mcp-stdio',
  );
  await page.keyboard.press('Enter');
  await expect
    .poll(async () => JSON.parse(await page.evaluate(() => navigator.clipboard.readText())))
    .toMatchObject({
      mcpServers: {
        'font-manager': { command: '/Applications/Font Manager.app/Contents/MacOS/fm-mcp-stdio' },
      },
    });

  // Off again: the snippets go away.
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).not.toBeChecked();
  await expect(page.getByRole('button', { name: 'Copy Claude Code command' })).toHaveCount(0);
});

// E27.6: one switch per mutating tool, and the Allow / Deny toast of a destructive
// call. `?mockMcpConfirm=1`: turning "Deactivate fonts" on makes a mock AI
// assistant ask to deactivate 3 fonts.
test.describe('tool grants and confirmations', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/?mockMcpConfirm=1#/settings?section=mcp');
    const server = page.getByRole('switch', { name: 'Allow AI assistants (MCP server)' });
    await server.focus();
    await page.keyboard.press('Space');
    await expect(server).toBeChecked();
  });

  test('every mutating tool has a labelled switch, all off', async ({ page }) => {
    const group = page.getByRole('group', { name: 'What AI assistants may change' });
    for (const name of ['Activate fonts', 'Deactivate fonts', 'Tag fonts', 'Export web fonts']) {
      await expect(group.getByRole('switch', { name, exact: true })).not.toBeChecked();
    }
    await expect(
      group.getByRole('switch', { name: 'Deactivate fonts' }),
    ).toHaveAccessibleDescription('Asks you every time (Allow / Deny)');
    // Turned on from the keyboard; the grant is saved and stays on.
    const tag = group.getByRole('switch', { name: 'Tag fonts' });
    await tag.focus();
    await page.keyboard.press('Space');
    await expect(tag).toBeChecked();
    await expect(page.getByTestId('mcp-confirm')).toHaveCount(0);
  });

  test('a confirmation is answered from the keyboard, Deny then Allow', async ({ page }) => {
    const deactivate = page.getByRole('switch', { name: 'Deactivate fonts' });
    const request = page.getByRole('region', { name: 'Request from an AI assistant' });

    await deactivate.focus();
    await page.keyboard.press('Space');
    await expect(deactivate).toBeChecked();
    await expect(request).toContainText('An AI assistant wants to deactivate 3 fonts.');
    await expect(request).toHaveAccessibleDescription(/No answer in 60 seconds counts as Deny/);
    const deny = request.getByRole('button', { name: 'Deny' });
    await deny.focus();
    await page.keyboard.press('Enter');
    await expect(request).toHaveCount(0);
    await expect(page.getByTestId('toast')).toContainText('Denied. Nothing was changed.');

    // Off and on again: the mock assistant asks a second time.
    await deactivate.focus();
    await page.keyboard.press('Space');
    await expect(deactivate).not.toBeChecked();
    await page.keyboard.press('Space');
    await expect(request).toBeVisible();
    const allow = request.getByRole('button', { name: 'Allow' });
    await allow.focus();
    await page.keyboard.press('Enter');
    await expect(request).toHaveCount(0);
    await expect(page.getByTestId('toast')).toContainText(
      "Allowed. The AI assistant's request runs now.",
    );
  });
});
