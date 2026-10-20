import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { compareLayout } from '../glyphs/compare';
import { FaceLoader, faceFamily, setFaceLoader } from '../preview/faceLoader';
import { useGlyphCompareStore } from '../stores/glyphCompare';
import GlyphCompare from './GlyphCompare.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const ZHE = 0x416;

async function mountOn(cp: number) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const s = useGlyphCompareStore();
  await s.start(1, cp);
  const wrapper = mount(GlyphCompare, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return { wrapper, s };
}

/** Types `name` into the picker and chooses the first result with Enter. */
async function pickByKeyboard(w: Awaited<ReturnType<typeof mountOn>>['wrapper'], name: string) {
  const input = w.get('input[role="combobox"]');
  await input.setValue(name);
  await flushPromises();
  await input.trigger('keydown', { key: 'Enter' });
  await flushPromises();
}

beforeEach(() => {
  setClient(createMockClient());
  // A loader whose fonts load at once, so the glyphs are drawn.
  setFaceLoader(
    new FaceLoader({
      fonts: { add: () => undefined, delete: () => true },
      create: () => ({ load: () => Promise.resolve() }),
      url: (id) => `fmfont://${id}`,
    }),
  );
});
afterEach(() => {
  setClient(undefined);
  setFaceLoader(undefined);
  document.body.innerHTML = '';
});

describe('glyph compare overlay', () => {
  it('is a labelled dialog that focuses the font picker and asks for a font', async () => {
    const { wrapper } = await mountOn(ZHE);
    const dialog = wrapper.get('[role="dialog"]');
    expect(dialog.attributes('aria-modal')).toBe('true');
    expect(wrapper.get('#fm-compare-title').text()).toBe('Compare U+0416');
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Compare with font');
    expect(wrapper.get('[role="status"]').text()).toContain('Pick a font to compare U+0416');
    expect(wrapper.findAll('[role="option"]').length).toBeGreaterThan(1);
  });

  it('draws both glyphs at one size on one baseline, at 50% opacity, with a legend', async () => {
    const { wrapper } = await mountOn(ZHE);
    await pickByKeyboard(wrapper, 'JetBrains Mono');
    const texts = wrapper.findAll('text.glyph');
    expect(texts.map((t) => t.text())).toEqual(['Ж', 'Ж']);
    expect(texts.map((t) => t.attributes('data-face'))).toEqual(['1', '6']);
    expect(new Set(texts.map((t) => t.attributes('y'))).size).toBe(1);
    expect(new Set(texts.map((t) => t.attributes('font-size'))).size).toBe(1);
    expect(texts[0]!.attributes('style')).toContain(faceFamily(1));
    expect(texts[1]!.attributes('style')).toContain(faceFamily(6));
    expect(texts[0]!.classes()).toContain('font-0');
    expect(texts[1]!.classes()).toContain('font-1');
    const legend = wrapper.get('[aria-label="Legend"]').text();
    expect(legend).toContain('Inter Regular');
    expect(legend).toContain('JetBrains Mono Regular');
    // The lines come from each face's metrics: Inter 520 / 720, JetBrains Mono 550 / 730.
    expect(legend).toContain('x-height 520, cap height 720');
    expect(legend).toContain('x-height 550, cap height 730');
    const want = compareLayout(200, [mockFaces[0]!.metrics, mockFaces[1]!.metrics]);
    const y = (line: string) => Number(wrapper.get(`[data-line="${line}"]`).attributes('y1'));
    expect(y('baseline')).toBeCloseTo(want.baseline);
    expect(y('xHeight-0')).toBeCloseTo(want.baseline - 0.52 * 200);
    expect(y('capHeight-1')).toBeCloseTo(want.baseline - 0.73 * 200);
  });

  it('says so when the second font lacks the character, and does not draw it', async () => {
    const { wrapper } = await mountOn(ZHE);
    await pickByKeyboard(wrapper, 'Legacy Grotesk');
    expect(wrapper.get('[role="status"]').text()).toBe(
      'Legacy Grotesk Regular has no glyph for U+0416.',
    );
    expect(wrapper.findAll('text.glyph').map((t) => t.attributes('data-face'))).toEqual(['1']);
    expect(wrapper.find('[data-line="capHeight-1"]').exists()).toBe(false);
  });

  it('changes the size and swaps the font without closing', async () => {
    const { wrapper, s } = await mountOn(0x41);
    await pickByKeyboard(wrapper, 'Legacy Grotesk');
    await wrapper.get('input[aria-label="Size"]').setValue('320');
    expect(s.size).toBe(320);
    expect(wrapper.get('text.glyph').attributes('font-size')).toBe('320');
    // Legacy Grotesk declares no x-height: no line for it.
    expect(wrapper.find('[data-line="xHeight-1"]').exists()).toBe(false);
    expect(wrapper.find('[data-line="capHeight-1"]').exists()).toBe(true);
    await pickByKeyboard(wrapper, 'JetBrains Mono');
    expect(s.open).toBe(true);
    expect(wrapper.findAll('text.glyph').map((t) => t.attributes('data-face'))).toEqual(['1', '6']);
    expect(wrapper.get('svg').attributes('aria-label')).toBe(
      'U+0041 in Inter Regular and JetBrains Mono Regular, 320 px',
    );
  });

  it('closes on Escape', async () => {
    const { wrapper, s } = await mountOn(ZHE);
    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' });
    expect(s.open).toBe(false);
  });
});
