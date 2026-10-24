import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { setPaletteSheet } from '../preview/fontPalette';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { useSelectionStore } from '../stores/selection';
import InspectorPalettes from './InspectorPalettes.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorPalettes, { global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  localStorage.clear();
  setPaletteSheet(null);
  setClient(createMockClient({ colorFace: true }));
});
afterEach(() => {
  setClient(undefined);
  setPaletteSheet(undefined);
});

describe('inspector palette picker', () => {
  it('is not there for a face without palettes', async () => {
    const w = await mountWith(1);
    expect(w.find('[data-testid="palette-picker"]').exists()).toBe(false);
  });

  it('is a labeled radio group of the palettes with swatches and names', async () => {
    const w = await mountWith(8);
    const group = w.get('[role="radiogroup"]');
    const label = w.get(`#${group.attributes('aria-labelledby')}`);
    expect(label.text()).toBe('Color palette');
    const radios = group.findAll('input[type="radio"]');
    expect(radios).toHaveLength(2);
    // One name for the group: the arrow keys move between its radios.
    expect(new Set(radios.map((r) => r.attributes('name'))).size).toBe(1);
    const options = group.findAll('label');
    expect(options.map((o) => o.text())).toEqual(['Daylight', 'Palette 2']);
    const swatches = options[1]!.findAll('[data-testid="palette-swatch"]');
    expect(swatches.map((s) => (s.element as HTMLElement).style.backgroundColor)).toHaveLength(5);
    expect((radios[0]!.element as HTMLInputElement).checked).toBe(true);
  });

  it('chooses a palette for the family in this view', async () => {
    const w = await mountWith(8);
    await w.findAll('input[type="radio"]')[1]!.setValue(true);
    const palettes = usePalettesStore();
    expect(palettes.chosenFor(ALL_FONTS_VIEW, 'Color Play')).toBe(1);
    expect(palettes.chosenFor('other-view', 'Color Play')).toBe(0);
    expect(w.findAll('label')[1]!.classes()).toContain('chosen');
  });
});
