import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { MOCK_ENDPOINT_URL, createMockClient, mockEndpointToken } from '../api/mock';
import { setLocale } from '../i18n';
import LocalApiSettings from './LocalApiSettings.vue';

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
  const wrapper = mount(LocalApiSettings, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

const button = (w: Awaited<ReturnType<typeof mountIt>>, name: string) =>
  w.findAll('button').find((b) => (b.attributes('aria-label') ?? b.text()) === name)!;

describe('Settings → Local API', () => {
  it('shows the address and masks the token until Show is pressed', async () => {
    const w = await mountIt();
    const token = mockEndpointToken(0);
    expect(w.text()).toContain(MOCK_ENDPOINT_URL);
    const input = w.get('input#local-api-token');
    expect(w.get('label[for="local-api-token"]').text()).toBe('Token');
    expect((input.element as HTMLInputElement).value).toBe('•'.repeat(token.length));
    expect(w.text()).not.toContain(token);
    const show = button(w, 'Show');
    expect(show.attributes('aria-pressed')).toBe('false');
    await show.trigger('click');
    expect((input.element as HTMLInputElement).value).toBe(token);
    expect(button(w, 'Hide').attributes('aria-pressed')).toBe('true');
    await button(w, 'Hide').trigger('click');
    expect((input.element as HTMLInputElement).value).not.toContain(token.slice(0, 8));
    w.unmount();
  });

  it('copies the token, not the dots', async () => {
    const { copyText } = await import('../api/clipboard');
    const w = await mountIt();
    await button(w, 'Copy token').trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(mockEndpointToken(0));
    expect(w.get('p[role="status"]').text()).toBe('Copied');
    w.unmount();
  });

  it('regenerate replaces the token and says the old one no longer works', async () => {
    const w = await mountIt();
    const old = (await client.endpointInfo()).token;
    await button(w, 'Regenerate').trigger('click');
    await flushPromises();
    const fresh = (await client.endpointInfo()).token;
    expect(fresh).not.toBe(old);
    expect(fresh).toMatch(/^[0-9a-f]{64}$/);
    await button(w, 'Show').trigger('click');
    expect((w.get('input#local-api-token').element as HTMLInputElement).value).toBe(fresh);
    expect(w.get('p[role="status"]').text()).toBe(
      'A new token was made. The old token no longer works.',
    );
    w.unmount();
  });

  it('says so when the endpoint is not running', async () => {
    client.endpointInfo = async () => ({ url: null, token: mockEndpointToken(0) });
    const w = await mountIt();
    expect(w.text()).toContain('Not running.');
    w.unmount();
  });
});
