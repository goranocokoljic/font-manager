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

describe('UiSegmentedControl segment that is off (E51.4, E54.5)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('keeps an off segment in place, focusable, with its reason, and ignores a press', async () => {
    const w = mountIt({
      options: [
        { value: 'a', label: 'A' },
        { value: 'b', label: 'B', reason: 'B does nothing here' },
        { value: 'c', label: 'C' },
      ],
    });
    const segs = w.findAll('.seg');
    expect(segs).toHaveLength(3);
    const off = segs[1]!;
    expect(off.attributes('aria-disabled')).toBe('true');
    expect(off.attributes('disabled')).toBeUndefined();
    expect(off.classes()).toContain('off');
    expect(off.attributes('aria-label')).toBe('B');
    expect(off.attributes('title')).toBe('B does nothing here');
    expect(w.get(`[id="${off.attributes('aria-describedby')}"]`).text()).toBe(
      'B does nothing here',
    );
    await off.trigger('click');
    expect(w.emitted('update:modelValue')).toBeUndefined();
    await segs[2]!.trigger('click');
    expect(w.emitted('update:modelValue')).toEqual([['c']]);
    expect(segs[2]!.attributes('aria-disabled')).toBeUndefined();
    expect(segs[2]!.attributes('title')).toBe('C');
  });
});
