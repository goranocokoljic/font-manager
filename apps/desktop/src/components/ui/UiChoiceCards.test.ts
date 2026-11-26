import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { h, nextTick, ref } from 'vue';
import UiChoiceCards, { type ChoiceCard } from './UiChoiceCards.vue';

type V = 'session' | 'permanent' | 'off';

const OPTIONS: ChoiceCard<V>[] = [
  { value: 'session', title: 'For this session', line: 'Until Font Fu quits.' },
  { value: 'off', title: 'Not now', disabled: true },
  { value: 'permanent', title: 'Permanently' },
];

let wrappers: { unmount(): void }[] = [];
afterEach(() => {
  wrappers.forEach((w) => w.unmount());
  wrappers = [];
});

function mountCards(start: V | null = 'session', columns?: 1 | 2 | 3) {
  const model = ref<V | null>(start);
  const wrapper = mount(
    {
      setup: () => () =>
        h(
          UiChoiceCards<V>,
          {
            modelValue: model.value,
            'onUpdate:modelValue': (v: V) => (model.value = v),
            options: OPTIONS,
            name: 'activation-mode',
            ariaLabel: 'The Activate button activates fonts',
            columns,
          },
          { figure: ({ option }: { option: ChoiceCard<V> }) => h('i', { class: option.value }) },
        ),
    },
    { attachTo: document.body },
  );
  wrappers.push(wrapper);
  const card = (v: V) => wrapper.get(`[role="radio"][data-value="${v}"]`);
  return { wrapper, model, card };
}

describe('UiChoiceCards', () => {
  it('is a named radio group of cards with the chosen one checked', () => {
    const { wrapper, card } = mountCards();
    const group = wrapper.get('[role="radiogroup"]');
    expect(group.attributes('aria-label')).toBe('The Activate button activates fonts');
    expect(group.attributes('data-name')).toBe('activation-mode');
    expect(wrapper.findAll('[role="radio"]')).toHaveLength(3);
    expect(card('session').attributes('aria-checked')).toBe('true');
    expect(card('permanent').attributes('aria-checked')).toBe('false');
    // The title names the card, the line describes it.
    const title = wrapper.get(`#${card('session').attributes('aria-labelledby')}`);
    expect(title.text()).toBe('For this session');
    const line = wrapper.get(`#${card('session').attributes('aria-describedby')}`);
    expect(line.text()).toBe('Until Font Fu quits.');
    expect(card('permanent').attributes('aria-describedby')).toBeUndefined();
    // No native radio button.
    expect(wrapper.find('input').exists()).toBe(false);
  });

  it('has one tab stop: the chosen card, else the first that is on', () => {
    const stops = (start: V | null) =>
      mountCards(start)
        .wrapper.findAll('[role="radio"]')
        .filter((c) => c.attributes('tabindex') === '0')
        .map((c) => c.attributes('data-value'));
    expect(stops('permanent')).toEqual(['permanent']);
    expect(stops(null)).toEqual(['session']);
  });

  it('moves the choice with the arrow keys, skipping a card that is off, and wraps', async () => {
    const { model, card } = mountCards();
    await card('session').trigger('keydown', { key: 'ArrowDown' });
    await nextTick();
    expect(model.value).toBe('permanent');
    expect(document.activeElement).toBe(card('permanent').element);
    await card('permanent').trigger('keydown', { key: 'ArrowRight' });
    await nextTick();
    expect(model.value).toBe('session');
    await card('session').trigger('keydown', { key: 'ArrowUp' });
    await nextTick();
    expect(model.value).toBe('permanent');
    await card('permanent').trigger('keydown', { key: 'ArrowLeft' });
    await nextTick();
    expect(model.value).toBe('session');
  });

  it('chooses with Space, Enter and a click; a card that is off is not chosen', async () => {
    const { model, card } = mountCards();
    await card('permanent').trigger('keydown', { key: ' ' });
    expect(model.value).toBe('permanent');
    await card('session').trigger('keydown', { key: 'Enter' });
    expect(model.value).toBe('session');
    await card('permanent').trigger('click');
    expect(model.value).toBe('permanent');
    await card('off').trigger('click');
    await card('off').trigger('keydown', { key: ' ' });
    expect(model.value).toBe('permanent');
    expect(card('off').attributes('aria-disabled')).toBe('true');
  });

  it('lays the cards out in one, two or three columns and draws the figure', () => {
    for (const cols of [1, 2, 3] as const) {
      const { wrapper } = mountCards('session', cols);
      expect(wrapper.get('[role="radiogroup"]').classes()).toContain(`cols-${cols}`);
    }
    const { wrapper } = mountCards();
    expect(wrapper.get('[role="radiogroup"]').classes()).toContain('cols-2');
    expect(wrapper.findAll('.figure i').map((i) => i.classes()[0])).toEqual([
      'session',
      'off',
      'permanent',
    ]);
  });
});
