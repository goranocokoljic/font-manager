import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../../api/client';
import { MOCK_ENDPOINT_URL, createMockClient, mockEndpointToken } from '../../api/mock';
import { setLocale } from '../../i18n';
import LocalApiSettings from './LocalApiSettings.vue';

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
  const wrapper = mount(LocalApiSettings, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

type Wrapper = Awaited<ReturnType<typeof mountIt>>;

const button = (w: Wrapper, name: string) =>
  w.findAll('button').find((b) => (b.attributes('aria-label') ?? b.text()) === name)!;
const tokenValue = (w: Wrapper) => w.get('#local-api-token-value').text();
/** The section's own status (CopyValue keeps its own for the address). */
const sectionStatus = (w: Wrapper) => w.get('p[role="status"]');

describe('Settings → Local API', () => {
  it('shows the address as a row with a running dot and Copy', async () => {
    const w = await mountIt();
    const row = w.get('[data-testid="local-api-address"]');
    expect(row.get('.title').text()).toBe('Address');
    expect(row.get('.value').text()).toBe(MOCK_ENDPOINT_URL);
    expect(row.find('.value .dot').exists()).toBe(true);
    const copy = row.get('button');
    expect(copy.text()).toBe('Copy');
    expect(copy.attributes('aria-label')).toBe(`Copy local API address: ${MOCK_ENDPOINT_URL}`);
    w.unmount();
  });

  it('masks the token until Show is pressed and never renders it while hidden', async () => {
    const w = await mountIt();
    const token = mockEndpointToken(0);
    const row = w.get('[data-testid="local-api-token"]');
    expect(row.get('.title').text()).toBe('Token');
    expect(row.get('.line').text()).toBe(
      'Keep the token secret: a program that has it can do everything in this app.',
    );
    expect(tokenValue(w)).toBe('•'.repeat(token.length));
    expect(w.html()).not.toContain(token);
    const show = button(w, 'Show');
    expect(show.attributes('aria-pressed')).toBe('false');
    await show.trigger('click');
    expect(tokenValue(w)).toBe(token);
    expect(button(w, 'Hide').attributes('aria-pressed')).toBe('true');
    await button(w, 'Hide').trigger('click');
    expect(tokenValue(w)).toBe('•'.repeat(token.length));
    expect(w.html()).not.toContain(token);
    w.unmount();
  });

  it('copies the token, not the dots, and says Copied in the status', async () => {
    const { copyText } = await import('../../api/clipboard');
    const w = await mountIt();
    await button(w, 'Copy token').trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(mockEndpointToken(0));
    expect(sectionStatus(w).text()).toBe('Copied');
    w.unmount();
  });

  it('Regenerate is a row of its own; it replaces the token and says so', async () => {
    const w = await mountIt();
    const old = (await client.endpointInfo()).token;
    const regenerate = button(w, 'Regenerate');
    const row = regenerate.element.closest('.ui-setting-row')!;
    expect(row.querySelector('.title')?.textContent).toBe('Regenerate the token');
    expect(row.querySelector('.line')?.textContent).toMatch(/stop working until they get/);
    await regenerate.trigger('click');
    await flushPromises();
    const fresh = (await client.endpointInfo()).token;
    expect(fresh).not.toBe(old);
    expect(fresh).toMatch(/^[0-9a-f]{64}$/);
    await button(w, 'Show').trigger('click');
    expect(tokenValue(w)).toBe(fresh);
    expect(sectionStatus(w).text()).toBe('A new token was made. The old token no longer works.');
    w.unmount();
  });

  it("says Not running. in the address's place when the endpoint is not running", async () => {
    client.endpointInfo = async () => ({ url: null, token: mockEndpointToken(0) });
    const w = await mountIt();
    const row = w.get('[data-testid="local-api-address"]');
    expect(row.text()).toContain('Not running.');
    expect(row.find('.dot').exists()).toBe(false);
    expect(row.find('button').exists()).toBe(false);
    w.unmount();
  });
});
