import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { mockFaces } from '../api/mock';
import type { FaceSummary, SimilarFace } from '../api/types';
import { FaceLoader, setFaceLoader } from '../preview/faceLoader';
import { useCompareStore } from '../stores/compare';
import { useSearchStore } from '../stores/search';
import CompareSheet from './CompareSheet.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

// Ten faces: 300 is the base, 301–309 come back from findSimilar.
const faces: FaceSummary[] = Array.from({ length: 10 }, (_, i) => ({
  ...mockFaces[i % mockFaces.length]!.face,
  id: 300 + i,
  familyName: `Family ${i}`,
  style: `Style ${i}`,
}));
const f = (i: number) => faces[i]!;
const DESIGNER = ['same designer', 'similar x-height'];
const WEIGHT = ['same weight'];
/** Best first: two rows share the designer, one the weight, one the designer again. */
const answer: SimilarFace[] = [
  { face: f(1), score: 0.9, reasons: DESIGNER },
  { face: f(2), score: 0.8, reasons: WEIGHT },
  { face: f(3), score: 0.7, reasons: DESIGNER },
  { face: f(4), score: 0.6, reasons: WEIGHT },
];

let findSimilar: ReturnType<typeof vi.fn>;

function setMatchMedia(reduced: boolean) {
  window.matchMedia = ((q: string) => ({
    matches: reduced && q.includes('reduce'),
    media: q,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
}

async function mountWith(pins: FaceSummary[]) {
  const pinia = createPinia();
  setActivePinia(pinia);
  localStorage.clear();
  const s = useCompareStore();
  pins.forEach((p) => s.pin(p));
  s.setMode('stacked');
  s.openSheet();
  await useSearchStore().loadLanguages();
  const wrapper = mount(CompareSheet, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return { wrapper, s };
}

beforeEach(() => {
  setClient(undefined);
  const client = useClient();
  findSimilar = vi.fn().mockResolvedValue(answer);
  setClient({ ...client, findSimilar } as typeof client);
  setFaceLoader(
    new FaceLoader({
      fonts: { add: () => undefined, delete: () => true },
      create: () => ({ load: () => Promise.resolve() }),
      url: (id) => `fmfont://${id}`,
    }),
  );
  setMatchMedia(false);
});
afterEach(() => {
  setClient(undefined);
  setFaceLoader(undefined);
  document.body.innerHTML = '';
});

describe('Similar to the base in the Compare sheet (E48.3)', () => {
  it('replaces Pairings with "Similar to <base>" from findSimilar for the base, best first', async () => {
    const { wrapper } = await mountWith([f(0), f(1)]);
    expect(wrapper.find('[data-testid="pairings-panel"]').exists()).toBe(false);
    expect(findSimilar).toHaveBeenCalledWith({ face: 300, limit: 8 });
    const band = wrapper.get('[data-testid="compare-similar"]');
    expect(band.attributes('role')).toBe('region');
    expect(band.get('h3').text()).toBe('Similar to Family 0');
    expect(band.find('.hint').text()).not.toBe('');
    // Grouped in rank order: the designer group (1, 3) leads, then the weight group (2, 4).
    const rows = band.findAll('[data-testid="similar-row"]').map((r) => r.attributes('data-face'));
    expect(rows).toEqual(['301', '303', '302', '304']);
  });

  it('names each shared reason once, never row by row, and shows no percent', async () => {
    const { wrapper } = await mountWith([f(0), f(5)]);
    const band = wrapper.get('[data-testid="compare-similar"]');
    const headers = band.findAll('[data-testid="similar-reason"]').map((h) => h.text());
    expect(headers).toEqual(['same designer · similar x-height', 'same weight']);
    for (const row of band.findAll('[data-testid="similar-row"]')) {
      expect(row.text()).not.toContain('same designer');
      expect(row.text()).not.toContain('same weight');
    }
    expect(band.text()).not.toMatch(/%/);
  });

  it('sets each row in the comparison’s text at half the compare size, with family and style under it', async () => {
    const { wrapper } = await mountWith([f(0), f(5)]);
    const faceSample = wrapper.get('.faces .sample').element as HTMLElement;
    const comparePx = parseFloat(faceSample.style.fontSize);
    const row = wrapper.get('[data-testid="similar-row"][data-face="301"]');
    const sample = row.get('.row-sample').element as HTMLElement;
    expect(parseFloat(sample.style.fontSize)).toBe(Math.round(comparePx * 0.5));
    expect(sample.textContent?.trim()).toBe(faceSample.textContent?.trim());
    expect(sample.style.fontFamily).toContain('fm-face');
    expect(row.get('.meta').text()).toBe('Family 1 Style 1');
    // The text is not a control: only the icon is.
    expect(row.findAll('button')).toHaveLength(1);
    expect(row.get('.row-sample').element.closest('button, a')).toBeNull();
  });

  it('the icon adds the face to the set, filled, and a second click removes it; a face in the set is listed once', async () => {
    const { wrapper, s } = await mountWith([f(0), f(3)]);
    const icon = (id: number) =>
      wrapper.get(`[data-testid="similar-row"][data-face="${id}"] [data-testid="similar-compare"]`);
    // Face 303 is already in the set: listed once, filled.
    expect(wrapper.findAll('[data-testid="similar-row"][data-face="303"]')).toHaveLength(1);
    expect(icon(303).attributes('data-in-set')).toBe('true');
    expect(icon(301).attributes('data-in-set')).toBe('false');
    await icon(301).trigger('click');
    expect(s.pins.map((p) => p.id)).toEqual([300, 303, 301]);
    expect(icon(301).attributes('data-in-set')).toBe('true');
    expect(icon(301).attributes('aria-label')).toBe('Remove Family 1 Style 1 from Compare');
    await icon(301).trigger('click');
    expect(s.pins.map((p) => p.id)).toEqual([300, 303]);
  });

  it('with a full set the icon is disabled with the reason in its title', async () => {
    const { wrapper, s } = await mountWith([f(0), f(1), f(6), f(7)]);
    const row = (id: number) =>
      wrapper.get(`[data-testid="similar-row"][data-face="${id}"] [data-testid="similar-compare"]`);
    expect(row(302).attributes('disabled')).toBeDefined();
    expect(row(302).attributes('title')).toBe(
      'Compare holds 4 fonts. Remove one to add Family 2 Style 2.',
    );
    await row(302).trigger('click');
    expect(s.pins).toHaveLength(4);
    // A face in the set can still be removed.
    expect(row(301).attributes('disabled')).toBeUndefined();
  });

  it('a new base reloads the section', async () => {
    const { wrapper, s } = await mountWith([f(0), f(1)]);
    findSimilar.mockResolvedValueOnce([{ face: f(8), score: 0.5, reasons: WEIGHT }]);
    s.onlyThis(f(1));
    await flushPromises();
    expect(findSimilar).toHaveBeenLastCalledWith({ face: 301, limit: 8 });
    const band = wrapper.get('[data-testid="compare-similar"]');
    expect(band.get('h3').text()).toBe('Similar to Family 1');
    expect(
      band.findAll('[data-testid="similar-row"]').map((r) => r.attributes('data-face')),
    ).toEqual(['308']);
  });

  it('keeps the loading, error and empty copy', async () => {
    let release: (v: SimilarFace[]) => void = () => undefined;
    findSimilar.mockImplementationOnce(() => new Promise((r) => (release = r)));
    const { wrapper, s } = await mountWith([f(0), f(1)]);
    const band = () => wrapper.get('[data-testid="compare-similar"]');
    expect(band().get('[role="status"]').text()).toBe('Looking for similar fonts…');
    release([]);
    await flushPromises();
    expect(band().get('[role="status"]').text()).toBe(
      'No other font in the library looks like Family 0 Style 0.',
    );
    findSimilar.mockRejectedValueOnce({ code: 'internal', message: 'Broken.', detail: '' });
    s.onlyThis(f(1));
    await flushPromises();
    expect(band().get('[role="alert"]').text()).toContain(
      'Font Manager could not find similar fonts.',
    );
  });

  it('plays the row-to-face move for an added face, and not under reduced motion', async () => {
    const { wrapper } = await mountWith([f(0), f(5)]);
    await wrapper
      .get('[data-testid="similar-row"][data-face="301"] [data-testid="similar-compare"]')
      .trigger('click');
    expect(wrapper.get('.faces .face[data-face="301"]').classes()).toContain('arrived');
    wrapper.unmount();

    setMatchMedia(true);
    const reduced = await mountWith([f(0), f(5)]);
    await reduced.wrapper
      .get('[data-testid="similar-row"][data-face="302"] [data-testid="similar-compare"]')
      .trigger('click');
    expect(reduced.wrapper.get('.faces .face[data-face="302"]').classes()).not.toContain('arrived');
  });
});
