import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FaceSummary } from '../api/types';
import { mockFaces, mockLanguages } from '../api/mock';
import { FaceLoader, faceFamily, setFaceLoader } from '../preview/faceLoader';
import { ALPHABET } from '../preview/sampleText';
import { useSearchStore } from '../stores/search';
import { useCompareStore } from '../stores/compare';
import { COMPARE_SHEET_STORAGE_KEY, useCompareSheetStore } from '../stores/compareSheet';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import CompareSheet from './CompareSheet.vue';
import { pickOption, selectOptions } from './ui/testing';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const faces: FaceSummary[] = Array.from({ length: 4 }, (_, i) => ({
  ...mockFaces[i % mockFaces.length]!.face,
  id: 200 + i,
  style: `Style ${i}`,
}));
const name = (f: FaceSummary) => `${f.familyName} ${f.style}`;

let loader: FaceLoader;

/** The mock API's pangram for `code`. */
const pangramOf = (code: string) => mockLanguages.find((l) => l.code === code)!.pangram!;

async function mountWith(count: number, mode: 'sideBySide' | 'stacked' | 'overlay') {
  const pinia = createPinia();
  setActivePinia(pinia);
  localStorage.clear();
  const s = useCompareStore();
  faces.slice(0, count).forEach((f) => s.pin(f));
  s.setMode(mode);
  s.openSheet();
  await useSearchStore().loadLanguages();
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
    expect(texts[0]!.text()).toBe(pangramOf('en'));
    // Two pins: nothing to choose.
    expect(wrapper.findAll('[role="combobox"]')).toHaveLength(0);
    // The legend names each face's family and style, by its color.
    const legend = wrapper.findAll('.legend li');
    expect(legend.map((li) => li.attributes('data-face'))).toEqual(['200', '201']);
    expect(legend.map((li) => li.text())).toEqual(faces.slice(0, 2).map(name));
    expect(
      legend.map((li) => [li.get('.legend-family').text(), li.get('.legend-style').text()]),
    ).toEqual(faces.slice(0, 2).map((f) => [f.familyName, f.style]));
    expect(legend.map((li) => li.get('.swatch').classes())).toEqual([
      ['swatch', 'color-0'],
      ['swatch', 'color-1'],
    ]);
  });

  it('with three or four pins lets the user choose the two overlaid faces', async () => {
    const { wrapper, s } = await mountWith(4, 'overlay');
    const selects = wrapper.findAll('[role="combobox"]');
    expect(selects).toHaveLength(2);
    expect((await selectOptions(selects[0]!)).map(([, label]) => label)).toEqual(faces.map(name));
    await pickOption(selects[1]!, '203');
    await flushPromises();
    const ids = () => wrapper.findAll('text.overlay-text').map((t) => t.attributes('data-face'));
    expect(ids()).toEqual(['200', '203']);
    expect(s.overlayPair.map((f) => f?.id)).toEqual([200, 203]);
    expect(wrapper.get('.stage svg').attributes('aria-label')).toBe(
      `Overlay of ${name(faces[0]!)} and ${name(faces[3]!)}, 64 px`,
    );
  });

  it('shows every pin in the current sample text at one size, with a remove icon', async () => {
    const { wrapper, s } = await mountWith(3, 'stacked');
    usePreviewStore().update(ALL_FONTS_VIEW, { sample: 'alphabet', sizePx: 40 });
    await flushPromises();
    const samples = wrapper.findAll('.face .sample');
    expect(samples).toHaveLength(3);
    for (const p of samples) {
      expect(p.text()).toBe(ALPHABET);
      // Auto follows the sheet's height (60 % by default), not the list's size.
      expect((p.element as HTMLElement).style.fontSize).toBe('64px');
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

  it('each face has the filled compare icon as Remove, no text button; the focus goes to the next face’s icon (E48.4)', async () => {
    const { wrapper, s } = await mountWith(3, 'stacked');
    const icons = () => wrapper.findAll('[data-testid="face-remove"]');
    expect(icons()).toHaveLength(3);
    for (const b of icons()) {
      expect(b.attributes('data-in-set')).toBe('true');
      expect(b.classes()).toContain('in-set');
      expect(b.text()).toBe('');
      expect(b.findAll('rect[fill="currentColor"]')).toHaveLength(2);
    }
    expect(wrapper.findAll('.face button').filter((b) => b.text() === 'Remove')).toHaveLength(0);
    expect(icons().map((b) => b.attributes('aria-label'))).toEqual(
      faces.slice(0, 3).map((f) => `Remove ${name(f)} from Compare`),
    );
    await icons()[1]!.trigger('click');
    await flushPromises();
    expect(s.pins.map((p) => p.id)).toEqual([200, 202]);
    expect(document.activeElement).toBe(icons()[1]!.element);
    expect(document.activeElement?.getAttribute('data-remove')).toBe('202');
    // The last face's removal goes to the face before it.
    await icons()[1]!.trigger('click');
    await flushPromises();
    expect(document.activeElement?.getAttribute('data-remove')).toBe('200');
  });

  it('has a labelled radio group for the mode and closes on Escape', async () => {
    const { wrapper, s } = await mountWith(2, 'sideBySide');
    const group = wrapper.get('[role="radiogroup"]');
    expect(wrapper.get(`#${group.attributes('aria-labelledby')}`).text()).toBe('Mode');
    const radios = group.findAll('input[type="radio"]');
    expect(radios.map((r) => r.element.parentElement?.textContent?.trim())).toEqual([
      'Stacked',
      'Grid',
      'Overlay',
    ]);
    expect(radios.map((r) => (r.element as HTMLInputElement).value)).toEqual([
      'stacked',
      'sideBySide',
      'overlay',
    ]);
    // Each segment has its icon before the label.
    for (const r of radios)
      expect(r.element.parentElement?.querySelector('svg.ui-icon')).not.toBeNull();
    // The open mode (Grid) has the focus.
    expect(document.activeElement).toBe(radios[1]!.element);
    expect(wrapper.get('ul.faces').attributes('aria-label')).toBe('Grid');
    await radios[0]!.setValue(true);
    expect(s.mode).toBe('stacked');
    expect(wrapper.get('ul.faces').attributes('aria-label')).toBe('Stacked');
    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' });
    expect(s.open).toBe(false);
    expect(s.pins).toHaveLength(2);
  });

  it('keeps keys away from the page behind when the focus fell out of the sheet', async () => {
    const { wrapper, s } = await mountWith(2, 'sideBySide');
    const behind = vi.fn();
    window.addEventListener('keydown', behind);
    try {
      // The focus is on the page (e.g. after a click on the sample text).
      (document.activeElement as HTMLElement).blur();
      expect(document.activeElement).toBe(document.body);
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));
      expect(behind).not.toHaveBeenCalled();
      // Tab goes back into the sheet, to its first control.
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
      expect(document.activeElement).toBe(wrapper.get('input[type="radio"]:checked').element);
      // Focus that moves to the page behind is taken back to the sheet.
      const outside = document.createElement('button');
      document.body.append(outside);
      outside.focus();
      expect(wrapper.get('[role="dialog"]').element.contains(document.activeElement)).toBe(true);
      // Escape closes it from anywhere.
      (document.activeElement as HTMLElement).blur();
      document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      expect(s.open).toBe(false);
    } finally {
      window.removeEventListener('keydown', behind);
    }
  });

  it('asks for a second pin when only one is pinned', async () => {
    const { wrapper } = await mountWith(1, 'sideBySide');
    expect(wrapper.get('[role="status"]').text()).toContain('Pin at least two fonts');
  });

  it('is a bottom sheet whose height, scrim and Auto size follow the handle and maximize', async () => {
    const { wrapper, s } = await mountWith(2, 'stacked');
    const sheet = useCompareSheetStore();
    const dialog = wrapper.get('[role="dialog"]');
    const el = dialog.element as HTMLElement;
    const sample = () => (wrapper.get('.face .sample').element as HTMLElement).style.fontSize;
    expect(el.style.height).toBe('60%');
    expect(wrapper.get('[data-testid="compare-count"]').text()).toBe(
      `2 of 4 · base ${name(faces[0]!)}`,
    );

    // The handle's keys: Home is the lowest height (56 px), End the full one (80 px).
    const handle = wrapper.get('[role="separator"]');
    await handle.trigger('keydown', { key: 'Home' });
    expect(el.style.height).toBe('40%');
    expect(sample()).toBe('56px');
    expect(handle.attributes('aria-valuenow')).toBe('40');
    expect(JSON.parse(localStorage.getItem(COMPARE_SHEET_STORAGE_KEY)!).height).toBe(0.4);
    await handle.trigger('keydown', { key: 'ArrowUp' });
    expect(sheet.height).toBeCloseTo(0.45);

    // Maximize: the whole window, no radius, no scrim; again goes back.
    await wrapper.get('[data-testid="compare-maximize"]').trigger('click');
    expect(el.style.height).toBe('100%');
    expect(dialog.classes()).toContain('full');
    expect(wrapper.get('[data-testid="compare-scrim"]').classes()).toContain('clear');
    expect(sample()).toBe('80px');
    expect(wrapper.get('[data-testid="compare-maximize"]').attributes('aria-label')).toBe(
      'Restore height',
    );
    // A double-click on the handle toggles too.
    await handle.trigger('dblclick');
    expect(sheet.height).toBeCloseTo(0.45);
    expect(dialog.classes()).not.toContain('full');
    expect(wrapper.get('[data-testid="compare-scrim"]').classes()).not.toContain('clear');

    // A press on the scrim closes the sheet.
    await wrapper.get('[data-testid="compare-scrim"]').trigger('mousedown');
    expect(s.open).toBe(false);
  });

  it('opens the saved comparisons from the bookmark; Escape closes the list first', async () => {
    const { wrapper, s } = await mountWith(2, 'stacked');
    const button = wrapper.get('button[aria-label="Saved comparisons"]');
    expect(button.attributes('aria-expanded')).toBe('false');
    await button.trigger('click');
    expect(button.attributes('aria-expanded')).toBe('true');
    expect(wrapper.get('[data-testid="compare-saved"]').text()).toContain(
      'No saved comparisons yet.',
    );
    (wrapper.get('[data-testid="compare-saved"]').element as HTMLElement).focus();
    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' });
    expect(wrapper.find('[data-testid="compare-saved"]').exists()).toBe(false);
    expect(document.activeElement).toBe(button.element);
    expect(s.open).toBe(true);
    await wrapper.get('[role="dialog"]').trigger('keydown', { key: 'Escape' });
    expect(s.open).toBe(false);
  });
});
