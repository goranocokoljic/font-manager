import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { MOCK_ENDPOINT_URL, createMockClient, mockEndpointToken } from '../api/mock';
import { setLocale } from '../i18n';
import McpSettings from './McpSettings.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('../api/clipboard', () => ({ copyText: vi.fn(() => Promise.resolve()) }));

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
const toggle = (w: Wrapper) => w.get('input[role="switch"]');

async function turnOn(w: Wrapper) {
  const input = toggle(w);
  (input.element as HTMLInputElement).checked = true;
  await input.trigger('change');
  await flushPromises();
}

describe('Settings → AI assistants', () => {
  it('is off by default and shows no snippets while off', async () => {
    const w = await mountIt();
    const input = toggle(w);
    expect((input.element as HTMLInputElement).checked).toBe(false);
    expect(input.attributes('aria-checked')).toBe('false');
    expect(w.get('label').text()).toBe('Allow AI assistants (MCP server)');
    expect(button(w, 'Copy Claude Code command')).toBeUndefined();
    expect(w.text()).not.toContain(mockEndpointToken(0));
    w.unmount();
  });

  it('turns the server on through the API and offers both snippets', async () => {
    const w = await mountIt();
    await turnOn(w);
    expect((await client.mcpStatus()).enabled).toBe(true);
    expect(toggle(w).attributes('aria-checked')).toBe('true');
    expect(w.text()).toContain(`${MOCK_ENDPOINT_URL}/mcp`);
    expect(button(w, 'Copy Claude Code command')).toBeDefined();
    expect(button(w, 'Copy HTTP client config')).toBeDefined();
    w.unmount();
  });

  it('copies each snippet with the real address and token', async () => {
    const { copyText } = await import('../api/clipboard');
    const w = await mountIt();
    await turnOn(w);
    const [claude, http] = (await client.mcpStatus()).snippets ?? [];
    expect(claude.text).not.toBe(http.text);

    await button(w, 'Copy Claude Code command')!.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(claude.text);
    expect(claude.text).toMatch(/^claude mcp add --transport http /);
    expect(claude.text).toContain(mockEndpointToken(0));

    await button(w, 'Copy HTTP client config')!.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(http.text);
    expect(w.get('p[role="status"]').text()).toBe('Copied');
    w.unmount();
  });

  it('hides the snippet text until Show snippets is pressed', async () => {
    const w = await mountIt();
    await turnOn(w);
    const token = mockEndpointToken(0);
    expect(w.text()).not.toContain(token);
    const show = button(w, 'Show snippets')!;
    expect(show.attributes('aria-pressed')).toBe('false');
    await show.trigger('click');
    expect(w.findAll('pre')).toHaveLength(2);
    expect(w.text()).toContain(token);
    await button(w, 'Hide snippets')!.trigger('click');
    expect(w.text()).not.toContain(token);
    w.unmount();
  });

  it('turns the server off again', async () => {
    const w = await mountIt();
    await turnOn(w);
    const input = toggle(w);
    (input.element as HTMLInputElement).checked = false;
    await input.trigger('change');
    await flushPromises();
    expect((await client.mcpStatus()).enabled).toBe(false);
    expect(button(w, 'Copy Claude Code command')).toBeUndefined();
    w.unmount();
  });

  it('keeps the switch off when the API refuses', async () => {
    client.setMcpEnabled = async () => {
      throw { code: 'unsupported', message: 'The MCP server is not available here.' };
    };
    const w = await mountIt();
    await turnOn(w);
    expect((toggle(w).element as HTMLInputElement).checked).toBe(false);
    w.unmount();
  });

  it('says so when the endpoint is not running', async () => {
    client.mcpStatus = async () => ({ enabled: true, url: null, snippets: [] });
    const w = await mountIt();
    expect(w.text()).toContain('The local API is not running');
    expect(button(w, 'Show snippets')).toBeUndefined();
    w.unmount();
  });

  it('shows one switch per mutating tool, all off, and saves a grant through the API', async () => {
    const w = await mountIt();
    await turnOn(w);
    const grants = w.findAll('input[name^="mcp-grant-"]');
    expect(grants.map((g) => g.attributes('name'))).toEqual([
      'mcp-grant-activate_fonts',
      'mcp-grant-deactivate_fonts',
      'mcp-grant-tag_fonts',
      'mcp-grant-export_web',
    ]);
    expect(grants.map((g) => g.element.closest('label')?.textContent?.trim())).toEqual([
      'Activate fonts',
      'Deactivate fonts',
      'Tag fonts',
      'Export web fonts',
    ]);
    expect(grants.every((g) => !(g.element as HTMLInputElement).checked)).toBe(true);
    // Only deactivate_fonts asks every time; its switch says so.
    const deactivate = w.get('input[name="mcp-grant-deactivate_fonts"]');
    expect(deactivate.attributes('aria-describedby')).toBe('mcp-grant-deactivate_fonts-hint');
    expect(w.get('#mcp-grant-deactivate_fonts-hint').text()).toBe(
      'Asks you every time (Allow / Deny)',
    );
    expect(w.get('input[name="mcp-grant-tag_fonts"]').attributes('aria-describedby')).toBe(
      undefined,
    );

    const tag = w.get('input[name="mcp-grant-tag_fonts"]');
    (tag.element as HTMLInputElement).checked = true;
    await tag.trigger('change');
    await flushPromises();
    const granted = (await client.mcpStatus()).grants?.filter((g) => g.granted);
    expect(granted?.map((g) => g.tool)).toEqual(['tag_fonts']);
    expect(w.get('input[name="mcp-grant-tag_fonts"]').attributes('aria-checked')).toBe('true');
    w.unmount();
  });

  it('puts a refused grant back where the API has it', async () => {
    const w = await mountIt();
    await turnOn(w);
    vi.spyOn(client, 'setMcpGrant').mockRejectedValueOnce({
      code: 'io',
      message: 'The settings could not be saved.',
      detail: null,
    });
    const tag = w.get('input[name="mcp-grant-tag_fonts"]');
    (tag.element as HTMLInputElement).checked = true;
    await tag.trigger('change');
    await flushPromises();
    expect((tag.element as HTMLInputElement).checked).toBe(false);
    w.unmount();
  });
});
