import { expect, test } from '@playwright/test';

// Settings → AI assistants (E27.1; board 15, E57.6): the opt-in switch for the MCP server, the
// address and the snippet block with one segment per client, all reachable and named for
// the keyboard.

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

test('the switch, the client segments and Copy work from the keyboard', async ({ page }) => {
  const toggle = page.getByRole('switch', { name: 'Allow AI assistants (MCP server)' });
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).toBeChecked();
  await expect(
    page.getByRole('button', {
      name: 'Copy MCP server address: http://127.0.0.1:47613/mcp',
    }),
  ).toBeVisible();
  const snippet = page.getByTestId('mcp-snippet');

  // Tab order: switch → address Copy → the three client segments → Show → Copy → How to connect.
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', { name: /^Copy MCP server address/ })).toBeFocused();
  await page.keyboard.press('Tab');
  const clients = page.getByRole('group', { name: 'Assistant' });
  const claudeCode = clients.getByRole('button', { name: 'Claude Code' });
  await expect(claudeCode).toBeFocused();
  await expect(claudeCode).toHaveAttribute('aria-pressed', 'true');
  // The token is dots until Show.
  await expect(snippet).toContainText('Authorization: Bearer ' + '•'.repeat(64));
  await expect(snippet).toHaveCSS('font-family', /mono|Menlo|Consolas/i);
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  const show = page.getByRole('button', { name: 'Show' });
  await expect(show).toBeFocused();
  await expect(show).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('button', { name: 'Hide' })).toHaveAttribute('aria-pressed', 'true');
  await expect(snippet).toContainText(/Bearer [0-9a-f]{64}/);
  await page.keyboard.press('Enter');
  await expect(snippet).not.toContainText(/Bearer [0-9a-f]{64}/);

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
  await expect(page.getByRole('button', { name: 'How to connect' })).toBeFocused();

  // Other HTTP clients, chosen by keyboard: its JSON config is copied.
  const http = clients.getByRole('button', { name: 'Other HTTP clients' });
  await http.focus();
  await page.keyboard.press('Enter');
  await expect(http).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Copy HTTP client config' }).press('Enter');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).not.toBe(command);
  const config = JSON.parse(await page.evaluate(() => navigator.clipboard.readText()));
  expect(config.mcpServers['font-manager']).toMatchObject({
    type: 'http',
    url: 'http://127.0.0.1:47613/mcp',
  });

  // Claude Desktop (E27.7): the bundled stdio shim by its absolute path, shown and copyable.
  const desktop = clients.getByRole('button', { name: 'Claude Desktop' });
  await desktop.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByLabel('Claude Desktop', { exact: true }).last()).toContainText(
    '/Applications/Font Fu.app/Contents/MacOS/fm-mcp-stdio',
  );
  await page.getByRole('button', { name: 'Copy Claude Desktop config' }).press('Enter');
  await expect
    .poll(async () => JSON.parse(await page.evaluate(() => navigator.clipboard.readText())))
    .toMatchObject({
      mcpServers: {
        'font-manager': { command: '/Applications/Font Fu.app/Contents/MacOS/fm-mcp-stdio' },
      },
    });

  // Off again: the address, the snippets and the tools go away.
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).not.toBeChecked();
  await expect(page.getByTestId('mcp-connect')).toHaveCount(0);
  await expect(page.getByTestId('mcp-address')).toHaveCount(0);
  await expect(page.getByTestId('mcp-grants')).toHaveCount(0);
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
    const group = page.getByRole('region', { name: 'What AI assistants may change' });
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
