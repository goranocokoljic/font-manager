import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

describe('CardLiveText fits its width', () => {
  /** Lays the line out as `measure` says for the font size on its card. */
  function layOut(measure: (fontPx: number) => number) {
    return vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
      this: HTMLElement,
    ) {
      const px = parseFloat(this.parentElement?.style.fontSize ?? '') || 0;
      return { width: measure(px) } as DOMRect;
    });
  }
  const fontPx = (card: VueWrapper) =>
    parseFloat((card.get('[data-testid="card-live-text"]').element as HTMLElement).style.fontSize);

  afterEach(() => vi.restoreAllMocks());

  it('shrinks text in proportion when its width is in proportion to its size', async () => {
    layOut((px) => px * 10);
    const card = mount(CardLiveText, {
      props: { face: 7, text: 'Wide', sizePx: 120, fitWidth: 500 },
    });
    await flushPromises();
    expect(fontPx(card)).toBeCloseTo(50);
    card.unmount();
  });

  it('keeps shrinking when whole-pixel glyphs leave the first size too wide', async () => {
    // 20 glyphs, each a whole pixel wide: the width moves in steps of 20 px.
    const width = (px: number) => 20 * Math.ceil(px / 2);
    layOut(width);
    const card = mount(CardLiveText, {
      props: { face: 7, text: 'Wide', sizePx: 120, fitWidth: 510 },
    });
    await flushPromises();
    expect(width(fontPx(card))).toBeLessThanOrEqual(510);
    expect(fontPx(card)).toBeGreaterThan(45);
    card.unmount();
  });

  it('counts half a pixel over as too wide', async () => {
    layOut((px) => px * 10 + 0.4);
    const card = mount(CardLiveText, {
      props: { face: 7, text: 'Wide', sizePx: 120, fitWidth: 500 },
    });
    await flushPromises();
    expect(fontPx(card) * 10 + 0.4).toBeLessThanOrEqual(500);
    card.unmount();
  });

  it('leaves text that fits at its size', async () => {
    layOut((px) => px * 2);
    const card = mount(CardLiveText, {
      props: { face: 7, text: 'Aa', sizePx: 120, fitWidth: 500 },
    });
    await flushPromises();
    expect(fontPx(card)).toBe(120);
    card.unmount();
  });
});

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

describe('CardLiveText tofu (E43.4)', () => {
  it('draws the characters the face lacks as boxes, and the rest as text', async () => {
    const card = mount(CardLiveText, {
      props: { face: 7, text: 'pokaže štos', sizePx: 24, missing: ['ž', 'š'] },
    });
    await flushPromises();
    const boxes = card.findAll('[role="img"]');
    expect(boxes.map((b) => b.attributes('aria-label'))).toEqual([
      'missing glyph ž',
      'missing glyph š',
    ]);
    expect(card.get('.line').text()).toBe('pokae tos');
    card.unmount();
  });

  it('draws text with nothing missing whole', async () => {
    const card = mount(CardLiveText, { props: { face: 7, text: 'pokaže', sizePx: 24 } });
    await flushPromises();
    expect(card.find('[role="img"]').exists()).toBe(false);
    expect(card.get('.line').text()).toBe('pokaže');
    card.unmount();
  });
});
