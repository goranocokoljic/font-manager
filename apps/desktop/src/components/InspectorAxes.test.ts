import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useAxesStore } from '../stores/axes';
import { useCollectionsStore } from '../stores/collections';
import { useSelectionStore } from '../stores/selection';
import { useVirtualStylesStore } from '../stores/virtualStyles';
import InspectorAxes from './InspectorAxes.vue';

const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorAxes, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const slider = (w: W) => w.get<HTMLInputElement>('li[data-tag="wght"] input[type="range"]');
const field = (w: W) => w.get<HTMLInputElement>('li[data-tag="wght"] input[type="text"]');

beforeEach(() => setClient(createMockClient()));
afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('inspector axes tab', () => {
  it('says so when nothing is selected', async () => {
    expect((await mountWith(null)).text()).toContain('Select a face');
  });

  it('says a static face has no variation axes', async () => {
    const w = await mountWith(6);
    expect(w.text()).toContain('This font has no variation axes');
    expect(w.find('input[type="range"]').exists()).toBe(false);
  });

  it('shows one labelled slider and number field per axis, min to max, default marked', async () => {
    const w = await mountWith(1);
    expect(w.findAll('input[type="range"]')).toHaveLength(1);
    const s = slider(w).element;
    expect([s.min, s.max, s.step, s.value]).toEqual(['100', '900', '1', '400']);
    const label = w.get(`label[for="${s.id}"]`).text();
    expect(label).toContain('Weight');
    expect(label).toContain('wght');
    expect(field(w).attributes('aria-label')).toBe('Weight (wght) value');
    const range = w.get('li[data-tag="wght"] .range').text();
    expect(range).toContain('100 to 900, default 400');
    expect(w.get(`datalist#${s.getAttribute('list')} option`).attributes('value')).toBe('400');
  });

  it('moves the axis with the slider and with a typed, clamped value', async () => {
    const w = await mountWith(1);
    await slider(w).setValue('650');
    expect(
      useAxesStore().value(1, { tag: 'wght', name: '', min: 100, default: 400, max: 900 }),
    ).toBe(650);
    expect(field(w).element.value).toBe('650');
    await field(w).setValue('2000');
    expect(field(w).element.value).toBe('900');
    expect(slider(w).element.value).toBe('900');
  });

  it('rejects a non-number without changing the preview', async () => {
    const w = await mountWith(1);
    await field(w).setValue('heavy');
    expect(useAxesStore().isChanged(1)).toBe(false);
    expect(field(w).attributes('aria-invalid')).toBe('true');
    expect(w.get('[role="alert"]').text()).toContain('type a number');
    await field(w).setValue('500');
    expect(field(w).attributes('aria-invalid')).toBe('false');
  });

  it('lists named instances as presets; choosing one sets the sliders; Reset goes back', async () => {
    const w = await mountWith(1);
    const presets = w.findAll('button.preset');
    expect(presets.map((p) => p.text())).toEqual(['Thin', 'Regular', 'Bold', 'Black']);
    expect(w.get('[data-instance="Regular"]').attributes('aria-pressed')).toBe('true');
    await w.get('[data-instance="Bold"]').trigger('click');
    expect(slider(w).element.value).toBe('700');
    expect(w.get('[data-instance="Bold"]').attributes('aria-pressed')).toBe('true');
    expect(w.get('[data-instance="Regular"]').attributes('aria-pressed')).toBe('false');
    await w.get('button.reset').trigger('click');
    expect(slider(w).element.value).toBe('400');
    expect(w.get('button.reset').attributes('disabled')).toBeDefined();
  });

  it('pins the current coordinates as a named virtual style into the chosen collection', async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const made = await useClient().createCollection({ name: 'Brand' });
    await useClient().createCollection({ name: 'Smart', rules: { variable: true } });
    await useCollectionsStore().refresh();
    useCollectionsStore().show(made.id);
    await useSelectionStore().select(1);
    const w = mount(InspectorAxes, { global: { plugins: [pinia] } });
    await flushPromises();
    // Only manual collections and sets can hold a virtual style.
    const options = w.findAll('select option').map((o) => o.text());
    expect(options).toContain('Brand');
    expect(options).not.toContain('Smart');
    expect((w.get('select').element as HTMLSelectElement).value).toBe(`c${made.id}`);

    await slider(w).setValue('550');
    await w.get('input.field').setValue('Medium Plus');
    await w.get('button.pin-button').trigger('click');
    await flushPromises();
    const pinned = await useClient().listVirtualStyles({ collection: made.id });
    expect(pinned).toEqual([
      expect.objectContaining({
        face: 1,
        name: 'Medium Plus',
        coordinates: [{ tag: 'wght', value: 550 }],
      }),
    ]);
    expect(useVirtualStylesStore().styles.map((p) => p.style.name)).toEqual(['Medium Plus']);

    // The same name again is a conflict the API reports; the tab shows it.
    await w.get('button.pin-button').trigger('click');
    await flushPromises();
    expect(w.get('.pin [role="alert"]').text()).toContain('already has a style named');
  });

  it('starts the pin name at the named instance the axes are at', async () => {
    setClient(createMockClient({ sets: [{ id: 9, name: 'Poster', faces: [1] }] }));
    const w = await mountWith(1);
    const name = () => (w.get('input.field').element as HTMLInputElement).value;
    expect(name()).toBe('Regular');
    expect(w.get('select option').text()).toBe('Set: Poster');
    await w.get('[data-instance="Black"]').trigger('click');
    expect(name()).toBe('Black');
  });

  it('asks for a collection or set when there is none to pin into', async () => {
    const w = await mountWith(1);
    expect(w.text()).toContain('Make a manual collection or a project set');
    expect(w.find('button.pin-button').exists()).toBe(false);
  });

  it('copies font-variation-settings from fm-core for the moved axes, normal when none', async () => {
    copyText.mockReset().mockResolvedValue(undefined);
    const cssValues = vi.spyOn(useClient(), 'cssValues');
    const w = await mountWith(1);
    const copy = w.get('button[aria-label="Copy CSS for the axes"]');
    await copy.trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith('font-variation-settings: normal;');
    expect(w.get('[role="status"]').text()).toBe('Copied');
    await slider(w).setValue('650');
    await copy.trigger('click');
    await flushPromises();
    expect(cssValues).toHaveBeenLastCalledWith({
      face: 1,
      variations: [{ tag: 'wght', value: 650 }],
      features: [],
    });
    expect(copyText).toHaveBeenLastCalledWith('font-variation-settings: "wght" 650;');
  });

  it('says "Could not copy" when the clipboard refuses', async () => {
    copyText.mockReset().mockRejectedValue(new Error('refused'));
    const w = await mountWith(1);
    await w.get('button[aria-label="Copy CSS for the axes"]').trigger('click');
    await flushPromises();
    expect(w.get('[role="status"]').text()).toBe('Could not copy');
  });
});
