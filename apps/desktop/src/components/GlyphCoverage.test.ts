import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useCoverageStore } from '../stores/coverage';
import GlyphCoverage from './GlyphCoverage.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountOn(face: number) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useCoverageStore().open(face);
  const wrapper = mount(GlyphCoverage, { global: { plugins: [pinia] }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

beforeEach(() => setClient(createMockClient()));
afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('GlyphCoverage', () => {
  it('lists blocks with covered / assigned as buttons that jump to the block', async () => {
    const w = await mountOn(7);
    const rows = w.findAll('[aria-labelledby="fm-coverage-blocks"] button');
    expect(rows.map((r) => r.attributes('aria-label'))).toEqual([
      'Basic Latin: 95 of 128 characters. Show in the glyph map.',
      'Latin-1 Supplement: 96 of 128 characters. Show in the glyph map.',
    ]);
    expect(rows[0]!.text()).toContain('95 of 128');
    await rows[1]!.trigger('click');
    expect(w.emitted('block')).toEqual([['Latin-1 Supplement']]);
  });

  it('lists complete languages first, partial ones with their count', async () => {
    const w = await mountOn(7);
    const rows = w.findAll('[aria-labelledby="fm-coverage-languages"] > li > button');
    const labels = rows.map((r) => r.attributes('aria-label'));
    expect(labels[0]).toBe('English: complete');
    expect(labels.slice(1)).toEqual([
      'Serbian (Latin): 22 of 27 characters',
      'Polish: 24 of 32 characters',
    ]);
    expect(rows[1]!.text()).toContain('22 of 27');
  });

  it('marks locl support', async () => {
    const w = await mountOn(1);
    const sr = w
      .findAll('[aria-labelledby="fm-coverage-languages"] > li > button')
      .find((r) => r.text().includes('Serbian (Cyrillic)'));
    expect(sr?.attributes('aria-label')).toBe(
      'Serbian (Cyrillic): complete, locl (has localized forms)',
    );
    expect(sr?.find('.badge').text()).toBe('locl');
  });

  it('shows the missing characters of a partial language on demand', async () => {
    const w = await mountOn(7);
    const pl = w
      .findAll('[aria-labelledby="fm-coverage-languages"] > li > button')
      .find((r) => r.text().includes('Polish'))!;
    expect(pl.attributes('aria-expanded')).toBe('false');
    await pl.trigger('click');
    await flushPromises();
    expect(pl.attributes('aria-expanded')).toBe('true');
    const panel = w.get(`#${pl.attributes('aria-controls')}`);
    expect(panel.get('[aria-label="Missing from Polish"]').text().replace(/\s/g, '')).toBe(
      'ąćęłńśźż',
    );
    await pl.trigger('click');
    expect(w.find(`#${pl.attributes('aria-controls')}`).exists()).toBe(false);
  });
});
