import { flushPromises, mount } from '@vue/test-utils';
import { createPinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.vue';
import { setClient, type FmClient } from '../api/client';
import {
  createMockClient,
  mockDuplicates,
  mockExtraProblems,
  mockSidecarConflicts,
  type MockOptions,
} from '../api/mock';
import type { Problem } from '../api/types';
import { createAppRouter } from '../router';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
const copied: string[] = [];
vi.mock('../api/clipboard', () => ({
  copyText: async (text: string) => {
    copied.push(text);
  },
}));

afterEach(() => {
  setClient(undefined);
  copied.length = 0;
  document.body.innerHTML = '';
});

function client(options: MockOptions = {}, more: Problem[] = []): FmClient {
  const c = createMockClient({
    setup: 'none',
    extraProblems: [...mockExtraProblems, ...more],
    duplicates: mockDuplicates,
    sidecarConflicts: mockSidecarConflicts,
    ...options,
  });
  setClient(c);
  return c;
}

async function mountAt(path: string) {
  const router = createAppRouter();
  await router.push(path);
  const wrapper = mount(App, {
    global: { plugins: [createPinia(), router] },
    attachTo: document.body,
  });
  await router.isReady();
  await flushPromises();
  return { wrapper, router };
}

const broken = (n: number): Problem[] =>
  Array.from({ length: n }, (_, i) => ({
    kind: 'broken',
    face: null,
    path: `/Users/demo/Fonts/many/Broken-${i}.otf`,
    other: null,
    message: `Broken file number ${i}.`,
  }));

describe('Problems view (board 16)', () => {
  it('lists All and the groups with their counts; a group with none is muted and opens', async () => {
    client();
    const { wrapper, router } = await mountAt('/problems');
    const tabs = wrapper.findAll('[role="tab"]');
    expect(tabs.map((t) => t.find('.label').text())).toEqual([
      'All',
      'Broken files',
      'Legacy formats',
      'Sync conflicts',
      'Activation conflicts',
      'Folders and index',
    ]);
    expect(tabs.map((t) => t.find('.n').exists() && t.find('.n').text())).toEqual([
      '4',
      '1',
      '1',
      '1',
      '1',
      false,
    ]);
    expect(tabs[0]!.attributes('aria-selected')).toBe('true');
    expect(tabs[5]!.classes()).toContain('none');
    expect(tabs[1]!.classes()).not.toContain('none');
    expect(wrapper.get('[data-testid="problems-heading"]').text()).toBe('4 things need a look');

    await tabs[5]!.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.query).toEqual({ kind: 'library' });
    expect(wrapper.get('[data-testid="problems-heading"]').text()).toBe('No problems');
    expect(wrapper.text()).toContain('Nothing of this kind right now.');
    expect(document.activeElement?.id).toBe('problems-heading');
    wrapper.unmount();
  });

  it('groups the rows under their headings, each with its name, kind, sentence and path', async () => {
    client();
    const { wrapper } = await mountAt('/problems');
    const groups = wrapper.findAll('[data-testid="problems-group"]');
    expect(groups.map((g) => g.get('h3').text())).toEqual([
      'Broken files · 1',
      'Legacy formats · 1',
      'Sync conflicts · 1',
      'Activation conflicts · 1',
    ]);
    expect(groups[0]!.get('.group-line').text()).toMatch(/^Files that could not be read\./);
    const row = groups[0]!.get('[data-testid="problem-row"]');
    expect(row.get('.name').text()).toBe('Quarto-Bold.otf');
    expect(row.get('.kind').text()).toBe('damaged file');
    expect(row.get('.message').text()).toBe(
      "The file is truncated: the 'glyf' table ends before its declared length.",
    );
    expect(row.get('.path').text()).toBe('/Users/demo/Fonts/damaged/Quarto-Bold.otf');
    expect(row.find('.tile').exists()).toBe(true);
    wrapper.unmount();
  });

  it('offers one action per row: Copy path, Decide…, Open Sources', async () => {
    const gone: Problem = {
      kind: 'missingSource',
      face: null,
      path: '/Volumes/Archive',
      other: null,
      message: 'The source folder cannot be found.',
    };
    client({}, [gone]);
    const { wrapper, router } = await mountAt('/problems');
    const action = (group: string) => wrapper.get(`[data-group="${group}"] [data-row-action]`);

    expect(action('broken').text()).toBe('Copy path');
    expect(action('broken').attributes('aria-label')).toBe('Copy path: Quarto-Bold.otf');
    await action('broken').trigger('click');
    await flushPromises();
    expect(copied).toEqual(['/Users/demo/Fonts/damaged/Quarto-Bold.otf']);
    expect(document.body.textContent).toContain('Path copied.');

    expect(action('library').text()).toBe('Open Sources');
    expect(wrapper.find('[data-group="library"]').text()).toContain('folder not found');
    await action('library').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('settings');
    expect(router.currentRoute.value.query).toEqual({ section: 'sources' });

    await router.push('/problems');
    await flushPromises();
    expect(action('activation').text()).toBe('Decide…');
    await action('activation').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('conflicts');
    expect(wrapper.text()).not.toContain('Show in folder');
    wrapper.unmount();
  });

  it('shows two rows of a group in All with "N more", which opens the group', async () => {
    client({}, broken(3));
    const { wrapper, router } = await mountAt('/problems');
    const group = wrapper.get('[data-group="broken"]');
    expect(group.findAll('[data-testid="problem-row"]')).toHaveLength(2);
    const more = group.get('[data-testid="problems-more"]');
    expect(more.text()).toBe('2 more');
    await more.trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.query).toEqual({ kind: 'broken' });
    expect(
      wrapper.get('[data-group="broken"]').findAll('[data-testid="problem-row"]'),
    ).toHaveLength(4);
    wrapper.unmount();
  });

  it('windows a long group: only the rows in view exist', async () => {
    client({}, broken(400));
    const { wrapper } = await mountAt('/problems?kind=broken');
    const win = wrapper.get('[data-testid="problems-window"]');
    const rows = win.findAll('[data-testid="problem-row"]');
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(40);
    expect(rows[0]!.attributes('aria-setsize')).toBe('401');
    expect(wrapper.get('[data-testid="problems-heading"]').text()).toBe('401 things need a look');

    // End on a row's action moves to the last row, which the window then holds.
    const first = rows[0]!.get('[data-row-action]');
    (first.element as HTMLElement).focus();
    await first.trigger('keydown', { key: 'End' });
    await flushPromises();
    const focused = document.activeElement as HTMLElement;
    expect(focused.closest('[data-row-index]')?.getAttribute('data-row-index')).toBe('400');
    wrapper.unmount();
  });

  it('lists a short group whole, without a window', async () => {
    client({}, broken(3));
    const { wrapper } = await mountAt('/problems?kind=broken');
    expect(wrapper.find('[data-testid="problems-window"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('resolves a sync conflict and moves the focus to the group heading when it was the last', async () => {
    client();
    const { wrapper } = await mountAt('/problems?kind=sidecar');
    const conflict = wrapper.get('[data-testid="sidecar-conflict"]');
    expect(conflict.attributes('aria-label')).toBe('Inter: Rating');
    expect(conflict.text()).toContain('This computer');
    expect(conflict.text()).toContain('Other computer');
    const keep = conflict.get('button[aria-label^="Keep mine: Rating of Inter"]');
    await keep.trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-testid="sidecar-conflict"]').exists()).toBe(false);
    expect(document.activeElement?.id).toBe('problems-group-sidecar');
    wrapper.unmount();
  });

  it('resolves a sync conflict and moves the focus to the next row', async () => {
    client({
      sidecarConflicts: [
        ...mockSidecarConflicts,
        { ...mockSidecarConflicts[0]!, id: 2, field: 'notes', mine: 'Bold', theirs: 'Light' },
      ],
    });
    const { wrapper } = await mountAt('/problems?kind=sidecar');
    expect(wrapper.findAll('[data-testid="sidecar-conflict"]')).toHaveLength(2);
    await wrapper.get('button[aria-label^="Use theirs: Rating of Inter"]').trigger('click');
    await flushPromises();
    const left = wrapper.findAll('[data-testid="sidecar-conflict"]');
    expect(left).toHaveLength(1);
    expect(document.activeElement?.getAttribute('aria-label')).toBe(
      'Keep mine: Notes of Inter on this computer, Bold',
    );
    wrapper.unmount();
  });

  it('in All, moves the focus to the next group when the last conflict of its group goes', async () => {
    client();
    const { wrapper } = await mountAt('/problems');
    await wrapper.get('button[aria-label^="Keep mine: Rating of Inter"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-group="sidecar"]').exists()).toBe(false);
    // Sync conflicts stood before Activation conflicts, which took its place.
    expect(document.activeElement?.getAttribute('aria-label')).toBe(
      'Decide: JetBrainsMono-Regular.ttf',
    );
    wrapper.unmount();
  });

  it('is the neutral "All good." panel without problems, the navigation still there', async () => {
    client({ problems: false, sidecarConflicts: [] });
    const { wrapper } = await mountAt('/problems');
    expect(wrapper.get('[data-testid="problems-heading"]').text()).toBe('No problems');
    expect(wrapper.get('[data-testid="problems-empty"]').text()).toContain('All good.');
    expect(wrapper.findAll('[role="tab"]')).toHaveLength(6);
    expect(wrapper.find('[data-testid="problems-group"]').exists()).toBe(false);
    // No font has copies in this library: the duplicates line is not shown.
    expect(wrapper.find('[data-testid="problems-duplicates"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
