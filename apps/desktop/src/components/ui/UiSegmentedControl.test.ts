import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import UiSegmentedControl from './UiSegmentedControl.vue';

const options = [
  { value: 'a', label: 'A' },
  { value: 'b', label: 'B' },
  { value: 'c', label: 'C' },
] as const;

// jsdom lays nothing out: each segment says it is 30 px wide, at 2 + 32 px per segment before it.
function stubLayout() {
  const left = vi.spyOn(HTMLElement.prototype, 'offsetLeft', 'get');
  left.mockImplementation(function (this: HTMLElement) {
    const i = [...(this.parentElement?.querySelectorAll('.seg') ?? [])].indexOf(this);
    return i < 0 ? 0 : 2 + 32 * i;
  });
  vi.spyOn(HTMLElement.prototype, 'offsetTop', 'get').mockReturnValue(2);
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(30);
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(24);
}

const mountIt = (props: Record<string, unknown>) =>
  mount(UiSegmentedControl, {
    props: { modelValue: 'a', options, label: 'Mode', ...props },
    attachTo: document.body,
  });

describe('UiSegmentedControl thumb (E46.2)', () => {
  beforeEach(stubLayout);
  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('draws no thumb without slide: the pressed segment fills itself', async () => {
    const w = mountIt({});
    await flushPromises();
    expect(w.find('.thumb').exists()).toBe(false);
    expect(w.classes()).not.toContain('sliding');
  });

  it('puts the thumb under the chosen segment and moves it there by transform', async () => {
    const w = mountIt({ slide: true });
    await flushPromises();
    const thumb = w.find('.thumb');
    expect(thumb.exists()).toBe(true);
    expect(thumb.attributes('aria-hidden')).toBe('true');
    expect(w.classes()).toContain('sliding');
    expect(thumb.attributes('style')).toContain('translate(2px, 2px)');
    expect(thumb.attributes('style')).toContain('width: 30px');

    await w.setProps({ modelValue: 'c' });
    await flushPromises();
    // The same element glides: a new one would not transition.
    expect(w.find('.thumb').element).toBe(thumb.element);
    expect(w.find('.thumb').attributes('style')).toContain('translate(66px, 2px)');
  });

  it('falls back to the filled segment when nothing is laid out', async () => {
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(0);
    const w = mountIt({ slide: true });
    await flushPromises();
    expect(w.find('.thumb').exists()).toBe(false);
    expect(w.classes()).not.toContain('sliding');
  });
});
