import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import McpConfirmToast from '../components/McpConfirmToast.vue';
import { setLocale } from '../i18n';
import { useMcpConfirmStore } from './mcpConfirm';
import { useToastStore } from './toast';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setLocale('en');
  setActivePinia(createPinia());
  client = createMockClient({ mcpConfirm: true });
  setClient(client);
});

afterEach(async () => {
  await useMcpConfirmStore().stop();
  setClient(undefined);
});

/** Makes the mock AI assistant ask to deactivate 3 fonts. */
async function ask(): Promise<void> {
  await client.setMcpGrant({ tool: 'deactivate_fonts', granted: true });
  await vi.waitFor(() => expect(useMcpConfirmStore().current).not.toBeNull());
}

describe('AI assistant confirmations', () => {
  it('shows the request with its count and sends Allow', async () => {
    const w = mount(McpConfirmToast, { attachTo: document.body });
    await flushPromises();
    await ask();
    await flushPromises();
    const toast = w.get('[data-testid="mcp-confirm"]');
    expect(toast.attributes('aria-label')).toBe('Request from an AI assistant');
    expect(toast.text()).toContain('An AI assistant wants to deactivate 3 fonts.');
    expect(toast.text()).toContain('No answer in 60 seconds counts as Deny.');
    const answer = vi.spyOn(client, 'answerMcpConfirmation');
    await toast.findAll('button')[0].trigger('click');
    await flushPromises();
    expect(answer).toHaveBeenCalledWith({ id: 1, allow: true });
    expect(w.find('[data-testid="mcp-confirm"]').exists()).toBe(false);
    expect(useToastStore().toast?.message).toBe("Allowed. The AI assistant's request runs now.");
    w.unmount();
  });

  it('sends Deny and says nothing changed', async () => {
    const w = mount(McpConfirmToast, { attachTo: document.body });
    await flushPromises();
    await ask();
    await flushPromises();
    const answer = vi.spyOn(client, 'answerMcpConfirmation');
    const deny = w.findAll('button').find((b) => b.text() === 'Deny');
    await deny!.trigger('click');
    await flushPromises();
    expect(answer).toHaveBeenCalledWith({ id: 1, allow: false });
    expect(useToastStore().toast?.message).toBe('Denied. Nothing was changed.');
    w.unmount();
  });

  it('takes the toast away when fm-core closes the request (timeout)', async () => {
    const store = useMcpConfirmStore();
    store.onEvent({
      type: 'mcpConfirmationRequested',
      id: 7,
      tool: 'deactivate_fonts',
      faces: null,
      timeoutSecs: 60,
    });
    expect(store.current?.faces).toBeNull();
    store.onEvent({ type: 'mcpConfirmationClosed', id: 7, allowed: false });
    expect(store.current).toBeNull();
  });

  it('a late answer counts as Deny', async () => {
    const store = useMcpConfirmStore();
    store.onEvent({
      type: 'mcpConfirmationRequested',
      id: 99,
      tool: 'deactivate_fonts',
      faces: 2,
      timeoutSecs: 60,
    });
    await store.answer(99, true);
    expect(store.current).toBeNull();
    expect(useToastStore().toast?.message).toBe('Denied. Nothing was changed.');
  });
});
