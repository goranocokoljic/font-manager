// E46.4: a count that ticks up crossfades its old and new number, both at once (FmCrossfade).
import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { defineComponent, h, nextTick, ref } from 'vue';
import FmCrossfade from './FmCrossfade.vue';

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

/** A count in a row, as the Collections sidebar has it. */
function mountCount(value = ref(31)) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const wrapper = mount(
    defineComponent({
      setup: () => () =>
        h('div', { class: 'row' }, [
          h(
            FmCrossfade,
            { value: value.value, speed: 'fast', class: 'count', 'data-testid': 'count' },
            () => String(value.value),
          ),
        ]),
    }),
    { attachTo: host },
  );
  const row = wrapper.element as HTMLElement;
  const count = row.querySelector<HTMLElement>('[data-testid="count"]')!;
  count.style.setProperty('--fm-motion-fast', '120ms');
  return { value, wrapper, row, count };
}

describe('FmCrossfade', () => {
  it('passes its attributes to the one element it draws', () => {
    const { count } = mountCount();
    expect(count.tagName).toBe('SPAN');
    expect(count.classList.contains('count')).toBe(true);
    expect(count.textContent).toBe('31');
  });

  it('lays the old number over the new one, both fading at the fast token', async () => {
    const { value, row, count } = mountCount();
    value.value = 34;
    await nextTick();
    await nextTick();
    expect(count.textContent).toBe('34');
    const copy = row.lastElementChild as HTMLElement;
    expect(copy).not.toBe(count);
    expect(copy.textContent).toBe('31');
    // The copy is no second count: no test id, hidden from assistive technology.
    expect(copy.hasAttribute('data-testid')).toBe(false);
    expect(copy.getAttribute('aria-hidden')).toBe('true');
    expect(copy.classList.contains('fm-crossfade-hidden')).toBe(true);
    expect(count.classList.contains('fm-crossfade')).toBe(true);
    expect(count.style.getPropertyValue('--fm-transition-time')).toBe('var(--fm-motion-fast)');
  });

  it('cuts under reduced motion: no copy', async () => {
    vi.stubGlobal('matchMedia', (q: string) => ({
      matches: q.includes('reduce'),
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    const { value, row, count } = mountCount();
    value.value = 34;
    await nextTick();
    await nextTick();
    expect(count.textContent).toBe('34');
    expect(row.children).toHaveLength(1);
  });

  it('does nothing while the value stays', async () => {
    const { wrapper, row } = mountCount();
    await wrapper.vm.$forceUpdate();
    await nextTick();
    expect(row.children).toHaveLength(1);
  });
});
