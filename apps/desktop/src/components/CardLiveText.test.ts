import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import CardLiveText from './CardLiveText.vue';

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return Promise.resolve(this);
  }
}

class FakeSet implements FontFaceSetLike<FakeFace> {
  add() {}
  delete() {
    return true;
  }
}

let loader: FaceLoader<FakeFace>;

beforeEach(() => {
  loader = new FaceLoader<FakeFace>({
    fonts: new FakeSet(),
    create: (family, source) => new FakeFace(family, source),
    url: (face) => `fmfont://localhost/${face}`,
  });
  setFaceLoader(loader);
});

afterEach(() => setFaceLoader(undefined));

describe('CardLiveText deferred while the grid moves fast (E35.8)', () => {
  it('shows an empty frame and loads nothing until defer is off', async () => {
    const card = mount(CardLiveText, { props: { face: 7, text: 'Aa', sizePx: 36, defer: true } });
    await flushPromises();
    const live = card.get('[data-testid="card-live-text"]');
    expect(live.attributes('data-state')).toBe('deferred');
    expect(live.text()).toBe('');
    expect(loader.loaded).toBe(0);
    await card.setProps({ defer: false });
    await flushPromises();
    expect(live.attributes('data-state')).toBe('ready');
    expect(live.text()).toBe('Aa');
    expect(loader.inUse).toBe(1);
    // A face it holds stays when the grid moves again; a new face waits.
    await card.setProps({ defer: true });
    await flushPromises();
    expect(live.attributes('data-state')).toBe('ready');
    await card.setProps({ face: 8 });
    await flushPromises();
    expect(live.attributes('data-state')).toBe('deferred');
    expect(loader.inUse).toBe(0);
    card.unmount();
  });
});
