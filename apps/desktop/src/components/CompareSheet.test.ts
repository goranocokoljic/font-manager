import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FaceSummary } from '../api/types';
import { mockFaces } from '../api/mock';
import { FaceLoader, faceFamily, setFaceLoader } from '../preview/faceLoader';
import { PANGRAMS } from '../preview/sampleText';
import { useCompareStore } from '../stores/compare';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import CompareSheet from './CompareSheet.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const faces: FaceSummary[] = Array.from({ length: 4 }, (_, i) => ({
  ...mockFaces[i % mockFaces.length]!.face,
  id: 200 + i,
  style: `Style ${i}`,
}));
const name = (f: FaceSummary) => `${f.familyName} ${f.style}`;

let loader: FaceLoader;

async function mountWith(count: number, mode: 'sideBySide' | 'stacked' | 'overlay') {
  const pinia = createPinia();
  setActivePinia(pinia);
  localStorage.clear();
  const s = useCompareStore();
  faces.slice(0, count).forEach((f) => s.pin(f));
  s.setMode(mode);
  s.openSheet();
  const wrapper = mount(CompareSheet, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return { wrapper, s };
}

beforeEach(() => {
  // A loader whose fonts load at once, so the faces render in their own font.
  loader = new FaceLoader({
    fonts: { add: () => undefined, delete: () => true },
    create: () => ({ load: () => Promise.resolve() }),
    url: (id) => `fmfont://${id}`,
  });
  setFaceLoader(loader);
});
afterEach(() => {
  setFaceLoader(undefined);
  document.body.innerHTML = '';
});

describe('compare sheet', () => {
  it('overlays two pins on one baseline in two colors at 50% opacity', async () => {
    const { wrapper } = await mountWith(2, 'overlay');
    const texts = wrapper.findAll('text.overlay-text');
    expect(texts).toHaveLength(2);
    expect(texts.map((t) => t.attributes('data-face'))).toEqual(['200', '201']);
    // One shared baseline: the same y, and the baseline line sits on it.
    const ys = texts.map((t) => t.attributes('y'));
    expect(ys[0]).toBe(ys[1]);
    expect(wrapper.get('line.baseline').attributes('y1')).toBe(ys[0]);
    const styles = texts.map((t) => (t.element as SVGTextElement).style);
    expect(styles.map((st) => st.fillOpacity)).toEqual(['0.5', '0.5']);
    expect(styles[0]!.fill).not.toBe(styles[1]!.fill);
    // Live text in each face's own font, through the face loader.
    expect(styles[0]!.fontFamily).toContain(faceFamily(200));
    expect(styles[1]!.fontFamily).toContain(faceFamily(201));
    expect(texts[0]!.text()).toBe(PANGRAMS.en.text);
    // Two pins: nothing to choose.
    expect(wrapper.findAll('select')).toHaveLength(0);
  });

  it('with three or four pins lets the user choose the two overlaid faces', async () => {
    const { wrapper, s } = await mountWith(4, 'overlay');
    const selects = wrapper.findAll('select');
    expect(selects).toHaveLength(2);
    expect(selects[0]!.findAll('option').map((o) => o.text())).toEqual(faces.map(name));
    await selects[1]!.setValue('203');
    await flushPromises();
    const ids = () => wrapper.findAll('text.overlay-text').map((t) => t.attributes('data-face'));
    expect(ids()).toEqual(['200', '203']);
    expect(s.overlayPair.map((f) => f?.id)).toEqual([200, 203]);
    expect(wrapper.get('svg').attributes('aria-label')).toBe(
      `Overlay of ${name(faces[0]!)} and ${name(faces[3]!)}, 28 px`,
    );
  });

  it('shows every pin in the current sample text at one size, with a Remove button', async () => {
    const { wrapper, s } = await mountWith(3, 'stacked');
    usePreviewStore().update(ALL_FONTS_VIEW, { sample: 'alphabet', sizePx: 40 });
    await flushPromises();
    const samples = wrapper.findAll('.face .sample');
    expect(samples).toHaveLength(3);
    for (const p of samples) {
      expect(p.text()).toBe('ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz');
      expect((p.element as HTMLElement).style.fontSize).toBe('40px');
    }
    expect((samples[2]!.element as HTMLElement).style.fontFamily).toContain(faceFamily(202));
    expect(loader.inUse).toBe(3);
    await wrapper
      .get(`button[aria-label="Remove ${name(faces[1]!)} from Compare"]`)
      .trigger('click');
    expect(s.pins.map((p) => p.id)).toEqual([200, 202]);
    await flushPromises();
    expect(wrapper.findAll('.face')).toHaveLength(2);
    expect(loader.inUse).toBe(2);
    wrapper.unmount();
    expect(loader.inUse).toBe(0);
  });

  it('has a labelled radio group for the mode and closes on Escape', async () => {
    const { wrapper, s } = await mountWith(2, 'sideBySide');
    const group = wrapper.get('[role="radiogroup"]');
    expect(wrapper.get(`#${group.attributes('aria-labelledby')}`).text()).toBe('Mode');
    const radios = group.findAll('input[type="radio"]');
    expect(radios.map((r) => r.element.parentElement?.textContent?.trim())).toEqual([
      'Side by side',
      'Stacked',
      'Overlay',
    ]);
    expect(document.activeElement).toBe(radios[0]!.element);
    await radios[1]!.setValue(true);
    expect(s.mode).toBe('stacked');
    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' });
    expect(s.open).toBe(false);
    expect(s.pins).toHaveLength(2);
  });

  it('asks for a second pin when only one is pinned', async () => {
    const { wrapper } = await mountWith(1, 'sideBySide');
    expect(wrapper.get('[role="status"]').text()).toContain('Pin at least two fonts');
  });
});
