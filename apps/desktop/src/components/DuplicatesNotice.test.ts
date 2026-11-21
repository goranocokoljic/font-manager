// The notice after a scan that found copies (E56.15): its two lines, Review, dismiss, and
// that it never opens the module by itself.
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useDuplicateSummaryStore } from '../stores/duplicateSummary';
import DuplicatesNotice from './DuplicatesNotice.vue';

const push = vi.fn();
vi.mock('vue-router', () => ({ useRouter: () => ({ push }) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

let wrapper: VueWrapper | undefined;

async function setup() {
  const pinia = createPinia();
  setActivePinia(pinia);
  setClient(createMockClient());
  const store = useDuplicateSummaryStore();
  wrapper = mount(DuplicatesNotice, { global: { plugins: [pinia] }, attachTo: document.body });
  store.notice = { job: 7, folder: 'Downloads', added: 4, copies: 2 };
  await flushPromises();
  return { w: wrapper, store };
}

beforeEach(() => push.mockReset());
afterEach(() => {
  wrapper?.unmount();
  wrapper = undefined;
  setClient(undefined);
});

describe('the duplicates notice', () => {
  it('says what the scan added and how many are copies, and opens nothing by itself', async () => {
    const { w } = await setup();
    const notice = w.get('[data-testid="duplicates-notice"]');
    expect(notice.text()).toContain('Downloads added · 4 fonts');
    expect(notice.text()).toContain('2 files are copies of fonts you have');
    expect(push).not.toHaveBeenCalled();
    expect(notice.element.contains(document.activeElement)).toBe(false);
  });

  it('Review opens the module and closes the notice', async () => {
    const { w, store } = await setup();
    await w.get('button.action').trigger('click');
    expect(push).toHaveBeenCalledWith({ name: 'duplicates' });
    expect(store.notice).toBeNull();
  });

  it('dismiss and Escape close it without opening the module', async () => {
    const { w, store } = await setup();
    await w.get('button[aria-label="Dismiss"]').trigger('click');
    expect(store.notice).toBeNull();
    store.notice = { job: 8, folder: 'Downloads', added: 1, copies: 1 };
    await flushPromises();
    await w.get('[data-testid="duplicates-notice"]').trigger('keydown', { key: 'Escape' });
    expect(store.notice).toBeNull();
    expect(push).not.toHaveBeenCalled();
  });
});
