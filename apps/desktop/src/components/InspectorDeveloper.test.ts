import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient } from '../api/mock';
import { useAxesStore } from '../stores/axes';
import { useFeaturesStore } from '../stores/features';
import { useSelectionStore } from '../stores/selection';
import InspectorDeveloper from './InspectorDeveloper.vue';

const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorDeveloper, { global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const snippet = (w: W) => w.get('[data-testid="css-snippet"]').text();
const copyButton = (w: W) => w.get('button[aria-label="Copy the CSS snippet"]');

/** MockApi face 1 (Inter Regular): wght 100–900 default 400; smcp off, liga on by default. */
function move(face: number, tag: string, to: number): void {
  const axis = useSelectionStore().detail!.axes.find((a) => a.tag === tag)!;
  useAxesStore().set(face, axis, to);
}
function toggle(face: number, tag: string, on: boolean): void {
  const feature = useSelectionStore().detail!.features.find((f) => f.tag === tag)!;
  useFeaturesStore().set(face, feature, on);
}

beforeEach(() => setClient(createMockClient()));
afterEach(() => setClient(undefined));

describe('inspector developer tab', () => {
  it('says so when nothing is selected', async () => {
    const w = await mountWith(null);
    expect(w.text()).toContain('Select a face');
    expect(w.find('[data-testid="css-snippet"]').exists()).toBe(false);
  });

  it('reads normal for both declarations while nothing changed', async () => {
    const w = await mountWith(1);
    expect(snippet(w)).toBe('font-variation-settings: normal;\nfont-feature-settings: normal;');
  });

  it('asks fm-core for both declarations at once and updates live', async () => {
    const w = await mountWith(1);
    const cssValues = vi.spyOn(useClient(), 'cssValues');
    move(1, 'wght', 650);
    await flushPromises();
    expect(snippet(w)).toBe('font-variation-settings: "wght" 650;\nfont-feature-settings: normal;');
    toggle(1, 'smcp', true);
    toggle(1, 'liga', false);
    await flushPromises();
    expect(cssValues).toHaveBeenLastCalledWith({
      face: 1,
      variations: [{ tag: 'wght', value: 650 }],
      features: [
        { tag: 'smcp', on: true },
        { tag: 'liga', on: false },
      ],
    });
    expect(snippet(w)).toBe(
      'font-variation-settings: "wght" 650;\nfont-feature-settings: "liga" 0, "smcp" 1;',
    );
    // Back to the defaults: normal again.
    useAxesStore().reset(1);
    useFeaturesStore().reset(1);
    await flushPromises();
    expect(snippet(w)).toBe('font-variation-settings: normal;\nfont-feature-settings: normal;');
  });

  it('shows the state of the newly selected face', async () => {
    const w = await mountWith(1);
    move(1, 'wght', 700);
    await flushPromises();
    await useSelectionStore().select(6);
    await flushPromises();
    expect(snippet(w)).toBe('font-variation-settings: normal;\nfont-feature-settings: normal;');
    await useSelectionStore().select(1);
    await flushPromises();
    expect(snippet(w)).toContain('"wght" 700');
  });

  it('keeps the latest answer when an earlier one comes back later', async () => {
    const w = await mountWith(1);
    const real = useClient().cssValues;
    let release!: () => void;
    const held = new Promise<void>((r) => (release = r));
    vi.spyOn(useClient(), 'cssValues').mockImplementationOnce(async (req) => {
      await held;
      return real(req);
    });
    move(1, 'wght', 300);
    await flushPromises();
    move(1, 'wght', 800);
    await flushPromises();
    release();
    await flushPromises();
    expect(snippet(w)).toContain('"wght" 800');
  });

  it('shows the error when fm-core cannot answer', async () => {
    const w = await mountWith(1);
    vi.spyOn(useClient(), 'cssValues').mockRejectedValueOnce({
      code: 'notFound',
      message: 'Face 1 was not found.',
      detail: null,
    });
    move(1, 'wght', 500);
    await flushPromises();
    expect(w.get('[role="alert"]').text()).not.toBe('');
    expect(w.find('[data-testid="css-snippet"]').exists()).toBe(false);
  });

  it('copies the whole snippet with the inline "Copied" feedback', async () => {
    copyText.mockReset().mockResolvedValue(undefined);
    const w = await mountWith(1);
    move(1, 'wght', 650);
    toggle(1, 'smcp', true);
    await flushPromises();
    await copyButton(w).trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith(
      'font-variation-settings: "wght" 650;\nfont-feature-settings: "smcp" 1;',
    );
    expect(w.get('[role="status"]').text()).toBe('Copied');
    expect(copyButton(w).text()).toBe('Copy');
  });

  it('says "Could not copy" when the clipboard refuses', async () => {
    copyText.mockReset().mockRejectedValue(new Error('refused'));
    const w = await mountWith(1);
    await copyButton(w).trigger('click');
    await flushPromises();
    expect(w.get('[role="status"]').text()).toBe('Could not copy');
  });
});
