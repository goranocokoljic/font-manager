import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../../api/client';
import {
  MOCK_ENDPOINT_URL,
  MOCK_MCP_SHIM,
  createMockClient,
  mockEndpointToken,
} from '../../api/mock';
import { setLocale } from '../../i18n';
import McpSettings from './McpSettings.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('../../api/clipboard', () => ({ copyText: vi.fn(() => Promise.resolve()) }));

let client: FmClient;

beforeEach(() => {
  setLocale('en');
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => setClient(undefined));

async function mountIt() {
  const wrapper = mount(McpSettings, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

type Wrapper = Awaited<ReturnType<typeof mountIt>>;

const button = (w: Wrapper, name: string) =>
  w.findAll('button').find((b) => (b.attributes('aria-label') ?? b.text()) === name);
const toggle = (w: Wrapper) => w.get('[name="mcp-enabled"]');
const segment = (w: Wrapper, name: string) =>
  w.findAll('[role="group"] button').find((b) => b.attributes('aria-label') === name)!;
const snippetText = (w: Wrapper) => w.get('[data-testid="mcp-snippet"]').text();
const sectionStatus = (w: Wrapper) => w.findAll('p[role="status"]').at(-1)!;

async function turnOn(w: Wrapper) {
  await toggle(w).trigger('click');
  await flushPromises();
}

describe('Settings → AI assistants', () => {
  it('is off by default and shows no address, snippet or tools while off', async () => {
    const w = await mountIt();
    const input = toggle(w);
    expect(input.attributes('role')).toBe('switch');
    expect(input.attributes('aria-checked')).toBe('false');
    const row = input.element.closest('.ui-setting-row')!;
    expect(row.querySelector('.title')?.textContent).toBe('Allow AI assistants (MCP server)');
    expect(w.find('[data-testid="mcp-address"]').exists()).toBe(false);
    expect(w.find('[data-testid="mcp-connect"]').exists()).toBe(false);
    expect(w.find('[data-testid="mcp-grants"]').exists()).toBe(false);
    expect(w.html()).not.toContain(mockEndpointToken(0));
    w.unmount();
  });

  it('turns the server on through the API and shows the address as a row with Copy', async () => {
    const w = await mountIt();
    await turnOn(w);
    expect((await client.mcpStatus()).enabled).toBe(true);
    expect(toggle(w).attributes('aria-checked')).toBe('true');
    const row = w.get('[data-testid="mcp-address"]');
    expect(row.get('.value').text()).toBe(`${MOCK_ENDPOINT_URL}/mcp`);
    expect(row.find('.dot').exists()).toBe(true);
    expect(row.get('button').text()).toBe('Copy');
    w.unmount();
  });

  it('opens the help page on AI assistants from the snippet block', async () => {
    const openHelp = vi.spyOn(client, 'openHelp').mockResolvedValue();
    const w = await mountIt();
    await turnOn(w);
    await button(w, 'How to connect')!.trigger('click');
    expect(openHelp).toHaveBeenCalledWith('mcp');
    w.unmount();
  });

  it('offers one segment per client fm-api lists, in its order, the first chosen', async () => {
    const w = await mountIt();
    await turnOn(w);
    const segs = w.findAll('[role="group"] button');
    expect(segs.map((s) => s.attributes('aria-label'))).toEqual([
      'Claude Code',
      'Other HTTP clients',
      'Claude Desktop',
    ]);
    expect(segs[0].attributes('aria-pressed')).toBe('true');
    expect(w.get('[role="group"]').attributes('aria-label')).toBe('Assistant');
    w.unmount();
  });

  it('shows the chosen snippet with dots for the token; Show reveals it, Hide masks it', async () => {
    const w = await mountIt();
    await turnOn(w);
    const token = mockEndpointToken(0);
    const [claude] = (await client.mcpStatus()).snippets ?? [];
    expect(snippetText(w)).toBe(claude.text.replace(token, '•'.repeat(token.length)));
    expect(w.html()).not.toContain(token);
    const show = button(w, 'Show')!;
    expect(show.attributes('aria-pressed')).toBe('false');
    await show.trigger('click');
    expect(snippetText(w)).toBe(claude.text);
    expect(button(w, 'Hide')!.attributes('aria-pressed')).toBe('true');
    await button(w, 'Hide')!.trigger('click');
    expect(w.html()).not.toContain(token);
    w.unmount();
  });

  it('draws no snippet text while the token is not known, and no Show', async () => {
    client.endpointInfo = async () => {
      throw { code: 'io', message: 'no', detail: null };
    };
    const w = await mountIt();
    await turnOn(w);
    expect(snippetText(w)).toBe('');
    expect(button(w, 'Show')).toBeUndefined();
    expect(button(w, 'Copy Claude Code command')).toBeDefined();
    w.unmount();
  });

  it('copies the real snippet of the chosen client, token and all', async () => {
    const { copyText } = await import('../../api/clipboard');
    const w = await mountIt();
    await turnOn(w);
    const [claude, http] = (await client.mcpStatus()).snippets ?? [];
    expect(claude.text).not.toBe(http.text);

    await button(w, 'Copy Claude Code command')!.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(claude.text);
    expect(claude.text).toMatch(/^claude mcp add --transport http /);
    expect(claude.text).toContain(mockEndpointToken(0));
    expect(sectionStatus(w).text()).toBe('Copied');

    await segment(w, 'Other HTTP clients').trigger('click');
    expect(button(w, 'Copy Claude Code command')).toBeUndefined();
    await button(w, 'Copy HTTP client config')!.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(http.text);
    w.unmount();
  });

  it('offers Claude Desktop the stdio shim by its absolute path, without the token or Show', async () => {
    const { copyText } = await import('../../api/clipboard');
    const w = await mountIt();
    await turnOn(w);
    const desktop = (await client.mcpStatus()).snippets?.find((s) => s.client === 'claudeDesktop');
    expect(desktop).toBeDefined();
    await segment(w, 'Claude Desktop').trigger('click');
    expect(w.get('[data-testid="mcp-snippet"]').attributes('aria-label')).toBe('Claude Desktop');
    expect(snippetText(w)).toBe(desktop!.text);
    expect(button(w, 'Show')).toBeUndefined();
    await button(w, 'Copy Claude Desktop config')!.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(desktop!.text);
    const config = JSON.parse(desktop!.text);
    expect(config.mcpServers['font-manager'].command).toBe(MOCK_MCP_SHIM);
    expect(desktop!.text).not.toContain(mockEndpointToken(0));
    w.unmount();
  });

  it('turns the server off again', async () => {
    const w = await mountIt();
    await turnOn(w);
    await toggle(w).trigger('click');
    await flushPromises();
    expect((await client.mcpStatus()).enabled).toBe(false);
    expect(w.find('[data-testid="mcp-connect"]').exists()).toBe(false);
    expect(w.find('[data-testid="mcp-grants"]').exists()).toBe(false);
    w.unmount();
  });

  it('keeps the switch off when the API refuses', async () => {
    client.setMcpEnabled = async () => {
      throw { code: 'unsupported', message: 'The MCP server is not available here.' };
    };
    const w = await mountIt();
    await turnOn(w);
    expect(toggle(w).attributes('aria-checked')).toBe('false');
    w.unmount();
  });

  it('says so when the endpoint is not running', async () => {
    client.mcpStatus = async () => ({ enabled: true, url: null, snippets: [] });
    const w = await mountIt();
    expect(w.get('[data-testid="mcp-address"]').text()).toContain('The local API is not running');
    expect(w.find('[data-testid="mcp-connect"]').exists()).toBe(false);
    w.unmount();
  });

  it('shows one switch row per mutating tool, all off, and saves a grant through the API', async () => {
    const w = await mountIt();
    await turnOn(w);
    const group = w.get('[data-testid="mcp-grants"]');
    expect(group.get('h3').text()).toBe('What AI assistants may change');
    expect(group.get('.note').text()).toMatch(/^Searching, reading font details/);
    const grants = group.findAll('[name^="mcp-grant-"]');
    expect(grants.map((g) => g.attributes('name'))).toEqual([
      'mcp-grant-activate_fonts',
      'mcp-grant-deactivate_fonts',
      'mcp-grant-tag_fonts',
      'mcp-grant-export_web',
    ]);
    const title = (id: string | undefined) => document.getElementById(id ?? '')?.textContent;
    expect(grants.map((g) => title(g.attributes('aria-labelledby')))).toEqual([
      'Activate fonts',
      'Deactivate fonts',
      'Tag fonts',
      'Export web fonts',
    ]);
    expect(grants.every((g) => g.attributes('aria-checked') === 'false')).toBe(true);
    // Only deactivate_fonts asks every time; its row says so as its line.
    const deactivate = w.get('[name="mcp-grant-deactivate_fonts"]');
    expect(title(deactivate.attributes('aria-describedby'))).toBe(
      'Asks you every time (Allow / Deny)',
    );
    expect(w.get('[name="mcp-grant-tag_fonts"]').attributes('aria-describedby')).toBe(undefined);

    await w.get('[name="mcp-grant-tag_fonts"]').trigger('click');
    await flushPromises();
    const granted = (await client.mcpStatus()).grants?.filter((g) => g.granted);
    expect(granted?.map((g) => g.tool)).toEqual(['tag_fonts']);
    expect(w.get('[name="mcp-grant-tag_fonts"]').attributes('aria-checked')).toBe('true');
    w.unmount();
  });

  it('leaves a refused grant where the API has it', async () => {
    const w = await mountIt();
    await turnOn(w);
    vi.spyOn(client, 'setMcpGrant').mockRejectedValueOnce({
      code: 'io',
      message: 'The settings could not be saved.',
      detail: null,
    });
    await w.get('[name="mcp-grant-tag_fonts"]').trigger('click');
    await flushPromises();
    expect(w.get('[name="mcp-grant-tag_fonts"]').attributes('aria-checked')).toBe('false');
    w.unmount();
  });
});
