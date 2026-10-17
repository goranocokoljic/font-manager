import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { useSelectionStore } from '../stores/selection';
import HealthBadge from './HealthBadge.vue';
import InspectorInfo from './InspectorInfo.vue';

const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorInfo, { global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  setClient(createMockClient());
  copyText.mockResolvedValue(undefined);
});

afterEach(() => {
  setClient(undefined);
  copyText.mockReset();
  vi.useRealTimers();
});

describe('inspector info tab', () => {
  it('says so when nothing is selected', async () => {
    expect((await mountWith(null)).text()).toContain('Nothing selected');
  });

  it('shows every field of the face detail', async () => {
    const text = (await mountWith(1)).text();
    const d = mockFaces[0];
    for (const value of [
      d.face.familyName,
      d.face.style,
      d.face.postscriptName,
      d.legacyFamily!,
      d.version!,
      d.foundry!,
      d.designer!,
      d.copyright!,
      d.path,
      'TrueType (TTF)',
      '121,000 bytes',
      d.hash,
      'Weight',
      '0°',
      'wght Weight: 100–900 (default 400)',
      'Thin, Regular, Bold, Black',
      '1,000',
      '-250',
      '520',
      '720',
      '187',
      'latn, cyrl',
      'kern, liga, smcp, onum, ss01',
      d.license.description!,
      d.license.url!,
      'SIL Open Font License',
      'Installable',
    ]) {
      expect(text).toContain(value);
    }
    for (const label of ['Vendor ID', 'Tags', 'Favorite', 'Rating', 'Notes', 'Face in file']) {
      expect(text).toContain(label);
    }
  });

  it('copies names and the path on click', async () => {
    const wrapper = await mountWith(1);
    for (const value of ['Inter', 'Inter-Regular', mockFaces[0].path]) {
      await wrapper.get(`button[aria-label$=": ${value}"]`).trigger('click');
      await flushPromises();
      expect(copyText).toHaveBeenLastCalledWith(value);
    }
    expect(wrapper.text()).toContain('Copied');
  });

  it('says when the clipboard refuses', async () => {
    copyText.mockRejectedValue(new Error('denied'));
    const wrapper = await mountWith(1);
    await wrapper.get('button[aria-label^="Copy path"]').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('Could not copy');
  });

  it('activates and deactivates the face from the button', async () => {
    const wrapper = await mountWith(1);
    const button = () => wrapper.get('button.activate');
    expect(button().text()).toBe('Activate');
    expect(wrapper.get('[data-testid="activation-state"]').text()).toBe('Not active');
    await button().trigger('click');
    await flushPromises();
    expect(button().text()).toBe('Deactivate');
    expect(button().attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="activation-state"]').text()).toBe('Active for this session');
    await button().trigger('click');
    await flushPromises();
    expect(button().text()).toBe('Activate');
  });

  it('shows why a clashing face was not activated', async () => {
    const wrapper = await mountWith(6);
    await wrapper.get('button.activate').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain(
      '/System/Library/Fonts/JetBrainsMono-Regular.ttf',
    );
  });

  it('disables activation for an unsupported face and shows its health', async () => {
    const wrapper = await mountWith(7);
    expect(wrapper.get('button.activate').attributes('disabled')).toBeDefined();
    const badge = wrapper.get('[data-testid="health-badge"]');
    expect(badge.text()).toContain('Unsupported');
    expect(badge.text()).toContain('Type 1 fonts cannot be activated');
  });

  it('shows the API error when the face cannot be loaded', async () => {
    const wrapper = await mountWith(999);
    expect(wrapper.get('[role="alert"]').text()).toBe('Face 999 was not found.');
  });
});

describe('health badge', () => {
  it('labels each health kind', () => {
    expect(mount(HealthBadge, { props: { health: { kind: 'ok' } } }).text()).toBe('OK');
    const broken = mount(HealthBadge, {
      props: { health: { kind: 'broken', reason: 'The glyf table is truncated.' } },
    });
    expect(broken.text()).toContain('Broken');
    expect(broken.text()).toContain('The glyf table is truncated.');
    expect(broken.get('.badge').classes()).toContain('broken');
  });
});
