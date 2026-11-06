import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { pickOption, selectOptions } from './testing';
import UiSelect from './UiSelect.vue';

const OPTIONS = [
  { value: 'pangram', label: 'Pangram' },
  { value: 'custom', label: 'Custom text' },
  { value: 'lorem', label: 'Lorem ipsum', disabled: true },
  { value: 'alphabet', label: 'Alphabet' },
];

const menu = () => document.body.querySelector<HTMLElement>('.ui-popover [role="listbox"]');
const options = () => [...(menu()?.querySelectorAll<HTMLElement>('[role="option"]') ?? [])];
const activeId = (w: ReturnType<typeof mountSelect>['w']) =>
  w.get('button').attributes('aria-activedescendant');
const activeLabel = (w: ReturnType<typeof mountSelect>['w']) =>
  document.getElementById(activeId(w) ?? '')?.textContent?.trim();

function mountSelect(props: Record<string, unknown> = {}) {
  const w = mount(UiSelect, {
    props: { modelValue: 'pangram', options: OPTIONS, 'aria-label': 'Sample', ...props },
    attachTo: document.body,
  });
  return { w };
}

afterEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

describe('UiSelect', () => {
  it('is a combobox button that shows the chosen option and lists the rest on click', async () => {
    const { w } = mountSelect();
    const button = w.get('button');
    expect(button.attributes('role')).toBe('combobox');
    expect(button.attributes('aria-expanded')).toBe('false');
    expect(button.attributes('data-value')).toBe('pangram');
    expect(button.get('.value').text()).toBe('Pangram');
    expect(menu()).toBeNull();

    expect(await selectOptions(button)).toEqual([
      ['pangram', 'Pangram'],
      ['custom', 'Custom text'],
      ['lorem', 'Lorem ipsum'],
      ['alphabet', 'Alphabet'],
    ]);
    expect(menu(), 'Escape closed it').toBeNull();

    await pickOption(button, 'alphabet');
    expect(w.emitted('update:modelValue')).toEqual([['alphabet']]);
    expect(menu()).toBeNull();
  });

  it('tells when the open menu chooses the current option again, and only then', async () => {
    const { w } = mountSelect();
    const button = w.get('button');
    await pickOption(button, 'pangram');
    expect(w.emitted('repick')).toEqual([['pangram']]);
    expect(w.emitted('update:modelValue')).toBeUndefined();
    expect(menu()).toBeNull();

    // Type-ahead on the closed button that lands on the current option is not a choice.
    await button.trigger('keydown', { key: 'p' });
    expect(w.emitted('repick')).toEqual([['pangram']]);

    await pickOption(button, 'alphabet');
    expect(w.emitted('repick')).toEqual([['pangram']]);
    expect(w.emitted('update:modelValue')).toEqual([['alphabet']]);
  });

  it('marks the chosen option and will not take a disabled one', async () => {
    const { w } = mountSelect();
    await w.get('button').trigger('click');
    await nextTick();
    expect(options().map((o) => o.getAttribute('aria-selected'))).toEqual([
      'true',
      'false',
      'false',
      'false',
    ]);
    expect(options()[2]!.getAttribute('aria-disabled')).toBe('true');
    options()[2]!.click();
    await nextTick();
    expect(w.emitted('update:modelValue')).toBeUndefined();
    expect(menu(), 'still open').not.toBeNull();
  });

  it('is driven by the keyboard with the focus staying on the button', async () => {
    const { w } = mountSelect();
    const button = w.get('button');
    await button.trigger('keydown', { key: 'ArrowDown' });
    await nextTick();
    expect(button.attributes('aria-expanded')).toBe('true');
    expect(activeLabel(w), 'starts on the chosen option').toBe('Pangram');
    await button.trigger('keydown', { key: 'ArrowDown' });
    expect(activeLabel(w)).toBe('Custom text');
    await button.trigger('keydown', { key: 'End' });
    expect(activeLabel(w)).toBe('Alphabet');
    await button.trigger('keydown', { key: 'a' });
    expect(activeLabel(w), 'type-ahead wraps').toBe('Alphabet');
    await button.trigger('keydown', { key: 'Home' });
    // Letters typed in quick succession are one word; a pause starts a new one.
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(Date.now() + 1000);
    await button.trigger('keydown', { key: 'c' });
    expect(activeLabel(w)).toBe('Custom text');
    await button.trigger('keydown', { key: 'Enter' });
    await nextTick();
    expect(w.emitted('update:modelValue')).toEqual([['custom']]);
    expect(button.attributes('aria-expanded')).toBe('false');
    expect(menu()).toBeNull();
  });

  it('closed, a typed letter changes the value like the native control', async () => {
    const { w } = mountSelect();
    await w.get('button').trigger('keydown', { key: 'c' });
    expect(w.emitted('update:modelValue')).toEqual([['custom']]);
    expect(menu()).toBeNull();
  });

  it('filters a long list as the user types', async () => {
    const many = Array.from({ length: 12 }, (_, i) => ({
      value: `l${i}`,
      label: i % 2 ? `Latin ${i}` : `Cyrillic ${i}`,
    }));
    const { w } = mountSelect({ modelValue: 'l0', options: many });
    const button = w.get('button');
    await button.trigger('click');
    await nextTick();
    expect(options()).toHaveLength(12);
    expect(document.body.querySelector('.filter')?.textContent).toContain('Type to filter');
    for (const key of ['l', 'a', 't']) await button.trigger('keydown', { key });
    expect(options().map((o) => o.textContent?.trim())).toEqual([
      'Latin 1',
      'Latin 3',
      'Latin 5',
      'Latin 7',
      'Latin 9',
      'Latin 11',
    ]);
    expect(document.body.querySelector('.filter')?.textContent).toContain('lat');
    await button.trigger('keydown', { key: 'Backspace' });
    expect(options()).toHaveLength(6);
    for (const key of ['z', 'z']) await button.trigger('keydown', { key });
    expect(options()).toHaveLength(0);
    expect(menu()?.textContent).toContain('No matches');
    await button.trigger('keydown', { key: 'Escape' });
    await nextTick();
    expect(menu()).toBeNull();
  });

  it('shows the placeholder while no option has the value', () => {
    const { w } = mountSelect({ modelValue: null, placeholder: 'Add…', fit: false });
    expect(w.get('.value').text()).toBe('Add…');
    expect(w.get('.value').classes()).toContain('placeholder');
    expect(w.get('button').attributes('data-value')).toBeUndefined();
    expect(w.findAll('.sizer')).toHaveLength(0);
  });

  it('passes id, name and class to the button so a label can point at it', () => {
    const { w } = mountSelect({ id: 'sample', name: 'sample', class: 'field' });
    const button = w.get('button');
    expect(button.attributes('id')).toBe('sample');
    expect(button.attributes('name')).toBe('sample');
    expect(button.classes()).toContain('field');
  });
});
