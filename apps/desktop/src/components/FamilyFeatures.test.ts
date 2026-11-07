import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FaceDetail, FeatureInfo } from '../api/types';
import { useFeaturesStore } from '../stores/features';
import FamilyFeatures from './FamilyFeatures.vue';

const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const PREVIEW = 'Preview words for ss01';

async function detailOf(face: number): Promise<FaceDetail> {
  return useClient().getFace(face);
}

async function mountWith(detail: FaceDetail | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  const wrapper = mount(FamilyFeatures, {
    props: { detail, text: PREVIEW },
    global: { plugins: [pinia] },
  });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const switchFor = (w: W, tag: string) => w.get(`[role="switch"][data-tag="${tag}"]`);
const rowFor = (w: W, tag: string) => w.get(`li[data-tag="${tag}"]`);
const copyButton = (w: W) => w.get('[data-testid="features-copy"]');
const resetButton = (w: W) =>
  w.findAll('button').find((b) => b.text() === 'Reset') ?? w.get('nothing');

const feature = (tag: string, extra: Partial<FeatureInfo> = {}): FeatureInfo => ({
  tag,
  name: `Name of ${tag}`,
  automatic: false,
  defaultOn: false,
  group: null,
  sample: `sample of ${tag}`,
  ...extra,
});

beforeEach(() => setActivePinia(createPinia()));
beforeEach(() => setClient(createMockClient()));
afterEach(() => setClient(undefined));

describe('family page features section', () => {
  it('says so before the face has loaded', async () => {
    expect((await mountWith(null)).text()).toContain('Select a face');
  });

  it('shows each switchable feature as a row: name, tag, off and on samples, a switch', async () => {
    const w = await mountWith(await detailOf(1));
    const rows = w.get('ul[aria-label="Features"]').findAll('li');
    expect(rows.map((r) => r.attributes('data-tag'))).toEqual([
      'kern',
      'liga',
      'onum',
      'smcp',
      'ss01',
    ]);
    const smcp = rowFor(w, 'smcp');
    expect(smcp.get('.name').text()).toContain('Small capitals');
    expect(smcp.get('.tag').text()).toBe('smcp');
    const off = smcp.get('.sample.off');
    const on = smcp.get('.sample.on');
    expect(off.attributes('style')).toContain('font-feature-settings: "smcp" 0');
    expect(on.attributes('style')).toContain('font-feature-settings: "smcp" 1');
    expect(switchFor(w, 'smcp').attributes('aria-label')).toBe('Small capitals (smcp)');
    expect(
      w
        .findAll('ul[aria-label="Features"] [role="switch"]')
        .map((s) => s.attributes('aria-checked')),
    ).toEqual(['true', 'true', 'false', 'false', 'false']);
  });

  it('shows each feature its own sample, and a stylistic set the preview text', async () => {
    const w = await mountWith(await detailOf(1));
    expect(rowFor(w, 'liga').get('.sample.on').text()).toBe('fi fl ffi ffl');
    expect(rowFor(w, 'smcp').get('.sample.on').text()).toBe('Small Capitals');
    expect(rowFor(w, 'onum').get('.sample.on').text()).toBe('0123456789');
    expect(rowFor(w, 'ss01').get('.sample.off').text()).toBe(PREVIEW);
    expect(rowFor(w, 'ss01').get('.sample.on').text()).toBe(PREVIEW);
  });

  it('keeps rare tags behind "N more", and the common ones out of it', async () => {
    const base = await detailOf(1);
    const w = await mountWith({
      ...base,
      features: [
        feature('cv01'),
        feature('frac', { sample: '1/2 3/4 7/8' }),
        feature('locl', { automatic: true }),
        feature('mark', { automatic: true }),
        feature('jp78'),
      ],
    });
    const main = w.get('ul[aria-label="Features"]');
    expect(main.findAll('li').map((r) => r.attributes('data-tag'))).toEqual(['frac']);
    const more = w.get('button[aria-expanded]');
    expect(more.text()).toBe('4 more');
    expect(more.attributes('aria-expanded')).toBe('false');
    expect(w.find('ul[aria-label="Rare features"]').exists()).toBe(false);

    await more.trigger('click');
    expect(more.attributes('aria-expanded')).toBe('true');
    const rare = w.get('ul[aria-label="Rare features"]');
    expect(rare.findAll('li').map((r) => r.attributes('data-tag'))).toEqual([
      'cv01',
      'locl',
      'mark',
      'jp78',
    ]);
    // A rare feature a user can switch keeps its switch; the shaper's own have none.
    expect(rare.find('[role="switch"][data-tag="cv01"]').exists()).toBe(true);
    expect(rare.find('[role="switch"][data-tag="locl"]').exists()).toBe(false);
    expect(rowFor(w, 'locl').text()).toContain('Applied automatically');
    // A character variant shows the preview text; frac its fractions.
    expect(rowFor(w, 'cv01').get('.sample.on').text()).toBe(PREVIEW);
    expect(rowFor(w, 'frac').get('.sample.on').text()).toBe('1/2 3/4 7/8');
  });

  it('has no expander when every feature is common', async () => {
    const base = await detailOf(1);
    const w = await mountWith({ ...base, features: [feature('liga'), feature('smcp')] });
    expect(w.find('button[aria-expanded]').exists()).toBe(false);
  });

  it('switches a feature for the face and Reset restores the defaults', async () => {
    const w = await mountWith(await detailOf(1));
    expect(resetButton(w).attributes('disabled')).toBeDefined();
    await switchFor(w, 'smcp').trigger('click');
    await switchFor(w, 'kern').trigger('click');
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('true');
    expect(switchFor(w, 'kern').attributes('aria-checked')).toBe('false');
    expect(useFeaturesStore().settings(1)).toBe('"kern" 0, "smcp" 1');
    await resetButton(w).trigger('click');
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('false');
    expect(useFeaturesStore().settings(1)).toBeUndefined();
  });

  it('keeps the switches per face', async () => {
    const w = await mountWith(await detailOf(1));
    await switchFor(w, 'smcp').trigger('click');
    await w.setProps({ detail: await detailOf(6) });
    expect(w.findAll('[role="switch"]').map((s) => s.attributes('aria-checked'))).toEqual([
      'true',
      'true',
    ]);
    await w.setProps({ detail: await detailOf(1) });
    expect(switchFor(w, 'smcp').attributes('aria-checked')).toBe('true');
  });

  it('copies the font-feature-settings line from fm-core and counts the switched features', async () => {
    copyText.mockReset().mockResolvedValue(undefined);
    const w = await mountWith(await detailOf(1));
    expect(copyButton(w).text()).toBe('Copy CSS for 0 switched on');
    expect(copyButton(w).attributes('disabled')).toBeDefined();
    const cssValues = vi.spyOn(useClient(), 'cssValues');
    await switchFor(w, 'smcp').trigger('click');
    expect(copyButton(w).text()).toBe('Copy CSS for 1 switched on');
    await switchFor(w, 'onum').trigger('click');
    expect(copyButton(w).text()).toBe('Copy CSS for 2 switched on');
    await copyButton(w).trigger('click');
    await flushPromises();
    expect(cssValues).toHaveBeenLastCalledWith({
      face: 1,
      variations: [],
      features: [
        { tag: 'smcp', on: true },
        { tag: 'onum', on: true },
      ],
    });
    expect(copyText).toHaveBeenLastCalledWith('font-feature-settings: "onum" 1, "smcp" 1;');
    expect(w.get('[role="status"]').text()).toBe('Copied');
  });

  it('says "Could not copy" when the clipboard refuses', async () => {
    copyText.mockReset().mockRejectedValue(new Error('refused'));
    const w = await mountWith(await detailOf(1));
    await switchFor(w, 'smcp').trigger('click');
    await copyButton(w).trigger('click');
    await flushPromises();
    expect(w.get('[role="status"]').text()).toBe('Could not copy');
  });
});
