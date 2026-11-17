import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, type MockOptions } from '../api/mock';
import { FaceLoader, faceFamily, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { useLibraryStore } from '../stores/library';
import { DEFAULT_PREVIEW } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { useSimilarStore } from '../stores/similar';
import SimilarPanel from './SimilarPanel.vue';

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return Promise.resolve(this);
  }
}
const fakeSet: FontFaceSetLike<FakeFace> = { add: () => undefined, delete: () => true };

let client: FmClient;

async function setup(options: MockOptions = {}) {
  client = createMockClient(options);
  setClient(client);
  await useLibraryStore().load();
  const wrapper = mount(SimilarPanel, {
    props: { preview: { ...DEFAULT_PREVIEW } },
    attachTo: document.body,
  });
  const similar = useSimilarStore();
  await similar.openFor((await client.getFace(1)).face);
  await flushPromises();
  return { wrapper, similar };
}

beforeEach(() => {
  setActivePinia(createPinia());
  setFaceLoader(
    new FaceLoader<FakeFace>({
      fonts: fakeSet,
      create: (family, source) => new FakeFace(family, source),
      url: (face) => `fmfont://localhost/${face}`,
    }),
  );
});
afterEach(() => {
  setClient(undefined);
  setFaceLoader(undefined);
  document.body.innerHTML = '';
});

describe('SimilarPanel', () => {
  it('is titled after the face and lists each result in its own font, with score and reasons', async () => {
    const { wrapper, similar } = await setup();
    const region = wrapper.get('[role="region"]');
    expect(wrapper.get('#fm-similar-title').text()).toBe('Similar to Inter Regular');
    expect(region.attributes('aria-labelledby')).toBe('fm-similar-title');
    const buttons = wrapper.findAll('button.result');
    expect(buttons).toHaveLength(similar.results.length);
    const first = similar.results[0]!;
    const percent = Math.round((first.score ?? 0) * 100);
    const name = `${first.face.familyName} ${first.face.style}`;
    expect(buttons[0]!.attributes('aria-label')).toBe(`${name}, ${percent}% similar`);
    expect(buttons[0]!.get('[data-testid="similar-score"]').text()).toBe(`${percent}% similar`);
    // Only what differs from the other results is explained ("same weight" is shared).
    expect(buttons[0]!.text()).toContain('Why: same classification');
    expect(buttons[0]!.text()).not.toContain('same weight');
    // Live text in the face's own font, not an image.
    const sample = buttons[0]!.get('.sample');
    expect(sample.attributes('style')).toContain(faceFamily(first.face.id));
    expect(buttons[0]!.find('img').exists()).toBe(false);
  });

  it('shows one row per family with its closest face and a "+N more" count', async () => {
    const { wrapper, similar } = await setup();
    const first = similar.results[0]!;
    similar.results = [
      first,
      { ...first, score: 0.1, face: { ...first.face, id: 9001, style: 'Bold' } },
      { ...first, score: 0.05, face: { ...first.face, id: 9002, style: 'Italic' } },
    ];
    await flushPromises();
    const buttons = wrapper.findAll('button.result');
    expect(buttons).toHaveLength(1);
    expect(buttons[0]!.text()).toContain('+2 more styles');
  });

  it('the first result takes the focus; arrows move; Enter selects', async () => {
    const { wrapper, similar } = await setup();
    const buttons = wrapper.findAll<HTMLButtonElement>('button.result');
    expect(document.activeElement).toBe(buttons[0]!.element);
    await buttons[0]!.trigger('keydown', { key: 'ArrowDown' });
    expect(document.activeElement).toBe(buttons[1]!.element);
    await buttons[1]!.trigger('keydown', { key: 'End' });
    expect(document.activeElement).toBe(buttons.at(-1)!.element);
    await buttons[1]!.trigger('click');
    await flushPromises();
    const picked = similar.results[1]!.face.id;
    expect(wrapper.emitted('select')).toEqual([[picked]]);
    expect(similar.picked).toBe(picked);
    expect(buttons[1]!.attributes('aria-current')).toBe('true');
  });

  it('Escape closes the panel', async () => {
    const { wrapper, similar } = await setup();
    await wrapper.get('button.result').trigger('keydown', { key: 'Escape' });
    expect(similar.open).toBe(false);
    expect(wrapper.find('[role="region"]').exists()).toBe(false);
  });

  it('says in plain words when nothing is close enough, and offers the way back', async () => {
    const { wrapper, similar } = await setup({ similar: 'none' });
    expect(wrapper.findAll('button.result')).toHaveLength(0);
    const status = wrapper.get('[data-testid="similar-empty"] [role="status"]');
    expect(status.get('h2').text()).toBe('Nothing is close enough');
    expect(status.text()).toContain('No font in the library looks close enough to this one.');
    expect(wrapper.find('[data-testid="similar-count"]').text()).toBe('0 fonts');
    const back = wrapper.get('[data-testid="similar-empty-back"]');
    expect(back.text()).toBe('Back to library Esc');
    await back.trigger('click');
    expect(similar.open).toBe(false);
  });

  it('says why a face cannot be compared', async () => {
    const { wrapper } = await setup({ similar: 'unmeasured' });
    const status = wrapper.get('[data-testid="similar-empty"] [role="status"]');
    expect(status.get('h2').text()).toBe('This font cannot be compared');
    expect(status.text()).toContain('This font cannot be compared: it does not draw');
  });

  it('has a header: the icon, the title, the count, Back to library with Esc, and close', async () => {
    const { wrapper, similar } = await setup();
    const head = wrapper.get('header.head');
    expect(head.find('[data-testid="similar-icon"]').exists()).toBe(true);
    expect(head.get('h2').text()).toBe('Similar to Inter Regular');
    expect(head.get('[data-testid="similar-count"]').text()).toBe(`${similar.rows.length} fonts`);
    const back = head.get('[data-testid="similar-back"]');
    expect(back.text()).toBe('Back to library Esc');
    expect(back.get('kbd').text()).toBe('Esc');
    expect(back.attributes('aria-keyshortcuts')).toBe('Escape');
    // Back sits beside the close button, the last thing in the header.
    const buttons = head.findAll('button');
    expect(buttons.at(-2)!.element).toBe(back.element);
    expect(buttons.at(-1)!.attributes('aria-label')).toBe('Close the similar fonts');
    await back.trigger('click');
    expect(similar.open).toBe(false);
  });

  it('sets the reference apart in its own labelled block, not as a result', async () => {
    const { wrapper } = await setup();
    const ref = wrapper.get('[data-testid="similar-source"]');
    expect(ref.element.tagName).toBe('SECTION');
    const label = ref.get('h3');
    expect(label.text()).toBe('Reference');
    expect(ref.attributes('aria-labelledby')).toBe(label.attributes('id'));
    expect(ref.text()).toContain('Inter Regular');
    expect(wrapper.get('ul.list').element.contains(ref.element)).toBe(false);
    expect(wrapper.findAll('button.result').map((b) => b.attributes('data-face'))).not.toContain(
      '1',
    );
  });

  it('gives each percentage bar an accessible name and value', async () => {
    const { wrapper, similar } = await setup();
    const bar = wrapper.findAll('button.result')[0]!.get('[data-testid="similar-bar"]');
    const percent = similar.rows[0]!.percent!;
    expect(bar.attributes('role')).toBe('meter');
    expect(bar.attributes('aria-label')).toBe(`Similarity: ${percent}%`);
    expect(bar.attributes('aria-valuenow')).toBe(String(percent));
    expect(bar.attributes('aria-hidden')).toBeUndefined();
  });

  it('says what a fallback text means in words, not with a bare "Fallback" badge', async () => {
    const { wrapper, similar } = await setup();
    const samples = useSamplesStore();
    const first = similar.rows[0]!.result.face;
    vi.spyOn(samples, 'get').mockImplementation((id) =>
      id === first.id
        ? { face: id, text: 'αβγ', fallback: { kind: 'script', script: 'Grek' }, missing: [] }
        : undefined,
    );
    // Re-render with the stubbed answer.
    similar.results = [...similar.results];
    await flushPromises();
    const button = wrapper.findAll('button.result')[0]!;
    expect(button.find('[data-testid="fallback-hint"]').exists()).toBe(false);
    const words = button.get('[data-testid="similar-fallback"]');
    expect(words.text()).toBe(
      'This font lacks characters of the sample text, so it shows its own letters.',
    );
    expect(button.attributes('aria-describedby')).toContain(words.attributes('id'));
    // A row with the sample says nothing.
    expect(
      wrapper.findAll('button.result')[1]!.get('[data-testid="similar-fallback"]').text(),
    ).toBe('');
  });

  it("shows the core's error as a plain message", async () => {
    const { wrapper } = await setup({ similar: 'fail' });
    expect(wrapper.findAll('button.result')).toHaveLength(0);
    expect(wrapper.get('[role="alert"]').text()).toBe(
      'Font Fu could not find similar fonts. Finding similar fonts is not available yet.',
    );
  });
});
