import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, type MockOptions } from '../api/mock';
import { FaceLoader, faceFamily, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { useLibraryStore } from '../stores/library';
import { DEFAULT_PREVIEW } from '../stores/preview';
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
    expect(buttons[0]!.text()).toContain(`Why: ${first.reasons.join(', ')}`);
    // Live text in the face's own font, not an image.
    const sample = buttons[0]!.get('.sample');
    expect(sample.attributes('style')).toContain(faceFamily(first.face.id));
    expect(buttons[0]!.find('img').exists()).toBe(false);
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

  it('says so when no face is similar', async () => {
    const { wrapper } = await setup({ similar: 'none' });
    expect(wrapper.findAll('button.result')).toHaveLength(0);
    expect(wrapper.get('[role="status"]').text()).toBe(
      'No other font in the library looks like Inter Regular.',
    );
  });

  it("shows the core's error as a plain message", async () => {
    const { wrapper } = await setup({ similar: 'fail' });
    expect(wrapper.findAll('button.result')).toHaveLength(0);
    expect(wrapper.get('[role="alert"]').text()).toBe(
      'Font Manager could not find similar fonts. Finding similar fonts is not available yet.',
    );
  });
});
