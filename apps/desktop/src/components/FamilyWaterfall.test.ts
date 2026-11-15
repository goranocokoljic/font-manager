// The Waterfall section of the family page (E50.10).
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import type { FaceSummary } from '../api/types';
import { FAMILY_WATERFALL_SIZES, WATERFALL_SIZES } from '../list/waterfall';
import { TOFU_CLASS } from '../preview/tofu';
import { useAxesStore } from '../stores/axes';
import FamilyWaterfall from './FamilyWaterfall.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const TEXT = 'Ljubazni fenjerdžija čađavog lica';
const INTER = mockFaces[0]!.face;

async function mountWith(face: FaceSummary | null, missing: string[] = []) {
  const wrapper = mount(FamilyWaterfall, {
    props: { face, text: TEXT, missing },
    global: { plugins: [createPinia()] },
  });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const lines = (w: W) => w.findAll('[data-testid="family-waterfall-line"]');

beforeEach(() => setActivePinia(createPinia()));
beforeEach(() => setClient(createMockClient()));
afterEach(() => setClient(undefined));

describe('family page waterfall section', () => {
  it('the ladder: 8 to 10 sizes from 10 to 96 px, smallest first', () => {
    expect(FAMILY_WATERFALL_SIZES.length).toBeGreaterThanOrEqual(8);
    expect(FAMILY_WATERFALL_SIZES.length).toBeLessThanOrEqual(10);
    expect(FAMILY_WATERFALL_SIZES[0]).toBe(10);
    expect(FAMILY_WATERFALL_SIZES.at(-1)).toBe(96);
    expect([...FAMILY_WATERFALL_SIZES]).toEqual([...FAMILY_WATERFALL_SIZES].sort((a, b) => a - b));
  });

  it("leaves the wall's own ladder as it was", () => {
    expect([...WATERFALL_SIZES]).toEqual([12, 16, 24, 32, 48, 72]);
  });

  it('shows the text in the face at every size, each line labelled with its size', async () => {
    const w = await mountWith(INTER);
    const all = lines(w);
    expect(all.map((l) => Number(l.attributes('data-size')))).toEqual([...FAMILY_WATERFALL_SIZES]);
    for (const [k, line] of all.entries()) {
      const size = FAMILY_WATERFALL_SIZES[k];
      expect(line.attributes('data-face')).toBe(String(INTER.id));
      expect(line.get('.label [data-family-name]').text()).toBe(String(size));
      const sample = line.get('.sample');
      expect(sample.attributes('style')).toContain(`font-size: ${size}px`);
      expect(sample.text()).toBe(TEXT);
    }
    // No Variable pill on the lines: the hero already says so.
    expect(w.find('[data-testid="variable-badge"]').exists()).toBe(false);
  });

  it("follows the hero's instance: a weight set on the hero applies to every line", async () => {
    const detail = await useClient().getFace(INTER.id);
    const wght = detail.axes.find((a) => a.tag === 'wght')!;
    const w = await mountWith(INTER);
    useAxesStore().set(INTER.id, wght, 650);
    await flushPromises();
    for (const line of lines(w))
      expect(line.get('.sample').attributes('style')).toContain('"wght" 650');
  });

  it('draws the characters the face lacks as tofu, and lists them only once (under the hero)', async () => {
    const w = await mountWith(INTER, ['đ']);
    for (const line of lines(w)) expect(line.find(`.${TOFU_CLASS}`).exists()).toBe(true);
  });

  it('shows no lines before the face has loaded', async () => {
    const w = await mountWith(null);
    expect(lines(w)).toHaveLength(0);
    expect(w.text()).toContain('10 → 96 px');
  });
});
