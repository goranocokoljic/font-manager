import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceDetail, FamilySummary } from '../api/types';
import { useSelectionStore } from '../stores/selection';
import FamilyInfo from './FamilyInfo.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const STYLE = { fontFamily: "'held-face'" };

async function faceOne(): Promise<{ detail: FaceDetail; summary: FamilySummary }> {
  const detail = await useClient().getFace(1);
  const family = await useClient().getFamily(detail.face.family);
  return {
    detail,
    summary: { ...family.summary, foundry: 'Inter Type Foundry', classification: 'sans' },
  };
}

function mountWith(
  detail: FaceDetail | null,
  summary: FamilySummary | null,
  extra: { year?: number | null; faceStyle?: Record<string, string> } = {},
) {
  return mount(FamilyInfo, {
    props: {
      detail,
      summary,
      year: extra.year === undefined ? 2016 : extra.year,
      faceStyle: extra.faceStyle ?? STYLE,
    },
    attachTo: document.body,
  });
}
type W = ReturnType<typeof mountWith>;
const value = (w: W, id: string) => w.get(`[data-testid="${id}"]`).text();
const line = (w: W, which: string) => w.find(`line[data-line="${which}"]`);

beforeEach(() => setActivePinia(createPinia()));
beforeEach(() => setClient(createMockClient()));
afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('family page info section', () => {
  it('waits for the face before it shows anything', () => {
    expect(mountWith(null, null).text()).toContain('Loading');
  });

  it('shows four groups: Design, Licence, Files, Metrics', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(detail, summary);
    expect(w.findAll('section h3').map((h) => h.text())).toEqual([
      'Design',
      'Licence',
      'Files',
      'Metrics',
    ]);
  });

  it('Design: designer, foundry · year, classification, axes with ranges, named styles, description', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(detail, summary);
    expect(value(w, 'info-designer')).toBe('Rasmus Andersson');
    expect(value(w, 'info-foundry')).toBe('Inter Type Foundry · 2016');
    expect(value(w, 'info-classification')).toBe('Sans serif');
    expect(value(w, 'info-axes')).toBe('Weight 100–900');
    expect(value(w, 'info-named-styles')).toBe('4');
    expect(value(w, 'info-description')).toBe(
      'A typeface carefully crafted and designed for computer screens.',
    );
  });

  it('Design: a part the font does not state is a dash, a missing year leaves the foundry alone', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(
      { ...detail, axes: [], instances: [], description: null },
      { ...summary, classification: null },
      { year: null },
    );
    expect(value(w, 'info-foundry')).toBe('Inter Type Foundry');
    expect(value(w, 'info-classification')).toBe('—');
    expect(value(w, 'info-axes')).toBe('—');
    expect(value(w, 'info-named-styles')).toBe('—');
    expect(value(w, 'info-description')).toBe('—');
  });

  it('Licence: the name and the You may / You may not lines from LicenseInfo.permissions', async () => {
    const { detail, summary } = await faceOne();
    const permissions = {
      may: ['Sell prints', 'Embed in PDFs'],
      mayNot: ['Resell the files'],
      source: 'open-license' as const,
    };
    const w = mountWith({ ...detail, license: { ...detail.license, permissions } }, summary);
    expect(value(w, 'info-licence')).toBe('SIL Open Font License');
    expect(value(w, 'info-may')).toBe('Sell prints · Embed in PDFs');
    expect(value(w, 'info-may-not')).toBe('Resell the files');
  });

  it('Licence: no You may lines when fm-core knows nothing about the licence', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith({ ...detail, license: { ...detail.license, permissions: null } }, summary);
    expect(w.find('[data-testid="info-may"]').exists()).toBe(false);
    expect(w.find('[data-testid="info-may-not"]').exists()).toBe(false);
    expect(w.text()).not.toContain('You may');
  });

  it('Licence: the user record with a link that opens the declaration form', async () => {
    const { detail, summary } = await faceOne();
    useSelectionStore().detail = detail;
    const w = mountWith(detail, summary);
    const link = w.get('[data-testid="info-declare"]');
    expect(link.text()).toBe('Add your purchase or licence note');
    const fields = w.get('#fm-license-fields');
    expect(link.attributes('aria-controls')).toBe('fm-license-fields');
    expect(fields.isVisible()).toBe(false);
    await link.trigger('click');
    await flushPromises();
    expect(fields.isVisible()).toBe(true);
    expect(document.activeElement?.id).toBe('fm-license-type');
  });

  it('Licence: a declared record shows its type, seats and date', async () => {
    const { detail, summary } = await faceOne();
    const license = {
      ...detail.license,
      declared: 'web' as const,
      seats: 3,
      purchaseDate: '2026-03-14',
    };
    const w = mountWith({ ...detail, license }, summary);
    expect(w.get('[data-testid="info-record"] .record').text()).toBe(
      'Web · 3 seats · bought Mar 14, 2026',
    );
    expect(w.get('[data-testid="info-record"] button').text()).toBe('Edit your record');
  });

  it('Files: format, location, added, health', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(detail, { ...summary, added: '2026-10-26T09:00:00Z' });
    expect(value(w, 'info-format')).toBe('TrueType (TTF) · variable · 121 kB');
    expect(w.text()).toContain('/Users/demo/Fonts/Inter/Inter[wght].ttf');
    expect(value(w, 'info-added')).toBe('Oct 26, 2026');
    expect(w.get('[data-testid="health-badge"]').text()).toBe('OK');
  });

  it('Files: a large file reads in MB and a broken face shows why', async () => {
    const { detail, summary } = await faceOne();
    const broken = { kind: 'broken' as const, reason: 'The glyf table is cut off.' };
    const w = mountWith(
      {
        ...detail,
        fileSize: 1_130_000,
        face: { ...detail.face, isVariable: false, health: broken },
      },
      summary,
    );
    expect(value(w, 'info-format')).toBe('TrueType (TTF) · 1.1 MB');
    expect(w.get('[data-testid="health-badge"]').text()).toContain('The glyf table is cut off.');
  });

  it('Metrics: units/em, cap, x-height with its ratio, ascender, descender', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(detail, summary);
    expect(value(w, 'info-upm')).toBe('1,000');
    expect(value(w, 'info-cap')).toBe('720');
    expect(value(w, 'info-x')).toBe('520 (0.52)');
    expect(value(w, 'info-ascender')).toBe('900');
    expect(value(w, 'info-descender')).toBe('-250');
  });

  it('Metrics: Hxg at 96 px, dashed lines at cap and x-height, a solid baseline, from the metrics', async () => {
    const { detail, summary } = await faceOne();
    const w = mountWith(detail, summary);
    const text = w.get('[data-testid="metrics-specimen"] text');
    expect(text.text()).toBe('Hxg');
    expect(text.attributes('font-size')).toBe('96');
    expect(text.attributes('style')).toContain('held-face');
    // 96 px over 1000 units: the baseline 900 units down, cap 720 and x 520 above it.
    expect(Number(text.attributes('y'))).toBeCloseTo(86.4);
    expect(Number(line(w, 'baseline').attributes('y1'))).toBeCloseTo(86.4);
    expect(Number(line(w, 'cap').attributes('y1'))).toBeCloseTo(86.4 - 69.12);
    expect(Number(line(w, 'x').attributes('y1'))).toBeCloseTo(86.4 - 49.92);
    expect(line(w, 'cap').classes()).toContain('dashed');
    expect(line(w, 'x').classes()).toContain('dashed');
    expect(line(w, 'baseline').classes()).not.toContain('dashed');
  });

  it('Metrics: no x-height line or ratio when the font does not state one, no letters before the face loads', async () => {
    const { detail, summary } = await faceOne();
    const metrics = { ...detail.metrics, xHeight: null };
    const w = mount(FamilyInfo, { props: { detail: { ...detail, metrics }, summary } });
    expect(line(w, 'x').exists()).toBe(false);
    expect(line(w, 'cap').exists()).toBe(true);
    expect(value(w, 'info-x')).toBe('—');
    expect(w.find('[data-testid="metrics-specimen"] text').exists()).toBe(false);
  });
});
