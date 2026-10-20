import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useFeaturesStore } from '../stores/features';
import { useSelectionStore } from '../stores/selection';
import InspectorFeatures from './InspectorFeatures.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorFeatures, { global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}
const switchFor = (w: Awaited<ReturnType<typeof mountWith>>, tag: string) =>
  w.get(`[role="switch"][data-tag="${tag}"]`);

beforeEach(() => setClient(createMockClient()));
afterEach(() => setClient(undefined));

describe('inspector features tab', () => {
  it('says so when nothing is selected', async () => {
    expect((await mountWith(null)).text()).toContain('Select a face');
  });

  it('lists every switchable feature with a switch at the browser default', async () => {
    const w = await mountWith(1);
    const switches = w.findAll('[role="switch"]');
    expect(switches.map((s) => s.attributes('aria-label'))).toEqual([
      'Kerning (kern)',
      'Standard ligatures (liga)',
      'Oldstyle figures (onum)',
      'Small capitals (smcp)',
      'Stylistic set 1 (ss01)',
    ]);
    expect(switches.map((s) => s.attributes('aria-checked'))).toEqual([
      'true',
      'true',
      'false',
      'false',
      'false',
    ]);
  });

  it('lists shaper features as applied automatically, without a switch', async () => {
    const w = await mountWith(1);
    expect(w.find('[role="switch"][data-tag="ccmp"]').exists()).toBe(false);
    const row = w.get('li[data-tag="ccmp"]');
    expect(row.text()).toContain('Glyph composition and decomposition');
    expect(row.text()).toContain('applied automatically');
  });

  it('switches a feature for the face and Reset restores the defaults', async () => {
    const w = await mountWith(1);
    const reset = w.get('button.reset');
    expect(reset.attributes('disabled')).toBeDefined();
    await switchFor(w, 'smcp').trigger('click');
    await switchFor(w, 'kern').trigger('click');
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('true');
    expect(switchFor(w, 'kern').attributes('aria-checked')).toBe('false');
    expect(useFeaturesStore().settings(1)).toBe('"kern" 0, "smcp" 1');
    expect(reset.attributes('disabled')).toBeUndefined();
    await reset.trigger('click');
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('false');
    expect(switchFor(w, 'kern').attributes('aria-checked')).toBe('true');
    expect(useFeaturesStore().settings(1)).toBeUndefined();
  });

  it('keeps the state per face', async () => {
    const w = await mountWith(1);
    await switchFor(w, 'smcp').trigger('click');
    await useSelectionStore().select(6);
    await flushPromises();
    expect(w.findAll('[role="switch"]').map((s) => s.attributes('aria-checked'))).toEqual([
      'true',
      'true',
    ]);
    await useSelectionStore().select(1);
    await flushPromises();
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('true');
  });
});
